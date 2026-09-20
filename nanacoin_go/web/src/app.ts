// The NanaCoin household client.
//
// Everything here is presentation. The browser never decides whether a
// transaction is valid, what a balance is, or who may do what - it asks, and
// renders the answer (spec 2.1).

import { ApiError, Client, Listing, Status, Transaction, User, newIdempotencyKey } from "./api.js";

// The API base. Empty means same-origin, which is the development case; a
// cloud-hosted copy of this page sets it to the board's address.
const API_BASE = (document.body.dataset.api ?? "").replace(/\/$/, "");

const client = new Client(API_BASE);

let me: User | null = null;
let household: User[] = [];
let status: Status | null = null;

// --- tiny DOM helpers -------------------------------------------------------

function $(id: string): HTMLElement {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing element #${id}`);
  return el;
}

/**
 * Builds an element. Text is always set through textContent, never innerHTML:
 * memos, display names and listing titles are user input, and this is the one
 * place that guarantee is worth making structural.
 */
function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Record<string, string> = {},
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") node.className = v;
    else node.setAttribute(k, v);
  }
  for (const c of children) {
    node.append(typeof c === "string" ? document.createTextNode(c) : c);
  }
  return node;
}

function clear(node: HTMLElement) {
  node.replaceChildren();
}

function coins(n: number): string {
  // Integers throughout - there is no fractional NanaCoin (spec 2.3).
  return `${n} ${n === 1 ? "coin" : "coins"}`;
}

function when(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function toast(message: string, kind: "ok" | "error" = "ok") {
  const box = $("toast");
  box.textContent = message;
  box.className = `toast toast--${kind}`;
  box.hidden = false;
  window.setTimeout(() => {
    box.hidden = true;
  }, 5000);
}

function reportError(e: unknown) {
  if (e instanceof ApiError) {
    toast(e.message, "error");
  } else {
    toast("Something went wrong.", "error");
  }
}

// --- screens ----------------------------------------------------------------

function show(screen: "setup" | "login" | "app") {
  for (const s of ["setup", "login", "app"] as const) {
    $(s).hidden = s !== screen;
  }
}

async function boot() {
  try {
    status = await client.status();
  } catch (e) {
    reportError(e);
    return;
  }

  $("household-name").textContent = status.household;
  document.title = `${status.household} — NanaCoin`;

  if (!status.provisioned) {
    show("setup");
    return;
  }
  if (!client.authenticated) {
    show("login");
    return;
  }
  // A stored token may have expired while the tab was closed, or the board
  // may have rebooted - sessions are RAM-only by design (spec 17).
  try {
    me = await client.me();
    show("app");
    await refresh();
  } catch {
    show("login");
  }
}

// --- refresh ----------------------------------------------------------------

async function refresh() {
  if (!me) return;
  try {
    const [meNow, users, listings, st] = await Promise.all([
      client.me(),
      client.users(),
      client.listings(),
      client.status(),
    ]);
    me = meNow;
    household = users.users;
    status = st;

    renderIdentity();
    renderRecipients();
    renderMarket(listings.listings);
    await renderHistory();
    if (me.role === "nana") await renderAdmin();
  } catch (e) {
    reportError(e);
  }
}

function renderIdentity() {
  if (!me) return;
  $("whoami").textContent = me.display_name;
  $("balance").textContent = String(me.balance ?? 0);
  $("balance-unit").textContent = (me.balance ?? 0) === 1 ? "coin" : "coins";
  $("nana-panel").hidden = me.role !== "nana";
  $("role-badge").hidden = me.role !== "nana";
}

/** Fills every account picker with the household, minus oneself. */
function renderRecipients() {
  if (!me) return;
  for (const id of ["transfer-to", "issue-to"]) {
    const select = document.getElementById(id) as HTMLSelectElement | null;
    if (!select) continue;
    const previous = select.value;
    clear(select);
    for (const u of household) {
      // You cannot pay yourself, so you are not in your own list. Nana
      // issuing to herself is legitimate, so she stays in that one.
      if (id === "transfer-to" && u.id === me.id) continue;
      if (u.status !== "ACTIVE") continue;
      select.append(el("option", { value: u.account }, u.display_name));
    }
    if (previous) select.value = previous;
  }
}

function renderMarket(listings: Listing[]) {
  const active = $("market-active");
  const closed = $("market-closed");
  clear(active);
  clear(closed);

  let activeCount = 0;
  for (const l of listings) {
    const card = listingCard(l);
    if (l.status === "ACTIVE") {
      active.append(card);
      activeCount++;
    } else {
      closed.append(card);
    }
  }
  if (activeCount === 0) {
    active.append(el("p", { class: "empty" }, "Nothing for sale right now."));
  }
}

function listingCard(l: Listing): HTMLElement {
  const mine = me !== null && l.seller === me.account;
  const canAfford = me !== null && (me.balance ?? 0) >= l.price;

  const card = el("article", { class: `card card--${l.status.toLowerCase()}` });
  card.append(el("h3", {}, l.title));

  if (l.description) card.append(el("p", { class: "card__desc" }, l.description));

  if (l.kind === "currency" && l.currency && l.minor_units) {
    // Make it explicit that NanaCoin is not tracking the dollars (spec 14).
    card.append(
      el(
        "p",
        { class: "card__note" },
        `Offers ${(l.minor_units / 100).toFixed(2)} ${l.currency} in cash. NanaCoin records only the coin side — the cash is between you and the seller.`,
      ),
    );
  }

  const meta = el("p", { class: "card__meta" });
  meta.append(el("strong", {}, coins(l.price)), ` · ${l.seller_name}`);
  card.append(meta);

  if (l.status === "ACTIVE") {
    if (mine) {
      const cancel = el("button", { class: "btn btn--quiet" }, "Cancel listing");
      cancel.addEventListener("click", async () => {
        cancel.setAttribute("disabled", "");
        try {
          await client.cancelListing(l.id);
          toast("Listing cancelled.");
          await refresh();
        } catch (e) {
          reportError(e);
          cancel.removeAttribute("disabled");
        }
      });
      card.append(cancel);
    } else {
      const buy = el("button", { class: "btn" }, `Buy for ${l.price}`);
      if (!canAfford) {
        buy.setAttribute("disabled", "");
        buy.title = "You do not have enough coins.";
      }
      buy.addEventListener("click", () => purchase(l, buy));
      card.append(buy);
    }
  } else {
    const note = l.status === "SOLD" ? `Sold to ${l.buyer_name ?? "someone"}` : "Cancelled";
    card.append(el("p", { class: "card__status" }, note));
  }
  return card;
}

/**
 * Buying is one request. The client never transfers and then marks the listing
 * sold - those could partially succeed, and the server refuses to offer that
 * shape anyway (spec 13).
 */
async function purchase(l: Listing, button: HTMLElement) {
  // The key is made once, before the first attempt, so that a retry after a
  // dropped connection is recognisably the same purchase.
  const key = newIdempotencyKey();
  button.setAttribute("disabled", "");
  try {
    const res = await client.purchase(l.id, key);
    toast(`Bought ${res.listing.title} for ${coins(res.listing.price)}.`);
    await refresh();
  } catch (e) {
    reportError(e);
    button.removeAttribute("disabled");
  }
}

async function renderHistory() {
  if (!me) return;
  const list = $("history");
  clear(list);
  try {
    const { transactions } = await client.accountTransactions(me.account, 30);
    if (transactions.length === 0) {
      list.append(el("p", { class: "empty" }, "No transactions yet."));
      return;
    }
    for (const t of transactions) list.append(transactionRow(t, me.account));
  } catch (e) {
    reportError(e);
  }
}

function transactionRow(t: Transaction, viewpoint: string): HTMLElement {
  // The amount shown is this account's own posting: what actually happened to
  // you, not the gross size of the transaction.
  let delta = 0;
  for (const p of t.postings) if (p.account === viewpoint) delta += p.amount;

  const other = t.postings.find((p) => p.account !== viewpoint);
  const row = el("div", { class: `txn txn--${delta >= 0 ? "in" : "out"}` });

  const label = el("div", { class: "txn__main" });
  label.append(el("span", { class: "txn__desc" }, t.description || kindLabel(t.kind)));
  if (other) label.append(el("span", { class: "txn__who" }, delta < 0 ? `to ${other.name}` : `from ${other.name}`));
  row.append(label);

  const right = el("div", { class: "txn__side" });
  right.append(el("span", { class: "txn__amount" }, `${delta >= 0 ? "+" : ""}${delta}`));
  right.append(el("span", { class: "txn__when" }, when(t.created_at)));
  row.append(right);

  if (t.reversed_by) {
    row.append(el("span", { class: "tag tag--reversed" }, "reversed"));
  }
  if (t.kind === "REVERSAL") {
    row.append(el("span", { class: "tag" }, "correction"));
  }
  return row;
}

function kindLabel(kind: Transaction["kind"]): string {
  switch (kind) {
    case "ISSUE":
      return "Issued";
    case "RETIRE":
      return "Retired";
    case "PURCHASE":
      return "Purchase";
    case "REVERSAL":
      return "Correction";
    default:
      return "Transfer";
  }
}

// --- Nana's panel -----------------------------------------------------------

async function renderAdmin() {
  if (!status) return;
  $("stat-circulation").textContent = String(status.circulation);
  $("stat-users").textContent = String(status.users);
  $("stat-txns").textContent = String(status.transactions);
  $("stat-journal").textContent = `${(status.journal_used / 1024).toFixed(1)} KB`;

  const warn = $("ledger-warning");
  warn.hidden = status.ledger_balanced;

  const members = $("member-list");
  clear(members);
  for (const u of household) {
    const row = el("div", { class: "member" });
    row.append(el("span", { class: "member__name" }, u.display_name));
    row.append(el("span", { class: "member__balance" }, u.balance === undefined ? "—" : coins(u.balance)));
    if (u.role === "nana") row.append(el("span", { class: "tag" }, "nana"));
    if (u.status === "DISABLED") row.append(el("span", { class: "tag tag--off" }, "disabled"));
    members.append(row);
  }

  const ledger = $("full-ledger");
  clear(ledger);
  try {
    const { transactions } = await client.allTransactions(40);
    for (const t of transactions) ledger.append(ledgerRow(t));
  } catch (e) {
    reportError(e);
  }
}

/** The full-ledger view shows every posting, since Nana audits both sides. */
function ledgerRow(t: Transaction): HTMLElement {
  const row = el("div", { class: "ledger-row" });
  const head = el("div", { class: "ledger-row__head" });
  head.append(el("span", { class: "ledger-row__kind" }, kindLabel(t.kind)));
  head.append(el("span", { class: "ledger-row__desc" }, t.description || ""));
  head.append(el("span", { class: "ledger-row__when" }, when(t.created_at)));
  row.append(head);

  const postings = el("div", { class: "ledger-row__postings" });
  for (const p of t.postings) {
    postings.append(
      el(
        "span",
        { class: `posting posting--${p.amount >= 0 ? "credit" : "debit"}` },
        `${p.name} ${p.amount >= 0 ? "+" : ""}${p.amount}`,
      ),
    );
  }
  row.append(postings);

  if (t.reversed_by) {
    row.append(el("span", { class: "tag tag--reversed" }, "reversed"));
  } else if (t.kind !== "REVERSAL" && t.kind !== "ISSUE") {
    const undo = el("button", { class: "btn btn--quiet btn--small" }, "Reverse");
    undo.addEventListener("click", async () => {
      const reason = prompt("Why is this being reversed?");
      if (reason === null) return;
      undo.setAttribute("disabled", "");
      try {
        await client.reverse(t.id, reason, newIdempotencyKey());
        toast("Transaction reversed.");
        await refresh();
      } catch (e) {
        reportError(e);
        undo.removeAttribute("disabled");
      }
    });
    row.append(undo);
  }
  return row;
}

// --- forms ------------------------------------------------------------------

/**
 * Wires a form to a handler, disabling it while in flight. The disable matters
 * more than usual here: a double-clicked transfer button on a slow link is
 * exactly the situation idempotency keys exist for, and preventing the second
 * click is cheaper than deduplicating it.
 */
function onSubmit(id: string, handler: (form: HTMLFormElement) => Promise<void>) {
  const form = document.getElementById(id) as HTMLFormElement;
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const button = form.querySelector("button[type=submit]") as HTMLButtonElement | null;
    button?.setAttribute("disabled", "");
    try {
      await handler(form);
    } catch (err) {
      reportError(err);
    } finally {
      button?.removeAttribute("disabled");
    }
  });
}

function field(form: HTMLFormElement, name: string): string {
  const input = form.elements.namedItem(name) as HTMLInputElement | HTMLSelectElement | null;
  return input?.value.trim() ?? "";
}

function setupForms() {
  onSubmit("setup-form", async (form) => {
    await client.provision(
      field(form, "username"),
      field(form, "display_name"),
      field(form, "password"),
      field(form, "household_name"),
    );
    toast("Household created. Log in to continue.");
    form.reset();
    await boot();
  });

  onSubmit("login-form", async (form) => {
    me = await client.login(field(form, "username"), field(form, "password"));
    form.reset();
    show("app");
    await refresh();
  });

  onSubmit("transfer-form", async (form) => {
    const to = field(form, "to");
    if (!to) {
      // An empty <select required> passes browser validation, so without
      // this a transfer with no recipient would be sent and quietly refused.
      toast("There is nobody to send coins to yet.", "error");
      return;
    }
    const amount = Number(field(form, "amount"));
    if (!Number.isInteger(amount) || amount <= 0) {
      toast("Enter a whole number of coins.", "error");
      return;
    }
    await client.transfer(to, amount, field(form, "memo"), newIdempotencyKey());
    toast("Sent.");
    form.reset();
    await refresh();
  });

  onSubmit("listing-form", async (form) => {
    const price = Number(field(form, "price"));
    if (!Number.isInteger(price) || price <= 0) {
      toast("Enter a whole number of coins.", "error");
      return;
    }
    await client.createListing({
      title: field(form, "title"),
      description: field(form, "description"),
      price,
    });
    toast("Listed.");
    form.reset();
    await refresh();
  });

  onSubmit("issue-form", async (form) => {
    const to = field(form, "to");
    if (!to) {
      toast("Choose who the coins are for.", "error");
      return;
    }
    const amount = Number(field(form, "amount"));
    if (!Number.isInteger(amount) || amount <= 0) {
      toast("Enter a whole number of coins.", "error");
      return;
    }
    await client.issue(to, amount, field(form, "reason"), newIdempotencyKey());
    toast("Issued.");
    form.reset();
    await refresh();
  });

  onSubmit("member-form", async (form) => {
    await client.createUser(
      field(form, "username"),
      field(form, "display_name"),
      field(form, "password"),
      (form.elements.namedItem("grant") as HTMLInputElement).checked,
    );
    toast("Member added.");
    form.reset();
    await refresh();
  });

  $("logout").addEventListener("click", async () => {
    await client.logout();
    me = null;
    show("login");
  });

  $("refresh").addEventListener("click", () => refresh());

  // Tabs.
  for (const tab of document.querySelectorAll<HTMLElement>("[data-tab]")) {
    tab.addEventListener("click", () => {
      const name = tab.dataset.tab!;
      for (const t of document.querySelectorAll<HTMLElement>("[data-tab]")) {
        t.classList.toggle("tab--active", t === tab);
      }
      for (const panel of document.querySelectorAll<HTMLElement>("[data-panel]")) {
        panel.hidden = panel.dataset.panel !== name;
      }
    });
  }
}

setupForms();
boot();
