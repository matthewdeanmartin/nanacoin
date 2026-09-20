// The NanaCoin API client.
//
// This file knows the wire protocol and nothing about the DOM. It is written
// against `fetch` with no framework and no generated runtime, because the
// server it talks to may be a microcontroller and the page it runs in may be
// hosted anywhere (spec 3).

export type AccountID = string;
export type TransactionID = string;
export type ListingID = string;

export interface User {
  id: string;
  username: string;
  display_name: string;
  role: "nana" | "user";
  status: "ACTIVE" | "DISABLED";
  account: AccountID;
  created_at: number;
  balance?: number;
}

export interface Posting {
  account: AccountID;
  name: string;
  amount: number;
}

export interface Transaction {
  id: TransactionID;
  kind: "ISSUE" | "RETIRE" | "TRANSFER" | "PURCHASE" | "REVERSAL";
  created_at: number;
  actor: string;
  description: string;
  reference?: string;
  reverses?: TransactionID;
  reversed_by?: TransactionID;
  postings: Posting[];
}

export interface Listing {
  id: ListingID;
  seller: AccountID;
  seller_name: string;
  title: string;
  description: string;
  price: number;
  status: "ACTIVE" | "SOLD" | "CANCELLED";
  created_at: number;
  updated_at: number;
  buyer?: AccountID;
  buyer_name?: string;
  sold_tx?: TransactionID;
  kind?: string;
  currency?: string;
  minor_units?: number;
}

export interface Status {
  provisioned: boolean;
  household: string;
  currency: string;
  users: number;
  transactions: number;
  active_listings: number;
  circulation: number;
  journal_used: number;
  journal_capacity: number;
  ledger_balanced: boolean;
}

/** ApiError carries the server's machine-readable code alongside its message. */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

// --- PKCE -------------------------------------------------------------------

function base64url(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** A code verifier: 43 characters of base64url, the minimum the RFC allows. */
function newVerifier(): string {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  return base64url(bytes);
}

async function challengeFor(verifier: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier));
  return base64url(new Uint8Array(digest));
}

/**
 * An idempotency key, generated once per attempted operation. The whole point
 * is that it survives a retry, so it is made here - before the request - and
 * reused for every attempt at the same operation (spec 20).
 */
export function newIdempotencyKey(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return base64url(bytes);
}

// --- client -----------------------------------------------------------------

const TOKEN_KEY = "nanacoin.token";

export class Client {
  private token: string | null = null;

  constructor(readonly base: string) {
    // The token is kept in sessionStorage rather than localStorage: on a
    // shared household computer, closing the tab should end the session.
    try {
      this.token = sessionStorage.getItem(TOKEN_KEY);
    } catch {
      // Private browsing and blocked site data both throw here. A page that
      // simply asks the user to log in again is better than one that fails
      // to load.
      this.token = null;
    }
  }

  get authenticated(): boolean {
    return this.token !== null;
  }

  private setToken(token: string | null) {
    this.token = token;
    try {
      if (token) sessionStorage.setItem(TOKEN_KEY, token);
      else sessionStorage.removeItem(TOKEN_KEY);
    } catch {
      // Storage being unavailable costs the user a re-login on refresh and
      // nothing else.
    }
  }

  private async request<T>(
    method: string,
    path: string,
    body?: unknown,
    idempotencyKey?: string,
  ): Promise<T> {
    const headers: Record<string, string> = {};
    if (body !== undefined) headers["Content-Type"] = "application/json";
    if (this.token) headers["Authorization"] = `Bearer ${this.token}`;
    if (idempotencyKey) headers["Idempotency-Key"] = idempotencyKey;

    let res: Response;
    try {
      res = await fetch(`${this.base}${path}`, {
        method,
        headers,
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    } catch (e) {
      // A network failure here is the interesting case: the request may or
      // may not have reached the board. Saying so lets the caller retry with
      // the same idempotency key rather than guess.
      throw new ApiError(0, "network", "Could not reach the server. It may be offline, or the request may not have gone through.");
    }

    if (res.status === 204) return undefined as T;

    const text = await res.text();
    let parsed: unknown = null;
    if (text) {
      try {
        parsed = JSON.parse(text);
      } catch {
        throw new ApiError(res.status, "bad_response", "The server sent something that was not JSON.");
      }
    }

    if (!res.ok) {
      const e = parsed as { error?: string; message?: string } | null;
      if (res.status === 401) this.setToken(null);
      throw new ApiError(res.status, e?.error ?? "error", e?.message ?? res.statusText);
    }
    return parsed as T;
  }

  // --- auth ---

  status(): Promise<Status> {
    return this.request("GET", "/api/v1/status");
  }

  provision(username: string, displayName: string, password: string, householdName: string): Promise<User> {
    return this.request("POST", "/api/v1/provision", {
      username,
      display_name: displayName,
      password,
      household_name: householdName,
    });
  }

  /** Runs the full authorization-code + PKCE exchange. */
  async login(username: string, password: string): Promise<User> {
    const verifier = newVerifier();
    const challenge = await challengeFor(verifier);
    const redirectURI = location.origin + location.pathname;

    const authz = await this.request<{ code: string }>("POST", "/api/v1/auth/authorize", {
      username,
      password,
      code_challenge: challenge,
      code_challenge_method: "S256",
      redirect_uri: redirectURI,
    });

    const tok = await this.request<{ access_token: string; user: User }>(
      "POST",
      "/api/v1/auth/token",
      { code: authz.code, code_verifier: verifier, redirect_uri: redirectURI },
    );
    this.setToken(tok.access_token);
    return tok.user;
  }

  async logout(): Promise<void> {
    try {
      await this.request("POST", "/api/v1/auth/logout");
    } finally {
      // Whatever the server said, this browser is logged out.
      this.setToken(null);
    }
  }

  me(): Promise<User> {
    return this.request("GET", "/api/v1/me");
  }

  // --- users ---

  users(): Promise<{ users: User[] }> {
    return this.request("GET", "/api/v1/users");
  }

  createUser(username: string, displayName: string, password: string, grant: boolean): Promise<User> {
    return this.request("POST", "/api/v1/users", {
      username,
      display_name: displayName,
      password,
      grant,
    });
  }

  setUserStatus(id: string, status: "ACTIVE" | "DISABLED"): Promise<User> {
    return this.request("PATCH", `/api/v1/users/${encodeURIComponent(id)}`, { status });
  }

  // --- money ---

  accountTransactions(id: AccountID, limit = 50): Promise<{ account: AccountID; balance: number; transactions: Transaction[] }> {
    return this.request("GET", `/api/v1/accounts/${encodeURIComponent(id)}/transactions?limit=${limit}`);
  }

  allTransactions(limit = 100): Promise<{ transactions: Transaction[]; circulation: number }> {
    return this.request("GET", `/api/v1/transactions?limit=${limit}`);
  }

  transfer(to: AccountID, amount: number, memo: string, key: string): Promise<Transaction> {
    return this.request("POST", "/api/v1/transfers", { to, amount, memo }, key);
  }

  issue(to: AccountID, amount: number, reason: string, key: string): Promise<Transaction> {
    return this.request("POST", "/api/v1/admin/issue", { to, amount, reason }, key);
  }

  reverse(id: TransactionID, reason: string, key: string): Promise<Transaction> {
    return this.request("POST", `/api/v1/transactions/${encodeURIComponent(id)}/reverse`, { reason }, key);
  }

  // --- marketplace ---

  listings(status?: string): Promise<{ listings: Listing[] }> {
    const q = status ? `?status=${encodeURIComponent(status)}` : "";
    return this.request("GET", `/api/v1/listings${q}`);
  }

  createListing(input: {
    title: string;
    description: string;
    price: number;
    kind?: string;
    currency?: string;
    minor_units?: number;
  }): Promise<Listing> {
    return this.request("POST", "/api/v1/listings", input);
  }

  purchase(id: ListingID, key: string): Promise<{ listing: Listing; transaction: Transaction }> {
    return this.request("POST", `/api/v1/listings/${encodeURIComponent(id)}/purchase`, {}, key);
  }

  cancelListing(id: ListingID): Promise<Listing> {
    return this.request("POST", `/api/v1/listings/${encodeURIComponent(id)}/cancel`, {});
  }
}
