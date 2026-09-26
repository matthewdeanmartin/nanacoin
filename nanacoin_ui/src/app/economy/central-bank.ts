// Nana as central bank: where money came from and went, what backs it, and
// what Nana has promised. Pure functions over the retained ledger and the
// open books, so every number on the report can be tested. See
// nanacoin_rs/spec/NANA_AS_CENTRAL_BANK.md.

import { Loan, Lotto, Quote, Transaction } from '../api/models';
import { SYSTEM_ISSUANCE } from './series';

export const USD_ISSUANCE = 'account:usd-issuance';
export const LOTTO_ESCROW = 'account:lotto-escrow';

export interface Flows {
  /** Transactions counted (messages excluded). */
  counted: number;
  from: number | null;
  to: number | null;
  // --- the money supply ---
  issuedToNana: number;
  issuedToMembers: number;
  /** Lotto interest Nana could not cover from her balance, issued new. */
  issuedForLottoInterest: number;
  retired: number;
  /** Reversals of issuing or retiring. Signed: positive adds coins back. */
  corrections: number;
  /** Change in coins in circulation over the window. */
  netIssuance: number;
  // --- dollars ---
  usdRecorded: number;
  // --- open-market operations (Nana on the exchange) ---
  coinsBoughtBack: number;
  usdPaidOut: number;
  coinsSold: number;
  usdTakenIn: number;
  // --- Nana's own coin income and spending ---
  interestReceived: number;
  interestPaid: number;
  lent: number;
  repaid: number;
  boughtFromMembers: number;
  soldToMembers: number;
  paidOut: number;
  receivedOther: number;
}

function empty(): Flows {
  return {
    counted: 0, from: null, to: null,
    issuedToNana: 0, issuedToMembers: 0, issuedForLottoInterest: 0, retired: 0, corrections: 0, netIssuance: 0,
    usdRecorded: 0, coinsBoughtBack: 0, usdPaidOut: 0, coinsSold: 0, usdTakenIn: 0,
    interestReceived: 0, interestPaid: 0, lent: 0, repaid: 0, boughtFromMembers: 0, soldToMembers: 0, paidOut: 0, receivedOther: 0,
  };
}

/**
 * Sorts every retained transaction since `since` into central-bank flows.
 * Reversals carry the original's economic kind with mirrored postings, so
 * they net out of the category they undo; only reversals that touch the money
 * supply are shown separately, as corrections.
 */
export function centralBankFlows(txns: Transaction[], nana: string, since = 0): Flows {
  const f = empty();
  const nanaUsd = `${nana}-usd`;
  for (const t of txns) {
    if (t.kind === 'MESSAGE' || t.created_at < since) continue;
    f.counted++;
    f.from = f.from === null ? t.created_at : Math.min(f.from, t.created_at);
    f.to = f.to === null ? t.created_at : Math.max(f.to, t.created_at);
    const at = (account: string) => t.postings.find((p) => p.account === account)?.amount ?? 0;
    const supply = t.postings.find((p) => p.account === SYSTEM_ISSUANCE);
    const quote = t.reference?.startsWith('quote-');
    // A reversal mirrors its original. Book it against the original's
    // category with a minus sign, so undoing a purchase cancels it rather
    // than showing up as a sale.
    const reversal = t.kind === 'REVERSAL';
    const book = (d: number, inbound: keyof Flows, outbound: keyof Flows) => {
      const original = reversal ? -d : d, size = reversal ? -Math.abs(d) : Math.abs(d);
      (f[original > 0 ? inbound : outbound] as number) += size;
    };

    if (t.postings.some((p) => p.account === USD_ISSUANCE)) {
      f.usdRecorded -= at(USD_ISSUANCE);
      continue;
    }
    if (t.postings.some((p) => p.account.endsWith('-usd'))) {
      if (quote) book(at(nanaUsd), 'usdTakenIn', 'usdPaidOut');
      continue;
    }
    if (supply) {
      const added = -supply.amount;
      f.netIssuance += added;
      if (t.kind === 'REVERSAL') f.corrections += added;
      else if (t.kind === 'RETIRE') f.retired -= added;
      else if (t.reference?.startsWith('lotto-')) f.issuedForLottoInterest += added;
      else if (at(nana) > 0) f.issuedToNana += added;
      else f.issuedToMembers += added;
      continue;
    }
    const d = at(nana);
    if (!d) continue;
    if (quote) { book(d, 'coinsBoughtBack', 'coinsSold'); continue; }
    switch (t.economic_kind) {
      case 'INTEREST': book(d, 'interestReceived', 'interestPaid'); break;
      case 'LOAN_PRINCIPAL': book(d, 'repaid', 'lent'); break;
      case 'LABOR': case 'GOOD': book(d, 'soldToMembers', 'boughtFromMembers'); break;
      default: book(d, 'receivedOther', 'paidOut');
    }
  }
  return f;
}

export interface Position {
  circulation: number;
  nanaCoins: number;
  /** Coins held by everyone except Nana, including lotto pools. */
  publicCoins: number;
  usdReserve: number;
  /** Dollars Nana pays if every open buy offer of hers is taken. */
  buyBackPromise: number;
  /** usdReserve / buyBackPromise; null when Nana promises nothing. */
  reserveRatio: number | null;
  /** Reserve spread over every coin the public holds, in cents per coin. */
  backingCentsPerCoin: number | null;
  bestBid: number | null;
  bestAsk: number | null;
  sellPromiseCoins: number;
  loansOwedToNana: number;
  loansOverdueToNana: number;
  loanOffersOpen: number;
  lottoPools: number;
  lottoInterestPromised: number;
}

export function centralBankPosition(input: {
  circulation: number; scale: number; nana: string; nanaCoins: number; usdReserve: number;
  quotes: Quote[]; loans: Loan[]; lottos: Lotto[];
}): Position {
  const { circulation, scale, nana, nanaCoins, usdReserve } = input;
  const mine = input.quotes.filter((q) => q.maker === nana && q.status === 'OPEN' && q.live !== false);
  const bids = mine.filter((q) => q.side === 'BID'), asks = mine.filter((q) => q.side === 'ASK');
  const buyBackPromise = bids.reduce((n, q) => n + (q.cents ?? Number((BigInt(q.coins) * BigInt(q.cents_per_coin)) / BigInt(scale))), 0);
  const publicCoins = Math.max(0, circulation - nanaCoins);
  const lent = input.loans.filter((l) => l.lender === nana);
  const houseLottos = input.lottos.filter((l) => l.house === nana && l.status !== 'SETTLED');
  return {
    circulation, nanaCoins, publicCoins, usdReserve, buyBackPromise,
    reserveRatio: buyBackPromise > 0 ? usdReserve / buyBackPromise : null,
    backingCentsPerCoin: publicCoins > 0 ? (usdReserve * scale) / publicCoins : null,
    bestBid: bids.length ? Math.max(...bids.map((q) => q.cents_per_coin)) : null,
    bestAsk: asks.length ? Math.min(...asks.map((q) => q.cents_per_coin)) : null,
    sellPromiseCoins: asks.reduce((n, q) => n + q.coins, 0),
    loansOwedToNana: lent.filter((l) => l.status === 'ACTIVE').reduce((n, l) => n + l.principal + l.interest, 0),
    loansOverdueToNana: lent.filter((l) => l.status === 'ACTIVE').reduce((n, l) => n + l.overdue, 0),
    loanOffersOpen: lent.filter((l) => l.status === 'OFFERED' || l.status === 'ARMED').reduce((n, l) => n + l.amount, 0),
    lottoPools: houseLottos.reduce((n, l) => n + l.pool, 0),
    lottoInterestPromised: houseLottos.filter((l) => l.terms.kind !== 'SIMPLE').reduce((n, l) => n + l.interest, 0),
  };
}

/** Money-supply growth over the window, as a fraction of where it started. */
export function supplyGrowth(circulation: number, netIssuance: number): number | null {
  const opening = circulation - netIssuance;
  return opening > 0 ? netIssuance / opening : null;
}
