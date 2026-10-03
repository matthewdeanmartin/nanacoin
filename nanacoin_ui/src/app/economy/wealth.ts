import { Loan, Lotto, Transaction } from '../api/models';
import { Bucket, Series, SYSTEM_ISSUANCE, bucketStart } from './series';

export interface WealthPosition {
  cash: number;
  dollars: number;
  receivable: number;
  payable: number;
  interestAsset: number;
  interestDebt: number;
  savings: number;
}
export function wealthPosition(
  account: string,
  cash: number,
  dollars: number,
  loans: Loan[],
  lottos: Lotto[],
  transactions: Transaction[] = [],
): WealthPosition {
  const active = loans.filter((l) => l.status === 'ACTIVE');
  return {
    cash,
    dollars,
    receivable: active.filter((l) => l.lender === account).reduce((n, l) => n + l.principal, 0),
    payable: active.filter((l) => l.borrower === account).reduce((n, l) => n + l.principal, 0),
    interestAsset: active.filter((l) => l.lender === account).reduce((n, l) => n + l.interest, 0),
    interestDebt: active.filter((l) => l.borrower === account).reduce((n, l) => n + l.interest, 0),
    savings: lottos
      .filter((l) => l.terms.kind === 'SAVINGS' && l.status !== 'SETTLED')
      .reduce((n, l) => {
        const returned =
          l.status === 'PAYING'
            ? transactions
                .filter((t) => t.reference === `lotto-${l.id}` && t.economic_kind !== 'INTEREST')
                .reduce((sum, t) => sum + Math.max(0, delta(t, account)), 0)
            : 0;
        return n + Math.max(0, l.my_tickets * l.terms.ticket_price - returned);
      }, 0),
  };
}
export interface PersonalFlow {
  category: string;
  income: number;
  expenses: number;
}
const categories = ['Work and goods', 'Gifts', 'Interest', 'Other payments'];
function delta(t: Transaction, account: string): number {
  return t.postings.filter((p) => p.account === account).reduce((n, p) => n + p.amount, 0);
}
function savingsTransfer(t: Transaction, lottos: Lotto[]): boolean {
  return (
    t.economic_kind !== 'INTEREST' &&
    lottos.some((l) => l.terms.kind === 'SAVINGS' && t.reference === `lotto-${l.id}`)
  );
}
/** Cash accounting. Reversals reduce the original income/expense category. */
export function personalFlows(
  transactions: Transaction[],
  account: string,
  lottos: Lotto[] = [],
): PersonalFlow[] {
  const rows = categories.map((category) => ({ category, income: 0, expenses: 0 }));
  for (const t of transactions) {
    if (
      t.kind === 'MESSAGE' ||
      t.kind === 'RETIRE' ||
      t.economic_kind === 'LOAN_PRINCIPAL' ||
      t.reference?.startsWith('quote-') ||
      savingsTransfer(t, lottos)
    )
      continue;
    // Grants are capital funding; classified newly issued work/gifts/interest are income.
    if (
      (t.kind === 'ISSUE' || t.postings.some((p) => p.account === SYSTEM_ISSUANCE)) &&
      !['LABOR', 'GOOD', 'GIFT', 'INTEREST'].includes(t.economic_kind ?? '')
    )
      continue;
    if (
      t.reference?.startsWith('lotto-') &&
      t.economic_kind !== 'INTEREST' &&
      !lottos.some((l) => t.reference === `lotto-${l.id}`)
    )
      continue;
    const amount = delta(t, account);
    if (!amount) continue;
    const category =
      t.economic_kind === 'GOOD' || t.economic_kind === 'LABOR'
        ? 0
        : t.economic_kind === 'GIFT'
          ? 1
          : t.economic_kind === 'INTEREST'
            ? 2
            : 3;
    const reversal = t.kind === 'REVERSAL';
    const inbound = reversal ? amount < 0 : amount > 0;
    rows[category][inbound ? 'income' : 'expenses'] += (reversal ? -1 : 1) * Math.abs(amount);
  }
  return rows;
}
export function personalFlowSeries(
  transactions: Transaction[],
  account: string,
  bucket: Bucket,
  lottos: Lotto[],
): Series[] {
  const groups = new Map<number, Transaction[]>();
  for (const t of transactions) {
    const at = bucketStart(t.created_at, bucket);
    groups.set(at, [...(groups.get(at) ?? []), t]);
  }
  const points = [...groups]
    .sort(([a], [b]) => a - b)
    .map(([at, ts]) => {
      const rows = personalFlows(ts, account, lottos);
      return {
        at,
        income: rows.reduce((n, r) => n + r.income, 0),
        expenses: rows.reduce((n, r) => n + r.expenses, 0),
      };
    });
  return [
    { name: 'Income', points: points.map((p) => ({ at: p.at, value: p.income })) },
    { name: 'Expenses', points: points.map((p) => ({ at: p.at, value: p.expenses })) },
  ];
}
/** Anchor at today's actual position; reconstruct principal changes from retained events.
 * Accrued interest has no historical snapshots, so it is excluded from this chart. */
export function personalPositionSeries(
  transactions: Transaction[],
  account: string,
  position: WealthPosition,
  loans: Loan[],
  lottos: Lotto[],
  now = Math.floor(Date.now() / 1000),
): Series[] {
  const ordered = [...transactions].sort(
    (a, b) =>
      a.created_at - b.created_at ||
      Number(a.id.split('-').at(-1)) - Number(b.id.split('-').at(-1)),
  );
  const changes = ordered.map((t) => {
    const amount = delta(t, account);
    const loan = loans.find((l) => t.reference === `loan-${l.id}`);
    const principal = t.economic_kind === 'LOAN_PRINCIPAL';
    return {
      at: t.created_at,
      cash: amount,
      dollars: delta(t, `${account}-usd`),
      receivable: principal && loan?.lender === account ? -amount : 0,
      payable: principal && loan?.borrower === account ? amount : 0,
      savings: savingsTransfer(t, lottos) ? -amount : 0,
    };
  });
  const fields = ['cash', 'dollars', 'receivable', 'payable', 'savings'] as const;
  const running = { ...position };
  for (const c of changes) for (const f of fields) running[f] -= c[f];
  const snapshots = [{ at: changes[0]?.at ?? now, ...running }];
  for (const c of changes) {
    for (const f of fields) running[f] += c[f];
    snapshots.push({ at: c.at, ...running });
  }
  snapshots.push({ at: now, ...position });
  return [
    {
      name: 'NC assets (principal)',
      points: snapshots.map((p) => ({ at: p.at, value: p.cash + p.receivable + p.savings })),
    },
    {
      name: 'NC liabilities (principal)',
      points: snapshots.map((p) => ({ at: p.at, value: p.payable })),
    },
    { name: 'USD cash', points: snapshots.map((p) => ({ at: p.at, value: p.dollars })) },
  ];
}
