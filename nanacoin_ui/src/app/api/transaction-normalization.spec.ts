import { normalizeTransactions } from './transaction-normalization';

const transaction = () => ({ id: 'tx-1', postings: [{ account: 'account-2', name: 'Alice', amount: -100 }], current_postings: [{ account: 'account-2', name: 'Alice', amount: -1000 }], current_money_epoch: 1 });
describe('current currency transaction presentation', () => {
  it('normalizes direct, array, and nested offer/purchase transactions without changing original facts', () => {
    const tx = transaction();
    const response = { transaction: tx, transactions: [tx], trade: { coin_transaction: tx }, unchanged: { balance: 200 } };
    const result = normalizeTransactions(response);
    expect(result.transaction.postings[0].amount).toBe(-1000);
    expect(result.transactions[0].postings[0].amount).toBe(-1000);
    expect(result.trade.coin_transaction.postings[0].amount).toBe(-1000);
    expect((result.transaction as unknown as {original_postings: unknown}).original_postings).toBe(tx.postings);
    expect(tx.postings[0].amount).toBe(-100);
    expect(result.unchanged).toBe(response.unchanged);
    expect(normalizeTransactions(tx).postings[0].amount).toBe(-1000);
  });
  it('rejects unrepresentable historical values instead of rounding or aggregating old units', () => {
    expect(() => normalizeTransactions({ transactions: [{ ...transaction(), current_postings: null }] })).toThrow(/cannot be represented exactly/);
  });
  it('leaves demo response shapes unchanged and preserves original facts on repeated normalization', () => {
    const demo = { id: 'tx-demo', postings: [{amount:10}] };
    expect(normalizeTransactions(demo)).toBe(demo);
    const once = normalizeTransactions(transaction());
    expect(normalizeTransactions(once)).toEqual(once);
  });
});
