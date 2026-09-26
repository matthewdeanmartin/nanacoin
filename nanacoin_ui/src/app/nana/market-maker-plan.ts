// Nana as market maker: turns a few ladders into a checked list of ordinary
// API calls. Everything here is pure so the arithmetic and the capacity rules
// can be tested without a page or a server. See
// nanacoin_rs/spec/NANA_AS_MARKET_MAKER.md.

import { Loan, LoanOfferInput, Lotto, LottoKind, LottoTerms, Quote, QuoteSide } from '../api/models';

/** Board limits (nanacoin_rs forex.rs QUOTES, loans.rs LOANS, lotto.rs LOTTOS). */
export const QUOTE_CAPACITY = 16;
export const LOAN_CAPACITY = 32;
export const LOTTO_CAPACITY = 16;
/** Slots a batch should leave for the rest of the household. */
export const QUOTE_RESERVE = 4;
export const LOAN_RESERVE = 8;
export const LOTTO_RESERVE = 2;

/** One rung of the dollar ladder: a share of circulation at a price. */
export interface PriceTier {
  /** Share of coins in circulation, in basis points (100 = 1%). */
  shareBps: number;
  /** Whole cents per coin. */
  cents: number;
}

/** One size of standing loan. */
export interface LoanTier {
  label: string;
  /** Loan size as a share of circulation, in basis points. */
  shareBps: number;
  /** Interest per 30 days, in basis points. */
  rateBps: number;
}

export interface LottoSeries {
  count: number;
  /** Closing time of the first draw, in Unix seconds. */
  firstClose: number;
  kind: LottoKind;
  ticketPrice: number;
  rateBps: number;
  title: string;
}

export interface MarketInput {
  /** Coins in circulation, minor units. */
  circulation: number;
  /** Minor units per whole coin (10^decimals). */
  scale: number;
  nana: { account: string; balance: number; usdCents: number };
  now: number;
  quotes: Quote[];
  loans: Loan[];
  lottos: Lotto[];
  forex?: {
    bids: PriceTier[];
    asks: PriceTier[];
    expiresAt: number;
    replace: boolean;
    topUpUsd: boolean;
    topUpCoins: boolean;
  };
  lending?: {
    borrowers: { account: string; name: string }[];
    tiers: LoanTier[];
    paymentDays: 1 | 7 | 30;
    replace: boolean;
  };
  lotto?: LottoSeries;
}

export type Step =
  | { kind: 'cancel-quote'; label: string; id: string }
  | { kind: 'close-loan'; label: string; id: number }
  | { kind: 'issue'; label: string; amount: number }
  | { kind: 'issue-usd'; label: string; cents: number }
  | { kind: 'quote'; label: string; side: QuoteSide; cents_per_coin: number; coins: number; expires_at?: number }
  | { kind: 'loan'; label: string; input: LoanOfferInput }
  | { kind: 'lotto'; label: string; terms: LottoTerms };

export interface Plan {
  steps: Step[];
  /** Must be fixed before the batch can run. */
  errors: string[];
  /** Worth reading, but the batch may still run. */
  warnings: string[];
  totals: {
    /** Dollars Nana pays if every buy offer is taken. */
    bidCents: number;
    /** Coins Nana hands over if every sell offer is taken. */
    askCoins: number;
    /** Coins Nana lends if every loan offer is accepted. */
    loanCoins: number;
    lottos: number;
  };
}

/**
 * A share of circulation in whole coins, never less than one coin. Whole coins
 * keep every quote's dollar side exact (the server requires coins x cents to
 * divide evenly by the coin scale). BigInt because circulation x basis points
 * can pass 2^53.
 */
export function shareOf(circulation: number, shareBps: number, scale: number): number {
  const whole = (BigInt(Math.max(circulation, 0)) * BigInt(shareBps)) / 10_000n / BigInt(scale);
  return Number((whole < 1n ? 1n : whole) * BigInt(scale));
}

/** The dollar side of a quote, in cents. Exact because coins are whole. */
export function quoteCents(coins: number, cents: number, scale: number): number {
  return Number((BigInt(coins) * BigInt(cents)) / BigInt(scale));
}

/** The same day of the month, `months` later, clamped to the month's end. */
export function addMonths(unixSeconds: number, months: number): number {
  const d = new Date(unixSeconds * 1000);
  const day = d.getDate();
  const target = new Date(d.getFullYear(), d.getMonth() + months, 1, d.getHours(), d.getMinutes());
  const last = new Date(target.getFullYear(), target.getMonth() + 1, 0).getDate();
  target.setDate(Math.min(day, last));
  return Math.floor(target.getTime() / 1000);
}

const MONTH_NAME = new Intl.DateTimeFormat('en-US', { month: 'long', year: 'numeric' });

export function planMarket(input: MarketInput): Plan {
  const steps: Step[] = [], errors: string[] = [], warnings: string[] = [];
  const totals = { bidCents: 0, askCoins: 0, loanCoins: 0, lottos: 0 };
  const { scale, circulation, nana, now } = input;
  if (circulation <= 0) errors.push('There are no coins in circulation yet. Issue some coins first, so the ladders have something to scale to.');

  // --- dollars ---------------------------------------------------------------
  if (input.forex) {
    const f = input.forex;
    const mine = input.quotes.filter((q) => q.maker === nana.account && q.status === 'OPEN');
    if (f.replace) for (const q of mine) steps.push({ kind: 'cancel-quote', id: q.id, label: `Withdraw old ${q.side === 'BID' ? 'buy' : 'sell'} offer at ${dollars(q.cents_per_coin)}` });
    const bids = f.bids.map((t) => ({ ...t, coins: shareOf(circulation, t.shareBps, scale) }));
    const asks = f.asks.map((t) => ({ ...t, coins: shareOf(circulation, t.shareBps, scale) }));
    totals.bidCents = bids.reduce((n, t) => n + quoteCents(t.coins, t.cents, scale), 0);
    totals.askCoins = asks.reduce((n, t) => n + t.coins, 0);
    if (bids.some((t) => t.cents < 1) || asks.some((t) => t.cents < 1)) errors.push('Every price must be at least 1 cent.');
    const bestBid = Math.max(0, ...bids.map((t) => t.cents)), bestAsk = Math.min(Infinity, ...asks.map((t) => t.cents));
    if (bids.length && asks.length && bestBid >= bestAsk)
      errors.push(`Nana's highest buy price (${dollars(bestBid)}) must be lower than her lowest sell price (${dollars(bestAsk)}). Otherwise anyone could buy from Nana and sell straight back for free money.`);
    for (let i = 1; i < bids.length; i++) if (bids[i].cents > bids[i - 1].cents)
      warnings.push('Buy prices usually go down rung by rung, so a big cash-out costs more than a small one.');

    const keptLive = input.quotes.filter((q) => q.status === 'OPEN' && !(f.replace && q.maker === nana.account)).length;
    const after = keptLive + bids.length + asks.length;
    if (after > QUOTE_CAPACITY) errors.push(`The board holds ${QUOTE_CAPACITY} open exchange offers. This batch would need ${after}. Use fewer rungs or replace Nana's old offers.`);
    else if (QUOTE_CAPACITY - after < QUOTE_RESERVE) warnings.push(`Only ${QUOTE_CAPACITY - after} exchange slots would be left for everyone else.`);

    const usdShort = totals.bidCents - nana.usdCents;
    if (usdShort > 0) {
      if (f.topUpUsd) steps.push({ kind: 'issue-usd', cents: usdShort, label: `Record ${dollars(usdShort)} of real dollars Nana is holding` });
      else errors.push(`Nana needs ${dollars(usdShort)} more in dollars to back every buy offer. Record the dollars she holds, or shrink the ladder.`);
    }
    const coinShort = totals.askCoins - nana.balance;
    if (coinShort > 0) {
      if (f.topUpCoins) steps.push({ kind: 'issue', amount: coinShort, label: 'Issue new coins to Nana so she can back her sell offers' });
      else errors.push('Nana does not hold enough coins to back every sell offer. Issue coins to Nana, or shrink the sell ladder.');
    }
    for (const t of bids) steps.push({ kind: 'quote', side: 'BID', cents_per_coin: t.cents, coins: t.coins, ...(f.expiresAt ? { expires_at: f.expiresAt } : {}), label: `Nana buys coins at ${dollars(t.cents)} each` });
    for (const t of asks) steps.push({ kind: 'quote', side: 'ASK', cents_per_coin: t.cents, coins: t.coins, ...(f.expiresAt ? { expires_at: f.expiresAt } : {}), label: `Nana sells coins at ${dollars(t.cents)} each` });
  }

  // --- loans -----------------------------------------------------------------
  if (input.lending) {
    const l = input.lending;
    const openOffers = input.loans.filter((x) => x.lender === nana.account && x.status === 'OFFERED');
    if (l.replace) for (const x of openOffers) steps.push({ kind: 'close-loan', id: x.id, label: `Withdraw old loan offer to ${x.borrower_name}` });
    if (!l.borrowers.length) errors.push('Pick at least one person who can borrow.');
    for (let i = 1; i < l.tiers.length; i++) if (l.tiers[i].shareBps > l.tiers[i - 1].shareBps && l.tiers[i].rateBps < l.tiers[i - 1].rateBps)
      warnings.push('A bigger loan usually costs more interest, not less.');
    for (const b of l.borrowers) for (const t of l.tiers) {
      const amount = shareOf(circulation, t.shareBps, scale);
      // Principal back in about four payments, whole coins, never more than the loan.
      const quarter = Math.ceil(amount / 4 / scale) * scale;
      totals.loanCoins += amount;
      steps.push({
        kind: 'loan',
        label: `${t.label} loan offer to ${b.name}`,
        input: { borrower: b.account, amount, rate_bps: t.rateBps, rate_days: 30, payment_days: l.paymentDays, installment: Math.min(amount, quarter), credit: false, memo: `Nana's standing loan · ${t.label}` },
      });
    }
    const live = input.loans.filter((x) => !['PAID', 'DECLINED', 'CANCELLED'].includes(x.status) && !(l.replace && x.lender === nana.account && x.status === 'OFFERED')).length;
    const after = live + l.borrowers.length * l.tiers.length;
    if (after > LOAN_CAPACITY) errors.push(`The board holds ${LOAN_CAPACITY} open loans. This batch would need ${after}. Pick fewer people or fewer loan sizes.`);
    else if (LOAN_CAPACITY - after < LOAN_RESERVE) warnings.push(`Only ${LOAN_CAPACITY - after} loan slots would be left for everyone else.`);
    if (totals.loanCoins > nana.balance)
      warnings.push('If every loan offer were accepted at once, Nana would not have enough coins. Loans are checked when accepted, so the last ones would be refused until Nana has the coins.');
  }

  // --- lottos ----------------------------------------------------------------
  if (input.lotto) {
    const s = input.lotto;
    if (!Number.isInteger(s.count) || s.count < 1 || s.count > 12) errors.push('Make between 1 and 12 lottos.');
    else if (!Number.isSafeInteger(s.firstClose) || s.firstClose <= now) errors.push('Pick a first closing time in the future.');
    else {
      if (s.ticketPrice <= 0) errors.push('A ticket has to cost more than 0.');
      if (s.kind !== 'SIMPLE' && (s.rateBps < 0 || s.rateBps > 10_000)) errors.push('Interest has to be between 0 and 100 percent.');
      const open = input.lottos.filter((x) => x.status !== 'SETTLED').length;
      if (open + s.count > LOTTO_CAPACITY) errors.push(`The board holds ${LOTTO_CAPACITY} lottos that are not finished. ${open} are running, so at most ${Math.max(0, LOTTO_CAPACITY - open)} more fit.`);
      else if (LOTTO_CAPACITY - open - s.count < LOTTO_RESERVE) warnings.push(`Only ${LOTTO_CAPACITY - open - s.count} lotto slots would be left.`);
      warnings.push('Every lotto in the series opens for tickets right away. The server has no later start date yet, so a child can buy a ticket for next December today.');
      for (let i = 0; i < s.count; i++) {
        const closes_at = addMonths(s.firstClose, i);
        const title = `${s.title.trim() || 'Monthly lotto'} · ${MONTH_NAME.format(new Date(closes_at * 1000))}`.slice(0, 80);
        steps.push({ kind: 'lotto', label: `Lotto “${title}”`, terms: { kind: s.kind, title, ticket_price: s.ticketPrice, closes_at, rate_bps: s.kind === 'SIMPLE' ? 0 : s.rateBps } });
      }
      totals.lottos = s.count;
    }
  }

  if (!input.forex && !input.lending && !input.lotto) errors.push('Turn on at least one kind of offer.');
  return { steps, errors, warnings, totals };
}

export function dollars(cents: number): string {
  return `$${(cents / 100).toFixed(2)}`;
}
