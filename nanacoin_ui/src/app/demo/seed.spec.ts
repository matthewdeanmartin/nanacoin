import { employmentSnapshot, inflationSeries, repeatPriceChanges } from '../economy/series';
import { DemoLedger } from './ledger';
import { seed } from './seed';

describe('public demo economic data', () => {
  it('demonstrates current employment, repeat-price inflation, and reusable things', () => {
    const ledger = new DemoLedger();
    seed(ledger);
    const transactions = ledger.ledger(500).transactions;
    const nana = ledger.userByName('nana')!;
    const workers = ledger
      .allUsers(nana)
      .filter((user) => user.role !== 'nana')
      .map((user) => user.account);
    expect(employmentSnapshot(transactions, workers).employed).toBeGreaterThan(0);
    expect(repeatPriceChanges(transactions)).toContainEqual({
      thing: 'Peanut butter cookies',
      unit: 'BATCH',
      previous: 6,
      latest: 9,
      percent: 0.5,
    });
    expect(inflationSeries(transactions, 'month').points.length).toBeGreaterThan(2);
    expect(inflationSeries(transactions, 'year').points.length).toBeGreaterThan(1);
    expect(ledger.allThings().some((thing) => thing.name === 'Peanut butter cookies')).toBe(true);
  });
});
