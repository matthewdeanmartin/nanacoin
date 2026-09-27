import { EconomicKind, Transaction, User } from '../api/models';

export interface ReportRow { label: string; received: number; paid: number }
export interface PublicReport {
  /** Money movements this report is built from (the latest page, not all history). */
  count: number;
  first: number | null;
  last: number | null;
  received: number;
  paid: number;
  rows: ReportRow[];
  /** Distinct people they traded with, most frequent first. */
  partners: { account: string; name: string; count: number }[];
}

const ROWS: { key: string; label: string }[] = [
  { key: 'LABOR', label: 'Work' },
  { key: 'GOOD', label: 'Goods' },
  { key: 'GIFT', label: 'Gifts' },
  { key: 'LOAN_PRINCIPAL', label: 'Loans' },
  { key: 'INTEREST', label: 'Interest' },
  { key: 'ISSUE', label: 'From Nana (new money)' },
  { key: 'RETIRE', label: 'Returned to Nana' },
  { key: 'OTHER', label: 'Other' },
];

function category(t: Transaction): string {
  if (t.kind === 'ISSUE' || t.kind === 'RETIRE') return t.kind;
  const kind: EconomicKind | undefined = t.economic_kind;
  return kind && ROWS.some((r) => r.key === kind) ? kind : 'OTHER';
}

/**
 * Someone's standing as the public ledger shows it: what came in and went out,
 * by kind of exchange, and who they trade with. Messages never appear here: the
 * board leaves them out of anyone else's view, and they move no money anyway.
 */
export function publicReport(account: string, transactions: readonly Transaction[]): PublicReport {
  const totals = new Map<string, ReportRow>();
  const partners = new Map<string, { account: string; name: string; count: number }>();
  let received = 0, paid = 0, count = 0, first: number | null = null, last: number | null = null;
  for (const t of transactions) {
    if (t.kind === 'MESSAGE') continue;
    const delta = t.postings.filter((p) => p.account === account).reduce((sum, p) => sum + p.amount, 0);
    if (delta === 0) continue;
    count++;
    first = first === null ? t.created_at : Math.min(first, t.created_at);
    last = last === null ? t.created_at : Math.max(last, t.created_at);
    const key = category(t);
    const row = totals.get(key) ?? { label: ROWS.find((r) => r.key === key)!.label, received: 0, paid: 0 };
    if (delta > 0) { row.received += delta; received += delta; } else { row.paid -= delta; paid -= delta; }
    totals.set(key, row);
    for (const p of t.postings) {
      if (p.account === account || !p.account.startsWith('account-') || p.account.endsWith('-usd')) continue;
      const seen = partners.get(p.account) ?? { account: p.account, name: p.name, count: 0 };
      seen.count++; partners.set(p.account, seen);
    }
  }
  return {
    count, first, last, received, paid,
    rows: ROWS.filter((r) => totals.has(r.key)).map((r) => totals.get(r.key)!),
    partners: [...partners.values()].sort((a, b) => b.count - a.count || a.name.localeCompare(b.name)),
  };
}

export interface Population {
  members: number;
  active: number;
  disabled: number;
  nanas: number;
  since: number | null;
  newest: User | null;
  /** Sum and median of visible coin balances (members only). */
  total: number;
  median: number | null;
}

export function population(users: readonly User[]): Population {
  const balances = users.filter((u) => u.status === 'ACTIVE' && typeof u.balance === 'number').map((u) => u.balance!).sort((a, b) => a - b);
  const mid = balances.length >> 1;
  const byJoin = [...users].sort((a, b) => a.created_at - b.created_at);
  return {
    members: users.length,
    active: users.filter((u) => u.status === 'ACTIVE').length,
    disabled: users.filter((u) => u.status !== 'ACTIVE').length,
    nanas: users.filter((u) => u.role === 'nana').length,
    since: byJoin[0]?.created_at ?? null,
    newest: byJoin.at(-1) ?? null,
    total: balances.reduce((sum, b) => sum + b, 0),
    median: balances.length === 0 ? null : balances.length % 2 ? balances[mid] : Math.floor((balances[mid - 1] + balances[mid]) / 2),
  };
}
