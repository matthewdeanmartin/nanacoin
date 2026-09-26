import { Loan, Lotto, Quote } from '../api/models';
import { MarketInput, addMonths, planMarket, shareOf } from './market-maker-plan';

const SCALE = 10_000; // four decimals
const coins = (n: number) => n * SCALE;
const now = Math.floor(Date.UTC(2026, 8, 26) / 1000);

function input(extra: Partial<MarketInput> = {}): MarketInput {
  return {
    circulation: coins(1000), scale: SCALE, now,
    nana: { account: 'account-1', balance: coins(5000), usdCents: 100_000 },
    quotes: [], loans: [], lottos: [],
    ...extra,
  };
}
const ladder = {
  bids: [{ shareBps: 100, cents: 100 }, { shareBps: 500, cents: 75 }, { shareBps: 1000, cents: 25 }, { shareBps: 10_000, cents: 1 }],
  asks: [{ shareBps: 100, cents: 150 }],
  expiresAt: 0, replace: true, topUpUsd: false, topUpCoins: false,
};

describe('market-maker plan', () => {
  it('scales the example ladder to circulation', () => {
    const plan = planMarket(input({ forex: ladder }));
    expect(plan.errors).toEqual([]);
    const bids = plan.steps.filter((s) => s.kind === 'quote' && s.side === 'BID');
    expect(bids.map((s) => s.kind === 'quote' && s.coins)).toEqual([coins(10), coins(50), coins(100), coins(1000)]);
    // 10 x $1 + 50 x $0.75 + 100 x $0.25 + 1000 x $0.01 = $82.50
    expect(plan.totals.bidCents).toBe(8250);
    // Double the money supply, double the rungs.
    expect(shareOf(coins(2000), 100, SCALE)).toBe(coins(20));
  });

  it('never makes a rung smaller than one whole coin', () => {
    expect(shareOf(coins(3), 100, SCALE)).toBe(coins(1));
  });

  it('refuses a buy price at or above the sell price', () => {
    const plan = planMarket(input({ forex: { ...ladder, asks: [{ shareBps: 100, cents: 90 }] } }));
    expect(plan.errors.join(' ')).toContain('free money');
  });

  it('asks for dollars Nana does not have, or records them when told to', () => {
    const poor = input({ nana: { account: 'account-1', balance: coins(5000), usdCents: 250 }, forex: ladder });
    expect(planMarket(poor).errors.join(' ')).toContain('$80.00 more');
    const topped = planMarket({ ...poor, forex: { ...ladder, topUpUsd: true } });
    expect(topped.errors).toEqual([]);
    expect(topped.steps.find((s) => s.kind === 'issue-usd')).toMatchObject({ cents: 8000 });
  });

  it('withdraws Nana\'s old quotes first and counts the freed slots', () => {
    const old = Array.from({ length: 14 }, (_, i) => ({ id: `q${i}`, maker: i < 10 ? 'account-1' : 'account-2', status: 'OPEN', side: 'BID', cents_per_coin: 5 }) as Quote);
    const plan = planMarket(input({ quotes: old, forex: ladder }));
    expect(plan.errors).toEqual([]);
    expect(plan.steps.slice(0, 10).every((s) => s.kind === 'cancel-quote')).toBe(true);
    expect(planMarket(input({ quotes: old, forex: { ...ladder, replace: false } })).errors.join(' ')).toContain('holds 16');
  });

  it('offers every chosen borrower each loan size', () => {
    const plan = planMarket(input({ lending: {
      borrowers: [{ account: 'a2', name: 'Ann' }, { account: 'a3', name: 'Ben' }],
      tiers: [{ label: 'Small', shareBps: 100, rateBps: 100 }, { label: 'Large', shareBps: 1500, rateBps: 1500 }],
      paymentDays: 7, replace: true,
    } }));
    const loans = plan.steps.filter((s) => s.kind === 'loan');
    expect(loans).toHaveLength(4);
    expect(loans[1]).toMatchObject({ input: { borrower: 'a2', amount: coins(150), rate_bps: 1500, rate_days: 30, installment: coins(38) } });
    expect(plan.totals.loanCoins).toBe(coins(320));
  });

  it('respects the loan capacity', () => {
    const busy = Array.from({ length: 30 }, (_, i) => ({ id: i, lender: 'x', status: 'ACTIVE' }) as Loan);
    const plan = planMarket(input({ loans: busy, lending: { borrowers: [{ account: 'a2', name: 'Ann' }], tiers: [{ label: 'S', shareBps: 100, rateBps: 100 }, { label: 'M', shareBps: 200, rateBps: 200 }, { label: 'L', shareBps: 300, rateBps: 300 }], paymentDays: 7, replace: false } }));
    expect(plan.errors.join(' ')).toContain('holds 32');
  });

  it('schedules a monthly lotto series, clamping short months', () => {
    const first = Math.floor(new Date(2027, 0, 31, 18, 0).getTime() / 1000);
    const plan = planMarket(input({ lotto: { count: 3, firstClose: first, kind: 'SAVINGS', ticketPrice: coins(1), rateBps: 200, title: 'Monthly' } }));
    const closes = plan.steps.map((s) => s.kind === 'lotto' ? new Date(s.terms.closes_at * 1000) : null);
    expect(closes.map((d) => [d!.getMonth(), d!.getDate()])).toEqual([[0, 31], [1, 28], [2, 31]]);
    expect(plan.steps[1]).toMatchObject({ terms: { title: 'Monthly · February 2027', rate_bps: 200 } });
    expect(addMonths(first, 12)).toBe(Math.floor(new Date(2028, 0, 31, 18, 0).getTime() / 1000));
  });

  it('keeps lotto slots for the rest of the house', () => {
    const running = Array.from({ length: 6 }, (_, i) => ({ id: i, status: 'OPEN' }) as Lotto);
    const plan = planMarket(input({ lottos: running, lotto: { count: 12, firstClose: now + 86_400, kind: 'SIMPLE', ticketPrice: coins(1), rateBps: 0, title: '' } }));
    expect(plan.errors.join(' ')).toContain('at most 10 more');
  });
});
