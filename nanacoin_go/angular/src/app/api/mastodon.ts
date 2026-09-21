import { Injectable, computed, inject, signal } from '@angular/core';

import { User } from './models';
import { NanacoinService } from './nanacoin.service';
import { Session } from './session';
import { digestSha256 } from './sha256';

const PENDING_KEY = 'nanacoin:mastodon:oauth';
const CREDENTIAL_KEY = 'nanacoin:mastodon:credentials:';
const SCOPES = 'read:accounts write:statuses';

interface PendingOAuth {
  userId: string;
  server: string;
  clientId: string;
  clientSecret: string;
  redirectUri: string;
  verifier: string;
  state: string;
}

interface MastodonCredentials {
  server: string;
  clientId: string;
  clientSecret: string;
  accessToken: string;
  acct: string;
}

interface MastodonAccount { acct: string; }

function base64Url(bytes: Uint8Array): string {
  let raw = '';
  for (const byte of bytes) raw += String.fromCharCode(byte);
  return btoa(raw).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function randomValue(bytes: number): string {
  return base64Url(crypto.getRandomValues(new Uint8Array(bytes)));
}

function normalizeServer(value: string): string {
  const entered = value.trim().replace(/\/+$/, '');
  if (!entered) throw new Error('Enter your Mastodon server.');
  const withScheme = /^https?:\/\//i.test(entered) ? entered : `https://${entered}`;
  const url = new URL(withScheme);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.pathname !== '/') {
    throw new Error('Enter a Mastodon server such as mastodon.social.');
  }
  return url.origin;
}

async function responseJson<T>(response: Response): Promise<T> {
  if (response.ok) return response.json() as Promise<T>;
  let detail = `Mastodon returned ${response.status}.`;
  try {
    const body = await response.json() as { error?: string; error_description?: string };
    detail = body.error_description || body.error || detail;
  } catch { /* Keep the status-only message. */ }
  throw new Error(detail);
}

/** Browser-owned Mastodon OAuth and direct-message transport. */
@Injectable({ providedIn: 'root' })
export class Mastodon {
  private readonly api = inject(NanacoinService);
  private readonly session = inject(Session);
  private readonly changed = signal(0);

  readonly connected = computed(() => {
    this.changed();
    const id = this.session.me()?.id;
    return !!id && this.credentials(id) !== null;
  });

  readonly account = computed(() => {
    this.changed();
    const id = this.session.me()?.id;
    return id ? this.credentials(id)?.acct ?? null : null;
  });

  /** Register a browser app and leave for the selected instance's PKCE login. */
  async connect(serverEntry: string): Promise<never> {
    const userId = this.session.me()?.id;
    if (!userId) throw new Error('Sign in to NanaCoin first.');
    const server = normalizeServer(serverEntry);
    // Hash fragments are not sent in OAuth redirect URIs. Return to the app
    // root and let App move the callback query into the /send hash route.
    const redirectUri = new URL(document.baseURI).toString();
    const app = await responseJson<{ client_id: string; client_secret?: string }>(
      await fetch(`${server}/api/v1/apps`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          client_name: 'NanaCoin',
          redirect_uris: redirectUri,
          scopes: SCOPES,
          website: 'https://github.com/matthewdeanmartin/nanacoin',
        }),
      }),
    );
    const verifier = randomValue(64);
    const challenge = base64Url(await digestSha256(new TextEncoder().encode(verifier)));
    const state = randomValue(32);
    const pending: PendingOAuth = {
      userId, server, clientId: app.client_id, clientSecret: app.client_secret ?? '',
      redirectUri, verifier, state,
    };
    sessionStorage.setItem(PENDING_KEY, JSON.stringify(pending));
    const params = new URLSearchParams({
      client_id: pending.clientId,
      redirect_uri: redirectUri,
      response_type: 'code',
      scope: SCOPES,
      state,
      code_challenge: challenge,
      code_challenge_method: 'S256',
    });
    window.location.assign(`${server}/oauth/authorize?${params}`);
    return new Promise<never>(() => undefined);
  }

  hasPendingCallback(): boolean { return sessionStorage.getItem(PENDING_KEY) !== null; }

  async finish(code: string, returnedState: string): Promise<string> {
    const raw = sessionStorage.getItem(PENDING_KEY);
    sessionStorage.removeItem(PENDING_KEY);
    if (!raw) throw new Error('That Mastodon sign-in has expired. Start again.');
    let pending: PendingOAuth;
    try { pending = JSON.parse(raw) as PendingOAuth; }
    catch { throw new Error('That Mastodon sign-in could not be read. Start again.'); }
    if (!pending.state || pending.state.length !== returnedState.length) {
      throw new Error('Mastodon sign-in state did not match.');
    }
    let mismatch = 0;
    for (let i = 0; i < pending.state.length; i++) mismatch |= pending.state.charCodeAt(i) ^ returnedState.charCodeAt(i);
    if (mismatch || pending.userId !== this.session.me()?.id) {
      throw new Error('Mastodon sign-in did not match this NanaCoin user.');
    }
    const tokenBody = new URLSearchParams({
      grant_type: 'authorization_code', code, client_id: pending.clientId,
      client_secret: pending.clientSecret, redirect_uri: pending.redirectUri,
      code_verifier: pending.verifier, scope: SCOPES,
    });
    const token = await responseJson<{ access_token: string }>(await fetch(`${pending.server}/oauth/token`, {
      method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded' }, body: tokenBody,
    }));
    const account = await responseJson<MastodonAccount>(await fetch(`${pending.server}/api/v1/accounts/verify_credentials`, {
      headers: { Authorization: `Bearer ${token.access_token}` },
    }));
    const host = new URL(pending.server).host;
    const acct = `@${account.acct.includes('@') ? account.acct : `${account.acct}@${host}`}`;
    await this.api.setUserMastodonId(pending.userId, acct);
    localStorage.setItem(CREDENTIAL_KEY + pending.userId, JSON.stringify({
      server: pending.server, clientId: pending.clientId, clientSecret: pending.clientSecret,
      accessToken: token.access_token, acct,
    } satisfies MastodonCredentials));
    this.changed.update((n) => n + 1);
    await this.session.refresh();
    return acct;
  }

  disconnect(): void {
    const id = this.session.me()?.id;
    if (!id) return;
    localStorage.removeItem(CREDENTIAL_KEY + id);
    this.changed.update((n) => n + 1);
  }

  /** Opens the connected home instance's public compose intent when possible. */
  shareUrl(text: string): string {
    const id = this.session.me()?.id;
    const server = id ? this.credentials(id)?.server : null;
    return `${server ?? 'https://mastodon.social'}/share?${new URLSearchParams({ text })}`;
  }

  /** Always a direct status, and only accepts a registered household member. */
  async sendDirect(recipient: User, message: string): Promise<void> {
    const senderId = this.session.me()?.id;
    const credentials = senderId ? this.credentials(senderId) : null;
    if (!credentials) throw new Error('Connect your Mastodon account first.');
    if (!recipient.mastodon_id) throw new Error(`${recipient.display_name} has no registered Mastodon ID.`);
    const text = message.trim();
    if (!text) throw new Error('Write a message first.');
    await responseJson(await fetch(`${credentials.server}/api/v1/statuses`, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${credentials.accessToken}`,
        'Content-Type': 'application/json',
        'Idempotency-Key': crypto.randomUUID(),
      },
      body: JSON.stringify({ status: `${recipient.mastodon_id} ${text}`, visibility: 'direct' }),
    }));
  }

  private credentials(userId: string): MastodonCredentials | null {
    const raw = localStorage.getItem(CREDENTIAL_KEY + userId);
    if (!raw) return null;
    try { return JSON.parse(raw) as MastodonCredentials; } catch { return null; }
  }
}
