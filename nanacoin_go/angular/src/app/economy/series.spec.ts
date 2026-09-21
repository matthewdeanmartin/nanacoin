// The series arithmetic decides what the economy charts claim happened, and a
// chart that is quietly wrong is worse than no chart. These are the cases that
// would be wrong in a way nobody notices: a window that does not reach back to
// the beginning, a reversal, an issuance mistaken for activity.

import { Transaction } from '../api/models';
import {
  SYSTEM_ISSUANCE,
  balanceSeries,
  bucketStart,
  employmentSnapshot,
  employmentSeries,
  gdpSeries,
  giftsThisWeek,
  moneySupplySeries,
  inflationSeries,
  repeatPriceChanges,
} from './series';

const DAY = 86_400;

function txn(partial: Partial<Transaction> & { postings: Transaction['postings'] }): Transaction {
  return {
    id: 'txn-1',
    kind: 'TRANSFER',
    created_at: 0,
    actor: 'user-1',
    description: '',
    ...partial,
  };
}

function transfer(at: number, amount: number, from = 'acct-a', to = 'acct-b'): Transaction {
  return txn({
    kind: 'TRANSFER',
    created_at: at,
    postings: [
      { account: from, name: 'A', amount: -amount },
      { account: to, name: 'B', amount },
    ],
  });
}

function issue(at: number, amount: number, to = 'acct-a'): Transaction {
  return txn({
    kind: 'ISSUE',
    created_at: at,
    postings: [
      { account: SYSTEM_ISSUANCE, name: 'issuance', amount: -amount },
      { account: to, name: 'A', amount },
    ],
  });
}

describe('gdpSeries', () => {
  it('counts the value that changed hands, not both sides of it', () => {
    const s = gdpSeries([transfer(0, 10)], 'day');
    expect(s.points.length).toBe(1);
    // Postings sum to zero; the activity is 10, not 0 and not 20.
    expect(s.points[0].value).toBe(10);
  });

  it('excludes issuance, so Nana cannot raise GDP by printing', () => {
    const s = gdpSeries([issue(0, 1000)], 'day');
    expect(s.points).toEqual([]);
  });

  it('counts purchases as activity', () => {
    const p = txn({
      kind: 'PURCHASE',
      created_at: 0,
      postings: [
        { account: 'acct-a', name: 'A', amount: -7 },
        { account: 'acct-b', name: 'B', amount: 7 },
      ],
    });
    expect(gdpSeries([p], 'day').points[0].value).toBe(7);
  });

  it('subtracts a reversal from the bucket it was made in', () => {
    const undo = txn({
      kind: 'REVERSAL',
      created_at: 0,
      postings: [
        { account: 'acct-b', name: 'B', amount: -10 },
        { account: 'acct-a', name: 'A', amount: 10 },
      ],
    });
    const s = gdpSeries([transfer(0, 10), undo], 'day');
    expect(s.points[0].value).toBe(0);
  });

  it('groups into buckets and orders them', () => {
    const s = gdpSeries([transfer(5 * DAY, 3), transfer(0, 1), transfer(0, 2)], 'day');
    expect(s.points.length).toBe(2);
    expect(s.points[0].value).toBe(3);
    expect(s.points[1].value).toBe(3);
    expect(s.points[0].at).toBeLessThan(s.points[1].at);
  });

  it('excludes classified gifts and other transfers', () => {
    const gift = { ...transfer(0, 12), economic_kind: 'GIFT' as const, quantity_milli: 1000 };
    expect(gdpSeries([gift], 'day').points).toEqual([]);
  });
});

describe('classified economy', () => {
  const now = Math.floor(new Date(2026, 8, 17, 12).getTime() / 1000);

  it('counts paid workers, excluding accounts outside the labor pool', () => {
    const labor = { ...transfer(now, 30, 'payer', 'worker'), economic_kind: 'LABOR' as const, quantity_milli: 1000 };
    expect(employmentSnapshot([labor], ['worker', 'idle'], now)).toEqual({
      employed: 1, laborPool: 2, rate: 0.5, laborPayments: 30, averagePayment: 30,
    });
  });

  it('tracks gifts separately', () => {
    const gift = { ...transfer(now, 9), economic_kind: 'GIFT' as const, quantity_milli: 1000 };
    expect(giftsThisWeek([gift], now)).toBe(9);
  });

  it('compares unit prices for repeat sales of the same good', () => {
    const first = { ...transfer(now - 10, 50), economic_kind: 'GOOD' as const, thing: 'thing-7', thing_name: 'Cookies', quantity_milli: 1000, unit: 'BATCH' as const };
    const second = { ...transfer(now, 120), id: 'txn-2', economic_kind: 'GOOD' as const, thing: 'thing-7', thing_name: 'Cookies', quantity_milli: 2000, unit: 'BATCH' as const };
    expect(repeatPriceChanges([first, second])).toEqual([{ thing: 'Cookies', unit: 'BATCH', previous: 50, latest: 60, percent: 0.2 }]);
  });

  it('plots employment rates by the selected period', () => {
    const january = Math.floor(new Date(2026, 0, 10, 12).getTime() / 1000);
    const february = Math.floor(new Date(2026, 1, 10, 12).getTime() / 1000);
    const labor = { ...transfer(january, 10, 'payer', 'worker'), economic_kind: 'LABOR' as const, quantity_milli: 1000 };
    const activity = transfer(february, 2);
    expect(employmentSeries([labor, activity], ['worker', 'idle'], 'month').points.map((point) => point.value))
      .toEqual([50, 0]);
  });

  it('plots repeat-sale inflation by month and year', () => {
    const january = Math.floor(new Date(2025, 0, 10, 12).getTime() / 1000);
    const february = Math.floor(new Date(2026, 1, 10, 12).getTime() / 1000);
    const first = { ...transfer(january, 10), economic_kind: 'GOOD' as const, thing: 'thing-1', quantity_milli: 1000 };
    const second = { ...transfer(february, 12), economic_kind: 'GOOD' as const, thing: 'thing-1', quantity_milli: 1000 };
    expect(inflationSeries([first, second], 'month').points[0].value).toBe(20);
    expect(inflationSeries([first, second], 'year').points[0].value).toBe(20);
  });
});

describe('moneySupplySeries', () => {
  it('ends at the circulation the server reports', () => {
    const s = moneySupplySeries([issue(0, 100)], 100);
    expect(s.points[s.points.length - 1].value).toBe(100);
  });

  it('starts from what the supply was before the window, not from zero', () => {
    // 500 already in circulation, and the retained window only shows 100 more
    // being issued. The line must start at 500, not at 0.
    const s = moneySupplySeries([issue(DAY, 100)], 600);
    expect(s.points[0].value).toBe(500);
    expect(s.points[s.points.length - 1].value).toBe(600);
  });

  it('ignores transfers however large', () => {
    const s = moneySupplySeries([issue(0, 100), transfer(DAY, 90)], 100);
    // The window contains the issuance that created all 100, so the line
    // opens at 0, rises once, and the 90 moving between two household
    // accounts adds no further point.
    expect(s.points.map((p) => p.value)).toEqual([0, 100]);
  });

  it('falls when coin is retired', () => {
    const retire = txn({
      kind: 'RETIRE',
      created_at: DAY,
      postings: [
        { account: 'acct-a', name: 'A', amount: -40 },
        { account: SYSTEM_ISSUANCE, name: 'issuance', amount: 40 },
      ],
    });
    const s = moneySupplySeries([issue(0, 100), retire], 60);
    expect(s.points[s.points.length - 1].value).toBe(60);
  });

  it('handles a reversed issuance without a special case', () => {
    const undoIssue = txn({
      kind: 'REVERSAL',
      created_at: DAY,
      postings: [
        { account: 'acct-a', name: 'A', amount: -100 },
        { account: SYSTEM_ISSUANCE, name: 'issuance', amount: 100 },
      ],
    });
    const s = moneySupplySeries([issue(0, 100), undoIssue], 0);
    expect(s.points[s.points.length - 1].value).toBe(0);
  });
});

describe('balanceSeries', () => {
  it('ends at the balance the server reports', () => {
    const s = balanceSeries([transfer(0, 10, 'acct-a', 'acct-b')], 'acct-b', 10, 'Bob');
    expect(s.points[s.points.length - 1].value).toBe(10);
  });

  it('starts from the balance before the window rather than zero', () => {
    // Bob is owed 25 already; the window shows him receiving 10 more.
    const s = balanceSeries([transfer(DAY, 10, 'acct-a', 'acct-b')], 'acct-b', 35, 'Bob');
    expect(s.points[0].value).toBe(25);
    expect(s.points[s.points.length - 1].value).toBe(35);
  });

  it('follows money out as well as in', () => {
    const s = balanceSeries([transfer(DAY, 10, 'acct-a', 'acct-b')], 'acct-a', 40, 'Alice');
    expect(s.points[0].value).toBe(50);
    expect(s.points[s.points.length - 1].value).toBe(40);
  });

  it('ignores transactions the account was not part of', () => {
    const s = balanceSeries([transfer(DAY, 10, 'acct-x', 'acct-y')], 'acct-a', 5, 'Alice');
    expect(s.points).toEqual([]);
  });

  it('is empty rather than misleading when there is no history', () => {
    expect(balanceSeries([], 'acct-a', 5, 'Alice').points).toEqual([]);
  });
});

describe('ordering within one second', () => {
  // Timestamps are unix seconds, so several transactions routinely share one.
  // Sorting by time alone leaves their order to chance, and a balance rebuilt
  // from that order can dip below zero where the real account never did. This
  // was found against a captured ledger whose 55 transactions all landed in
  // the same second.
  it('breaks ties by commit order, not by position in the array', () => {
    const at = 1_000_000;
    // Committed as: receive 100 (txn-1), then spend 60 (txn-2). Handed over
    // in the reverse order, as a newest-first API returns them.
    const spend: Transaction = {
      ...transfer(at, 60, 'acct-a', 'acct-b'),
      id: 'txn-2',
    };
    const receive: Transaction = {
      ...transfer(at, 100, 'acct-z', 'acct-a'),
      id: 'txn-1',
    };

    const s = balanceSeries([spend, receive], 'acct-a', 40, 'Alice');
    expect(s.points.map((p) => p.value)).toEqual([0, 100, 40]);
    expect(s.points.every((p) => p.value >= 0)).toBe(true);
  });
});

describe('bucketStart', () => {
  it('puts the same day in one bucket', () => {
    const morning = new Date(2026, 0, 15, 8, 30).getTime() / 1000;
    const evening = new Date(2026, 0, 15, 22, 5).getTime() / 1000;
    expect(bucketStart(morning, 'day')).toBe(bucketStart(evening, 'day'));
  });

  it('starts weeks on Monday', () => {
    // 2026-01-15 is a Thursday; its week starts on the 12th.
    const thursday = new Date(2026, 0, 15, 12).getTime() / 1000;
    const start = new Date(bucketStart(thursday, 'week') * 1000);
    expect(start.getDay()).toBe(1);
    expect(start.getDate()).toBe(12);
  });

  it('starts months and years at their first day', () => {
    const instant = Math.floor(new Date(2026, 8, 17, 12).getTime() / 1000);
    const month = new Date(bucketStart(instant, 'month') * 1000);
    const year = new Date(bucketStart(instant, 'year') * 1000);
    expect([month.getDate(), month.getMonth()]).toEqual([1, 8]);
    expect([year.getDate(), year.getMonth()]).toEqual([1, 0]);
  });
});
