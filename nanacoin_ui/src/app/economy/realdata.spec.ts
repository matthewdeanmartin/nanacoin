// The series functions against a real server's output.
//
// The unit tests use transactions shaped by hand, which is exactly the way to
// miss a wrong assumption about what the API actually sends. This fixture was
// captured from a running nanacoin: five people, eight weeks of allowance,
// chores and treats.

import { Transaction } from '../api/models';
import { balanceSeries, gdpSeries, moneySupplySeries } from './series';
import BALANCES_JSON from './testdata/balances.json';
import LEDGER from './testdata/ledger.json';

const txns = LEDGER.transactions as Transaction[];
const CIRCULATION = LEDGER.circulation;

/**
 * Balances the server reported at the moment the ledger was captured, keyed by
 * the generated account ids the server actually uses.
 */
const ACCOUNTS = BALANCES_JSON as Record<string, { name: string; balance: number }>;

/** Everyone but Nana, whose own account stays at zero in this household. */
const BALANCES: [string, number][] = Object.entries(ACCOUNTS)
  .filter(([, v]) => v.name !== 'Nana')
  .map(([account, v]) => [account, v.balance]);

describe('series against a captured ledger', () => {
  it('reads the fixture', () => {
    expect(txns.length).toBe(55);
    expect(CIRCULATION).toBe(880);
  });

  it('ends the money supply at the circulation the server reports', () => {
    const s = moneySupplySeries(txns, CIRCULATION);
    expect(s.points[s.points.length - 1].value).toBe(880);
  });

  it('starts the money supply at zero, since this window holds every issuance', () => {
    // The household was provisioned inside the captured window, so the line
    // genuinely begins at nothing - unlike a board that has overwritten its
    // early history.
    const s = moneySupplySeries(txns, CIRCULATION);
    expect(s.points[0].value).toBe(0);
  });

  it('never lets the money supply go negative', () => {
    const s = moneySupplySeries(txns, CIRCULATION);
    expect(s.points.every((p) => p.value >= 0)).toBe(true);
  });

  it('counts no issuance as GDP', () => {
    const gdp = gdpSeries(txns, 'week');
    const issued = txns
      .filter((t) => t.kind === 'ISSUE')
      .reduce((sum, t) => sum + t.postings.filter((p) => p.amount > 0)
        .reduce((a, p) => a + p.amount, 0), 0);
    const transferred = txns
      .filter((t) => t.kind === 'TRANSFER')
      .reduce((sum, t) => sum + t.postings.filter((p) => p.amount > 0)
        .reduce((a, p) => a + p.amount, 0), 0);

    const total = gdp.points.reduce((a, p) => a + p.value, 0);
    // Unclassified payments do not establish that production occurred.
    expect(total).toBe(0);
    expect(total).toBeLessThan(transferred + issued);
  });

  it('ends each balance line at the balance the server reports', () => {
    for (const [account, balance] of BALANCES) {
      const s = balanceSeries(txns, account, balance, account);
      expect(s.points[s.points.length - 1].value).toBe(balance);
    }
  });

  it('keeps every balance non-negative, as the ledger enforces', () => {
    for (const [account, balance] of BALANCES) {
      const s = balanceSeries(txns, account, balance, account);
      expect(s.points.every((p) => p.value >= 0)).toBe(true);
    }
  });

  it('has the balances sum to the money in circulation', () => {
    const total = BALANCES.reduce((a, [, b]) => a + b, 0);
    expect(total).toBe(CIRCULATION);
  });
});
