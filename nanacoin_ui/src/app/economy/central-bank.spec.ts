import { Loan, Lotto, Quote, Transaction } from '../api/models';
import { centralBankFlows, centralBankPosition, supplyGrowth } from './central-bank';

const NANA = 'account-1', KID = 'account-2', SUPPLY = 'account:system-issuance';
let seq = 0;
function tx(kind: Transaction['kind'], from: string, to: string, amount: number, extra: Partial<Transaction> = {}): Transaction {
  return {
    id: `tx-${++seq}`, kind, created_at: 1_000 + seq, actor: 'user-1', description: '',
    postings: [{ account: from, name: '', amount: -amount }, { account: to, name: '', amount }],
    ...extra,
  } as Transaction;
}

describe('central bank flows', () => {
  it('sorts issuance, retirement and corrections into the money supply', () => {
    const f = centralBankFlows([
      tx('ISSUE', SUPPLY, NANA, 1000),
      tx('ISSUE', SUPPLY, KID, 200),
      tx('ISSUE', SUPPLY, 'account:lotto-escrow', 7, { reference: 'lotto-3' }),
      tx('RETIRE', KID, SUPPLY, 50),
      tx('REVERSAL', KID, SUPPLY, 200),
      tx('MESSAGE', NANA, KID, 0),
    ], NANA);
    expect(f).toMatchObject({ issuedToNana: 1000, issuedToMembers: 200, issuedForLottoInterest: 7, retired: 50, corrections: -200, netIssuance: 957, counted: 5 });
  });

  it('tracks Nana on the exchange and recorded dollars separately', () => {
    const f = centralBankFlows([
      tx('TRANSFER', 'account:usd-issuance', `${NANA}-usd`, 5000),
      tx('TRANSFER', KID, NANA, 100, { reference: 'quote-9' }),
      tx('TRANSFER', `${NANA}-usd`, `${KID}-usd`, 100, { reference: 'quote-9' }),
      tx('TRANSFER', NANA, KID, 30, { reference: 'quote-10' }),
      tx('TRANSFER', `${KID}-usd`, `${NANA}-usd`, 45, { reference: 'quote-10' }),
    ], NANA);
    expect(f).toMatchObject({ usdRecorded: 5000, coinsBoughtBack: 100, usdPaidOut: 100, coinsSold: 30, usdTakenIn: 45, netIssuance: 0 });
  });

  it('separates interest from principal and nets reversals', () => {
    const f = centralBankFlows([
      tx('TRANSFER', NANA, KID, 500, { economic_kind: 'LOAN_PRINCIPAL', reference: 'loan-1' }),
      tx('TRANSFER', KID, NANA, 100, { economic_kind: 'LOAN_PRINCIPAL', reference: 'loan-1' }),
      tx('TRANSFER', KID, NANA, 12, { economic_kind: 'INTEREST', reference: 'loan-1' }),
      tx('PURCHASE', NANA, KID, 40, { economic_kind: 'LABOR' }),
      tx('REVERSAL', KID, NANA, 40, { economic_kind: 'LABOR' }),
      tx('TRANSFER', NANA, KID, 25, { economic_kind: 'GIFT' }),
    ], NANA);
    expect(f).toMatchObject({ lent: 500, repaid: 100, interestReceived: 12, boughtFromMembers: 0, paidOut: 25 });
  });

  it('honours the period', () => {
    seq = 0;
    const old = tx('ISSUE', SUPPLY, NANA, 1), recent = tx('ISSUE', SUPPLY, NANA, 2);
    expect(centralBankFlows([old, recent], NANA, recent.created_at).issuedToNana).toBe(2);
  });
});

describe('central bank position', () => {
  const scale = 10_000;
  it('measures the reserve against Nana\'s open buy offers', () => {
    const quotes = [
      { maker: NANA, side: 'BID', status: 'OPEN', live: true, coins: 10 * scale, cents_per_coin: 100, cents: 1000 },
      { maker: NANA, side: 'BID', status: 'OPEN', live: true, coins: 50 * scale, cents_per_coin: 75, cents: 3750 },
      { maker: NANA, side: 'ASK', status: 'OPEN', live: true, coins: 10 * scale, cents_per_coin: 150, cents: 1500 },
      { maker: KID, side: 'BID', status: 'OPEN', live: true, coins: scale, cents_per_coin: 90, cents: 90 },
    ] as Quote[];
    const loans = [{ lender: NANA, status: 'ACTIVE', principal: 300, interest: 5, overdue: 20, amount: 400 }] as Loan[];
    const lottos = [{ house: NANA, status: 'OPEN', pool: 40, interest: 4, terms: { kind: 'SAVINGS' } }] as Lotto[];
    const p = centralBankPosition({ circulation: 1000 * scale, scale, nana: NANA, nanaCoins: 200 * scale, usdReserve: 2375, quotes, loans, lottos });
    expect(p.buyBackPromise).toBe(4750);
    expect(p.reserveRatio).toBe(0.5);
    expect(p.publicCoins).toBe(800 * scale);
    expect(p.backingCentsPerCoin).toBeCloseTo(2.97, 2);
    expect(p).toMatchObject({ bestBid: 100, bestAsk: 150, sellPromiseCoins: 10 * scale, loansOwedToNana: 305, loansOverdueToNana: 20, lottoPools: 40, lottoInterestPromised: 4 });
  });

  it('reports supply growth against the opening supply', () => {
    expect(supplyGrowth(1100, 100)).toBeCloseTo(0.1);
    expect(supplyGrowth(100, 100)).toBeNull();
  });
});
