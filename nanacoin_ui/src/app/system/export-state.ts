// "Export server state": everything Nana would have to type in again to get a
// wiped or replaced board back to normal, as one ordinary HTML file she can
// save, print, or open later with no server running. Pure so it can be tested.

import { Fulfillment, Listing, Loan, Lotto, Offer, Quote, Status, Thing, User } from '../api/models';
import { Allowance } from '../pages/allowances';

export interface ServerState {
  exportedAt: number;
  source: string;
  status: Status | null;
  config: { household_name?: string; initial_grant?: number; currency?: string; offer_settles_after?: number } | null;
  transport: { https_only: boolean; supported: boolean } | null;
  users: User[];
  listings: Listing[];
  things: Thing[];
  loans: Loan[];
  lottos: Lotto[];
  quotes: Quote[];
  offers: Offer[];
  fulfillments: Fulfillment[];
  allowances: Allowance[];
  /** Sections that could not be read, so the file says what is missing. */
  missing: string[];
}

export interface Formatters {
  coins: (minor: number) => string;
  date: (unixSeconds: number) => string;
}

const esc = (value: unknown): string =>
  String(value ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);
const usd = (cents: number | undefined) => (cents === undefined ? '' : `$${(cents / 100).toFixed(2)}`);

function table(caption: string, headers: string[], rows: unknown[][], empty = 'None.'): string {
  if (!rows.length) return `<h3>${esc(caption)}</h3><p class="muted">${esc(empty)}</p>`;
  return `<table><caption>${esc(caption)}</caption><thead><tr>${headers.map((h) => `<th>${esc(h)}</th>`).join('')}</tr></thead>`
    + `<tbody>${rows.map((r) => `<tr>${r.map((c) => `<td>${esc(c)}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
}

/** The whole export as a standalone HTML document. */
export function exportHtml(s: ServerState, f: Formatters): string {
  const name = (account: string) => s.users.find((u) => u.account === account)?.display_name ?? account;
  const active = s.users.filter((u) => u.status === 'ACTIVE');
  const household = s.config?.household_name || s.status?.household || 'NanaCoin';
  const openLoans = s.loans.filter((l) => ['OFFERED', 'ARMED', 'ACTIVE'].includes(l.status));
  const openLottos = s.lottos.filter((l) => l.status !== 'SETTLED');
  const openQuotes = s.quotes.filter((q) => q.status === 'OPEN');
  const openOffers = s.offers.filter((o) => o.status === 'OPEN');
  const todo = s.fulfillments.filter((x) => x.status === 'TODO' || x.status === 'DISPUTED');
  const activeListings = s.listings.filter((l) => l.status === 'ACTIVE');
  // JSON inside a script element must not be able to close the element.
  const data = JSON.stringify(s, null, 2).replace(/</g, '\\u003c');

  const sections = [
    `<section><h2>1. Household settings</h2>`
      + table('Settings', ['Setting', 'Value'], [
        ['Household name', household],
        ['Currency name', s.config?.currency ?? s.status?.currency ?? ''],
        ['Starting grant for new members', s.config?.initial_grant === undefined ? '' : `${f.coins(s.config.initial_grant)} coins`],
        ['Offer settlement window', s.config?.offer_settles_after ? `${Math.round(s.config.offer_settles_after / 3600)} hours` : 'default'],
        ['Decimal places', s.status?.decimals ?? ''],
        ['HTTPS required for everyone', s.transport ? (s.transport.https_only ? 'Yes' : 'No') : 'unknown'],
      ])
      + `<p>Set these on the setup screen and under Household → Configuration.</p></section>`,

    `<section><h2>2. Members</h2>`
      + table('Everyone in the household', ['Name', 'Username', 'Role', 'Status', 'Coins', 'Dollars', 'Mastodon'],
        s.users.map((u) => [u.display_name, u.username, u.role, u.status, u.balance === undefined ? '' : f.coins(u.balance), usd(u.usd_cents), u.mastodon_id ?? '']))
      + `<p><strong>Passwords are not exported</strong> (the board only keeps scrambled copies). Give each person a new password when you add them back.</p>`
      + `<p>To restore money: add each member, then issue them their coins and record their dollars from Household → Issue money. Starting grants are paid automatically, so issue only the difference.</p></section>`,

    `<section><h2>3. Things for sale and wanted</h2>`
      + table('Active listings', ['Title', 'Posted by', 'Side', 'Price (coins)', 'Kind', 'Description'],
        activeListings.map((l) => [l.title, l.seller_name, l.side ?? 'SELL', f.coins(l.price), l.kind || l.economic_kind || '', l.description]))
      + table('Catalog of standard things', ['Name', 'Kind', 'Unit', 'Standard'], s.things.map((t) => [t.name, t.economic_kind, t.unit, t.standard ? 'yes' : 'no']))
      + table('Open offers on listings', ['Listing', 'From', 'Amount (coins)', 'Message'], openOffers.map((o) => [o.listing_title, o.offerer_name, f.coins(o.amount), o.message ?? '']))
      + `</section>`,

    `<section><h2>4. Promises still running</h2>`
      + table('Loans', ['Lender', 'Borrower', 'Status', 'Amount', 'Principal left', 'Interest owed', 'Rate', 'Payments', 'Memo'],
        openLoans.map((l) => [l.lender_name, l.borrower_name, l.status, f.coins(l.amount), f.coins(l.principal), f.coins(l.interest),
          `${l.rate_bps / 100}% per ${l.rate_days} days`, `${f.coins(l.installment)} every ${l.payment_days} days`, l.memo]))
      + table('Lottos', ['Title', 'Kind', 'Status', 'Ticket', 'Tickets sold', 'Pool', 'Sales close', '30-day interest'],
        openLottos.map((l) => [l.terms.title, l.terms.kind, l.status, f.coins(l.terms.ticket_price), l.tickets, f.coins(l.pool), f.date(l.terms.closes_at), `${l.terms.rate_bps / 100}%`]))
      + `<p>A lotto's ticket holders are not listed per person by the board. Refund or re-run open lottos by hand.</p>`
      + table('Dollar exchange offers', ['Posted by', 'Side', 'Coins', 'Price per coin', 'Dollars'],
        openQuotes.map((q) => [q.maker_name, q.side === 'BID' ? 'buys coins' : 'sells coins', f.coins(q.coins), usd(q.cents_per_coin), usd(q.cents)]))
      + table('Work and deliveries still to do', ['What', 'Who does it', 'For whom', 'Status'],
        todo.map((x) => [x.description, x.provider_name, x.recipient_name, x.status]))
      + `</section>`,

    `<section><h2>5. Allowances saved in this browser</h2>`
      + table('Recurring payments', ['To', 'Amount (coins)', 'How often', 'Next due', 'Description'],
        s.allowances.map((a) => [a.recipientName, f.coins(a.amount), a.cadence.toLowerCase(), a.nextDue, a.memo]),
        'None in this browser. Allowances live in the payer\'s browser, so each parent should check their own.')
      + `</section>`,
  ];

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(household)} — NanaCoin export</title>
<style>
  :root { color-scheme: light dark; --ink:#1f1b16; --bg:#fbf8f3; --line:#d9d0c3; --muted:#6b6258; }
  @media (prefers-color-scheme: dark) { :root { --ink:#eee6db; --bg:#1c1915; --line:#3b352d; --muted:#a89f93; } }
  body { font: 16px/1.5 system-ui, sans-serif; color: var(--ink); background: var(--bg); margin: 0 auto; max-width: 60rem; padding: 1rem 16px 3rem; }
  table { border-collapse: collapse; width: 100%; margin: .5rem 0 1.25rem; display: block; overflow-x: auto; }
  caption { text-align: left; font-weight: 700; padding-bottom: .25rem; }
  th, td { border-bottom: 1px solid var(--line); padding: .35rem .6rem .35rem 0; text-align: left; vertical-align: top; }
  .muted { color: var(--muted); } .warn { border-left: 4px solid #b8562f; padding-left: .75rem; }
  @media print { details { display: none; } }
</style>
</head>
<body>
<h1>${esc(household)}: server state</h1>
<p class="muted">Exported ${esc(f.date(s.exportedAt))} from ${esc(s.source)} · ${active.length} active members · ${s.status ? `${f.coins(s.status.circulation)} coins in circulation` : ''}</p>
<p class="warn">This is what you would type in again if the NanaCoin board were wiped or replaced. Keep it somewhere private: it lists everyone's balances. It does not contain passwords or transaction history.</p>
${s.missing.length ? `<p class="warn">Could not read: ${esc(s.missing.join(', '))}. Those parts are missing from this file.</p>` : ''}
${sections.join('\n')}
<details><summary>Machine-readable copy</summary>
<p class="muted">The same data as JSON, for a future import tool.</p>
<pre>${esc(data)}</pre>
</details>
<script type="application/json" id="nanacoin-export">${data}</script>
</body>
</html>
`;
}

export function exportFileName(household: string, at: number): string {
  const slug = household.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'nanacoin';
  return `${slug}-export-${new Date(at * 1000).toISOString().slice(0, 10)}.html`;
}
