import { Loan, Lotto, Transaction } from '../api/models';
import { personalFlows, personalPositionSeries, wealthPosition } from './wealth';

function transaction(id: number, amount: number, fields: Partial<Transaction> = {}): Transaction {
  return {
    id: `tx-${id}`,
    created_at: id * 86400,
    actor: 'user-1',
    description: 'Payment',
    kind: 'TRANSFER',
    economic_kind: 'GOOD',
    postings: [
      { account: 'account-1', name: 'Me', amount },
      { account: 'account-2', name: 'Them', amount: -amount },
    ],
    ...fields,
  };
}
const loan = (fields: Partial<Loan> = {}): Loan => ({
  id: 1,
  lender: 'account-2',
  lender_name: 'Them',
  borrower: 'account-1',
  borrower_name: 'Me',
  amount: 100,
  rate_bps: 100,
  rate_days: 30,
  payment_days: 30,
  installment: 10,
  credit: false,
  memo: '',
  status: 'ACTIVE',
  principal: 80,
  interest: 4,
  overdue: 0,
  next_due_at: 0,
  created_at: 0,
  updated_at: 0,
  waiting_reason: '',
  ...fields,
});
const savings: Lotto = {
  id: 1,
  terms: { kind: 'SAVINGS', title: 'Save', ticket_price: 10, closes_at: 0, rate_bps: 100 },
  house: 'account-2',
  pool: 20,
  interest: 1,
  tickets: 2,
  my_tickets: 2,
  winner: null,
  winner_name: null,
  due_at: 0,
  status: 'WAITING',
};

describe('personal wealth accounting', () => {
  it('nets refunds against original income or expense and excludes principal, exchanges, savings deposits and capital grants', () => {
    const transactions = [
      transaction(1, 100),
      transaction(2, -30),
      transaction(3, 10, { kind: 'REVERSAL' }),
      transaction(4, -20, { kind: 'REVERSAL' }),
      transaction(5, 50, { economic_kind: 'LOAN_PRINCIPAL' }),
      transaction(6, -100, { reference: 'quote-1' }),
      transaction(7, -20, { reference: 'lotto-1', economic_kind: 'OTHER' }),
      transaction(8, 200, { kind: 'ISSUE', economic_kind: 'OTHER' }),
      transaction(9, 5, { economic_kind: 'INTEREST', kind: 'ISSUE' }),
    ];
    const rows = personalFlows(transactions, 'account-1', [savings]);
    expect(rows[0]).toEqual({ category: 'Work and goods', income: 80, expenses: 20 });
    expect(rows[2].income).toBe(5);
    expect(rows[3]).toEqual({ category: 'Other payments', income: 0, expenses: 0 });
  });
  it('counts only funded loans, treats overdue as part of debt, and excludes settled savings', () => {
    const position = wealthPosition(
      'account-1',
      50,
      300,
      [
        loan(),
        loan({ id: 2, lender: 'account-1', borrower: 'account-2', principal: 40, interest: 2 }),
        loan({ id: 3, status: 'ARMED', principal: 999 }),
        loan({ id: 4, status: 'PAID', principal: 0, interest: 0 }),
      ],
      [savings, { ...savings, status: 'SETTLED' }],
    );
    expect(position).toEqual({
      cash: 50,
      dollars: 300,
      receivable: 40,
      payable: 80,
      interestAsset: 2,
      interestDebt: 4,
      savings: 20,
    });
  });
  it('anchors history to current assets and keeps borrowing and repayment neutral to principal net worth', () => {
    const transactions = [
      transaction(1, 100, { reference: 'loan-1', economic_kind: 'LOAN_PRINCIPAL' }),
      transaction(2, -20, { reference: 'loan-1', economic_kind: 'LOAN_PRINCIPAL' }),
    ];
    const position = wealthPosition('account-1', 130, 0, [loan()], []);
    const [assets, debts] = personalPositionSeries(
      transactions,
      'account-1',
      position,
      [loan()],
      [],
      3 * 86400,
    );
    expect(assets.points.map((p) => p.value)).toEqual([50, 150, 130, 130]);
    expect(debts.points.map((p) => p.value)).toEqual([0, 100, 80, 80]);
    expect(assets.points.map((p, i) => p.value - debts.points[i].value)).toEqual([50, 50, 50, 50]);
  });
  it('keeps lending and savings deposits neutral to assets and reconstructs dollar cash independently', () => {
    const lent = loan({ lender: 'account-1', borrower: 'account-2', principal: 100 });
    const transactions = [
      transaction(1, -100, { reference: 'loan-1', economic_kind: 'LOAN_PRINCIPAL' }),
      transaction(2, -20, { reference: 'lotto-1', economic_kind: 'OTHER' }),
      transaction(3, 0, { postings: [{ account: 'account-1-usd', name: 'Me', amount: 300 }] }),
    ];
    const p = wealthPosition('account-1', 30, 500, [lent], [savings]);
    const [assets, , dollars] = personalPositionSeries(
      transactions,
      'account-1',
      p,
      [lent],
      [savings],
      4 * 86400,
    );
    expect(assets.points.every((point) => point.value === 150)).toBe(true);
    expect(dollars.points[0].value).toBe(200);
    expect(dollars.points.at(-1)?.value).toBe(500);
  });
  it('does not double count savings principal already returned while a draw is paying other members', () => {
    const paying = { ...savings, status: 'PAYING' as const };
    const payout = transaction(1, 20, { reference: 'lotto-1', economic_kind: 'OTHER' });
    expect(wealthPosition('account-1', 70, 0, [], [paying], [payout]).savings).toBe(0);
  });
});
