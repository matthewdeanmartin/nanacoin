// Turning a list of transactions into the three series the Economy page draws.
//
// All of this is pure and runs in the browser. The board is a microcontroller
// with a fixed RAM budget and no spare cycles for aggregation, and it already
// serves the transactions themselves - so the arithmetic belongs here, where
// adding a chart costs the board nothing.
//
// # What the window means
//
// The ledger retains a bounded number of transactions (365 at the time of
// writing) and overwrites the oldest. Lifetime balances survive that, but the
// transactions behind them do not, so these series describe the retained
// window rather than all of history. A chart that started its running total at
// zero would therefore be wrong - it would draw a household that began life
// however many transactions ago the window happens to hold. `openingBalance`
// exists to say where the line really starts.

import { Transaction } from '../api/models';

/**
 * Orders transactions as they were committed.
 *
 * Timestamps are unix *seconds*, so a busy moment - a seeded demo, or a parent
 * paying three chores in one go - puts several transactions on the same
 * second. Sorting by time alone then leaves their order to chance, and a
 * running balance rebuilt from it can dip below zero at points where the real
 * account never did.
 *
 * Transaction ids are assigned in commit order and look like "txn-12", so the
 * number breaks the tie exactly the way the ledger would. An id that does not
 * parse sorts as 0, which keeps the comparison total rather than throwing.
 */
function byCommitOrder(a: Transaction, b: Transaction): number {
  if (a.created_at !== b.created_at) return a.created_at - b.created_at;
  return sequence(a) - sequence(b);
}

function sequence(txn: Transaction): number {
  const n = Number(txn.id.slice(txn.id.lastIndexOf('-') + 1));
  return Number.isFinite(n) ? n : 0;
}

/** One plotted point: a moment, and the value at it. */
export interface Point {
  /** Unix seconds. */
  at: number;
  value: number;
}

export interface Series {
  name: string;
  points: Point[];
}

/** How to group transactions along the time axis. */
export type Bucket = 'day' | 'week';

/**
 * The GDP series: the value of economic activity per bucket.
 *
 * Counts TRANSFER and PURCHASE only. Issuance is money creation rather than
 * activity - counting it would let Nana raise GDP by printing - and retirement
 * is its destruction. A reversal is counted as negative activity in the bucket
 * it was made, which is what makes a corrected week visibly smaller rather
 * than silently identical.
 *
 * Only one side of each transaction is counted. Every transaction has a payer
 * and a payee, so summing the positive postings gives the value that changed
 * hands; summing all postings would give zero, since they are balanced.
 */
export function gdpSeries(txns: Transaction[], bucket: Bucket): Series {
  const totals = new Map<number, number>();

  for (const txn of txns) {
    let value = 0;
    const included = !txn.quantity_milli || txn.economic_kind === 'LABOR' || txn.economic_kind === 'GOOD';
    if (included && (txn.kind === 'TRANSFER' || txn.kind === 'PURCHASE')) {
      value = positiveSum(txn);
    } else if (included && txn.kind === 'REVERSAL') {
      // A reversal of a transfer undoes activity; a reversal of an issuance
      // was never activity in the first place. The postings say which: an
      // undone transfer still moves money between two household accounts.
      value = -positiveSum(txn);
    }
    if (value === 0) continue;

    const key = bucketStart(txn.created_at, bucket);
    totals.set(key, (totals.get(key) ?? 0) + value);
  }

  return { name: 'GDP', points: sorted(totals) };
}

export interface EmploymentSnapshot {
  employed: number;
  laborPool: number;
  rate: number;
  laborPayments: number;
  averagePayment: number;
}

/** People in the labor pool who received at least one labor payment this week. */
export function employmentSnapshot(
  txns: Transaction[],
  eligibleAccounts: readonly string[],
  now = Math.floor(Date.now() / 1000),
): EmploymentSnapshot {
  const start = bucketStart(now, 'week');
  const eligible = new Set(eligibleAccounts);
  const earners = new Set<string>();
  let laborPayments = 0;
  let count = 0;
  for (const txn of txns) {
    if (txn.created_at < start || txn.economic_kind !== 'LABOR' || txn.reversed_by || txn.kind === 'REVERSAL') continue;
    const payee = txn.postings.find((p) => p.amount > 0 && eligible.has(p.account));
    if (!payee) continue;
    earners.add(payee.account);
    laborPayments += payee.amount;
    count++;
  }
  const laborPool = eligible.size;
  return {
    employed: earners.size,
    laborPool,
    rate: laborPool ? earners.size / laborPool : 0,
    laborPayments,
    averagePayment: count ? laborPayments / count : 0,
  };
}

export function giftsThisWeek(
  txns: Transaction[],
  now = Math.floor(Date.now() / 1000),
): number {
  const start = bucketStart(now, 'week');
  return txns
    .filter((t) => t.created_at >= start && t.economic_kind === 'GIFT' && !t.reversed_by && t.kind !== 'REVERSAL')
    .reduce((sum, t) => sum + positiveSum(t), 0);
}

export interface RepeatPriceChange {
  thing: string;
  unit: string;
  previous: number;
  latest: number;
  percent: number;
}

/** Last two observed unit prices for every repeatedly sold standard good. */
export function repeatPriceChanges(txns: Transaction[]): RepeatPriceChange[] {
  const observations = new Map<string, { name: string; unit: string; at: number; price: number }[]>();
  for (const txn of txns) {
    if (txn.economic_kind !== 'GOOD' || !txn.thing || !txn.quantity_milli || txn.reversed_by || txn.kind === 'REVERSAL') continue;
    const price = positiveSum(txn) * 1000 / txn.quantity_milli;
    const list = observations.get(txn.thing) ?? [];
    list.push({ name: txn.thing_name ?? txn.description, unit: txn.unit ?? 'EACH', at: txn.created_at, price });
    observations.set(txn.thing, list);
  }
  return [...observations.values()].flatMap((values) => {
    if (values.length < 2) return [];
    values.sort((a, b) => a.at - b.at);
    const previous = values.at(-2)!;
    const latest = values.at(-1)!;
    return [{
      thing: latest.name,
      unit: latest.unit,
      previous: previous.price,
      latest: latest.price,
      percent: previous.price ? (latest.price - previous.price) / previous.price : 0,
    }];
  }).sort((a, b) => a.thing.localeCompare(b.thing));
}

/**
 * The money-supply series: total NanaCoin in circulation after each change.
 *
 * Only ISSUE and RETIRE move it, and a REVERSAL of either. Transfers between
 * household members do not change the supply however large they are, which is
 * the distinction this chart exists to show.
 *
 * `closing` is the circulation the server reports now. The series is built
 * backwards from it rather than forwards from zero, because the retained
 * window may not contain the issuance that created the coin already in
 * circulation - counting forwards from nothing would draw a household whose
 * money appeared from nowhere partway along.
 */
export function moneySupplySeries(txns: Transaction[], closing: number): Series {
  const ordered = [...txns].sort(byCommitOrder);

  // Walk backwards, undoing each change, to find what the supply was before
  // the window began.
  let opening = closing;
  for (const txn of ordered) opening -= supplyDelta(txn);

  const points: Point[] = [];
  let running = opening;
  if (ordered.length > 0) {
    points.push({ at: ordered[0].created_at, value: running });
  }
  for (const txn of ordered) {
    const delta = supplyDelta(txn);
    if (delta === 0) continue;
    running += delta;
    points.push({ at: txn.created_at, value: running });
  }
  return { name: 'Money supply', points };
}

/**
 * One account's balance over time.
 *
 * `closingBalance` is the balance the server reports now, and the line is
 * built backwards from it for the same reason the money supply is: the
 * retained window rarely starts at the account's creation, so a running total
 * from zero would draw the wrong balance for every point on the chart.
 */
export function balanceSeries(
  txns: Transaction[],
  account: string,
  closingBalance: number,
  name: string,
): Series {
  const ordered = [...txns]
    .filter((t) => t.postings.some((p) => p.account === account))
    .sort(byCommitOrder);

  let opening = closingBalance;
  for (const txn of ordered) opening -= accountDelta(txn, account);

  const points: Point[] = [];
  let running = opening;
  if (ordered.length > 0) {
    points.push({ at: ordered[0].created_at, value: running });
  }
  for (const txn of ordered) {
    running += accountDelta(txn, account);
    points.push({ at: txn.created_at, value: running });
  }
  return { name, points };
}

/** The sum of an account's postings in one transaction, which may be several. */
export function accountDelta(txn: Transaction, account: string): number {
  let delta = 0;
  for (const p of txn.postings) if (p.account === account) delta += p.amount;
  return delta;
}

/** Positive postings only: the value that changed hands. */
function positiveSum(txn: Transaction): number {
  let total = 0;
  for (const p of txn.postings) if (p.amount > 0) total += p.amount;
  return total;
}

/**
 * How much a transaction changed the total in circulation.
 *
 * Circulation is the negation of the issuance account's balance, so a
 * transaction's effect on it is the negation of its issuance postings -
 * whatever kind it claims to be. Reading the postings rather than switching on
 * Kind means a reversal of an issuance is handled without a special case.
 */
function supplyDelta(txn: Transaction): number {
  let delta = 0;
  for (const p of txn.postings) {
    if (p.account === SYSTEM_ISSUANCE) delta -= p.amount;
  }
  return delta;
}

/** The one account allowed to go negative; see ledger.SystemIssuance. */
export const SYSTEM_ISSUANCE = 'account:system-issuance';

/** The start of the bucket a timestamp falls in, in unix seconds, local time. */
export function bucketStart(unixSeconds: number, bucket: Bucket): number {
  const d = new Date(unixSeconds * 1000);
  d.setHours(0, 0, 0, 0);
  if (bucket === 'week') {
    // Weeks start on Monday, which is how a household thinks about chores.
    const weekday = (d.getDay() + 6) % 7;
    d.setDate(d.getDate() - weekday);
  }
  return Math.floor(d.getTime() / 1000);
}

function sorted(totals: Map<number, number>): Point[] {
  return [...totals.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([at, value]) => ({ at, value }));
}
