import { Fulfillment, FulfillmentAction } from '../api/models';
// A NanaCoin ledger, in TypeScript.
//
// This is the demo's server: the same economic rules the Go service enforces,
// reimplemented in the browser so the site can be shown to someone without a
// board, a network, or a Go process anywhere.
//
// # What it copies, and what it deliberately does not
//
// It copies the *rules*: money is an append-only list of double-entry
// transactions, a balance is a fold over postings, issuance comes from a
// distinguished account that may go negative, corrections are mirror
// transactions rather than edits, and every authorisation check the real
// server makes is made here too.
//
// It does not copy the *constraints*. There is no 365-transaction window, no
// 12KiB text arena, no fixed-capacity anything. Those exist because the real
// server runs on a microcontroller with 8MB of PSRAM and a fixed budget; a
// browser tab has no such problem, and pretending otherwise would make the
// demo worse at the one thing it is for.
//
// So: the same answers, none of the scarcity.

import {
  AccountHistory,
  EconomicKind,
  EconomicUnit,
  Listing,
  ListingSide,
  ListingStatus,
  Offer,
  Posting,
  Quote,
  QuoteSide,
  Role,
  Status,
  Thing,
  TradeResult,
  Transaction,
  TransactionKind,
  User,
} from '../api/models';
import { sha256 } from '../api/sha256';
import { DemoLending } from './lending';
import { DemoLotto } from './lotto';
import { MAX_MONEY } from '../api/money';
import { ReformInput, ReformResult } from '../api/models';

/** The one account allowed to go negative; see ledger.SystemIssuance. */
export const SYSTEM_ISSUANCE = 'account:system-issuance';
export const NICKLE_RESERVE = 'account:nickle-reserve';
const digestNickle = (token: string) => Array.from(sha256(new TextEncoder().encode(token)), b => b.toString(16).padStart(2, '0')).join('');

export class DemoError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

interface DemoUser extends User {
  /** Plain text here on purpose: see the note in DemoLedger. */
  password: string;
}

export class DemoLedger {
  decimals = 4;
  moneyEpoch = 0;
  revision = 0;
  private liveClock = false;
  readonly lotto = new DemoLotto({
    balance: account => this.balanceOf(account),
    name: account => this.userByAccount(account)?.display_name ?? account,
    post: (id,memo,postings) => { this.append('TRANSFER','system',memo,postings,{reference:`lotto-${id}`,economic_kind:'OTHER'}); },
  });
  readonly lending = new DemoLending({
    balance: account => this.balanceOf(account),
    user: account => this.userByAccount(account)!,
    post: (from,to,amount,loan,interest) => {
      this.append('TRANSFER', 'system', interest ? 'Loan interest' : 'Loan principal', [
        {account: from,name:'',amount:-amount}, {account:to,name:'',amount}],
        {reference:`loan-${loan}`,economic_kind:interest?'INTEREST':'LOAN_PRINCIPAL'});
    },
  });
  startLive(): void { this.liveClock = true; }
  tickLoans(): void { const now=Math.floor(Date.now()/1000);this.revision += this.lending.tick(now);this.lotto.tick(now); }
  reform(actor: User, input: ReformInput): ReformResult {
    if (actor.role !== 'nana' || actor.status !== 'ACTIVE') throw new DemoError(403,'forbidden','Only Nana can reform currency.');
    if (input.expected_epoch !== this.moneyEpoch || input.expected_sequence !== this.revision) throw new DemoError(409,'conflict','The ledger changed. Preview again.');
    if (!Number.isInteger(input.decimals) || input.decimals<0 || input.decimals>8 || !Number.isInteger(input.power) || Math.abs(input.power)>12 || (input.decimals===this.decimals && input.power===0)) throw new Error('Invalid reform.');
    const exponent = input.decimals-this.decimals-input.power;
    const convert = (value: number, exp = exponent, limit = Number.MAX_SAFE_INTEGER) => {
      const factor = 10n ** BigInt(Math.abs(exp)), n = BigInt(value);
      if (exp<0 && n%factor) throw new DemoError(409,'inexact_reform','The reform would lose fractions.');
      const v = exp<0 ? n/factor : n*factor;
      if (v>BigInt(limit) || v< -BigInt(limit)) throw new Error('Reform exceeds amount limit.');
      return Number(v);
    };
    const changes: (()=>void)[] = [];
    for (const t of this.transactions) for (const p of t.postings) { const v = convert(p.amount,exponent,MAX_MONEY); changes.push(()=>{p.amount=v;}); }
    for (const l of this.listings) { const v=convert(l.price,exponent,MAX_MONEY); changes.push(()=>{l.price=v;}); }
    for (const o of this.offers) { const v=convert(o.amount,exponent,MAX_MONEY); changes.push(()=>{o.amount=v;}); }
    for (const q of this.quotes) { const coins=convert(q.coins,exponent,MAX_MONEY),rate=convert(q.cents_per_coin,input.power,MAX_MONEY); changes.push(()=>{q.coins=coins;q.cents_per_coin=rate;}); }
    for (const v of this.nickles.values()) { const amount=convert(v.amount,exponent,MAX_MONEY);changes.push(()=>{v.amount=amount;}); }
    for (const user of this.users) convert(this.balanceOf(user.account));
    const circulation=convert(-this.balanceOf(SYSTEM_ISSUANCE));
    const loans = this.lending.reform(exponent);
    const lotto = this.lotto.reform(value=>convert(value,exponent,MAX_MONEY));
    if (!input.preview) { for (const change of changes) change(); loans();lotto();this.decimals=input.decimals;this.moneyEpoch++;this.revision++; }
    return {decimals:input.decimals,money_epoch:this.moneyEpoch,sequence:this.revision,circulation,preview:input.preview};
  }
  private users: DemoUser[] = [];
  private listings: Listing[] = [];
  private offers: Offer[] = [];
  private quotes: Quote[] = [];
  private transactions: Transaction[] = [];
  private usdBalances = new Map<string, number>();
  private nextTxn = 1;
  private nextId = 1;
  private nickles = new Map<string, { amount: number; serial: string; issuer: string }>();
  private nextNickle = 1;

  createNickle(actor: DemoUser, amount: number, freshMoney = false): { token: string; amount: number; serial: string } {
    this.requireActive(actor);
    this.requireAmount(amount);
    if (freshMoney) this.requireNana(actor);
    if (this.nickles.size >= 128) throw new DemoError(409, 'capacity', 'Redeem outstanding vouchers before creating more.');
    const token = 'DEMO-NN-' + Array.from(crypto.getRandomValues(new Uint8Array(32)), b => b.toString(16).padStart(2, '0')).join('');
    const digest = digestNickle(token);
    if (this.nickles.has(digest)) throw new DemoError(409, 'collision', 'Please try again.');
    const serial = `NN-${this.nextNickle}`;
    this.append(freshMoney ? 'ISSUE' : 'TRANSFER', actor.id, `Create nana-nickle ${serial}`, [
      { account: freshMoney ? SYSTEM_ISSUANCE : actor.account, name: '', amount: -amount },
      { account: NICKLE_RESERVE, name: '', amount },
    ], { reference: `nickle:${serial}` });
    this.nickles.set(digest, { amount, serial, issuer: actor.id });
    this.nextNickle++;
    return { token, amount, serial };
  }

  redeemNickle(actor: DemoUser, token: string): Transaction {
    this.requireActive(actor);
    if (!/^DEMO-NN-[a-f0-9]{64}$/.test(token)) throw new DemoError(400, 'invalid_voucher', 'Unknown or already redeemed voucher.');
    const digest = digestNickle(token);
    const voucher = this.nickles.get(digest);
    if (!voucher) throw new DemoError(400, 'invalid_voucher', 'Unknown or already redeemed voucher.');
    if (voucher.issuer === actor.id) {
      throw new DemoError(403, 'self_redemption', 'You cannot redeem a voucher you issued. Give it to someone else.');
    }
    // Synchronous check/post/consume: no await between these operations.
    const transaction = this.append('TRANSFER', actor.id, `Redeem nana-nickle ${voucher.serial}`, [
      { account: NICKLE_RESERVE, name: '', amount: -voucher.amount },
      { account: actor.account, name: '', amount: voucher.amount },
    ], { reference: `nickle:${voucher.serial}` });
    this.nickles.delete(digest);
    return transaction;
  }

  household = 'The Demo House';
  provisioned = false;

  /**
   * Passwords are stored in plain text, and that is correct here.
   *
   * The real server uses PBKDF2 because it holds a household's real
   * credentials. This holds four made-up accounts in one browser tab, whose
   * passwords are printed on the screen next to the login box. Hashing them
   * would be theatre: it would not protect anything, and it would suggest the
   * demo is somewhere secrets could be kept.
   */
  private account(userId: string): string {
    return `account-${userId}`;
  }

  // --- reading ---

  status(): Status {
    return {
      decimals: this.decimals, money_epoch: this.moneyEpoch, sequence: this.revision, lending_enabled: true,
      provisioned: this.provisioned,
      household: this.household,
      currency: 'NanaCoin',
      users: this.users.length,
      transactions: this.transactions.length,
      active_listings: this.listings.filter((l) => l.status === 'ACTIVE').length,
      circulation: -this.balanceOf(SYSTEM_ISSUANCE),
      journal_used: this.transactions.length * 48,
      journal_capacity: 0,
      ledger_balanced: this.balanced(),
      logs_enabled: false,
      diag_enabled: false,
    };
  }

  /**
   * Every posting in the book sums to zero, including issuance.
   *
   * The real server checks this on every boot. Here it backs the same banner,
   * and it is the property that would break first if any of the arithmetic
   * below were wrong - which makes it worth computing rather than asserting.
   */
  balanced(): boolean {
    let total = 0;
    for (const t of this.transactions) {
      for (const p of t.postings) total += p.amount;
    }
    return total === 0;
  }

  balanceOf(account: string): number {
    let total = 0;
    for (const t of this.transactions) {
      for (const p of t.postings) {
        if (p.account === account) total += p.amount;
      }
    }
    return total;
  }

  userById(id: string): DemoUser | undefined {
    return this.users.find((u) => u.id === id);
  }

  userByName(username: string): DemoUser | undefined {
    return this.users.find((u) => u.username.toLowerCase() === username.toLowerCase());
  }

  userByAccount(account: string): DemoUser | undefined {
    return this.users.find((u) => u.account === account);
  }

  /** A user as the API returns them: balance only where the caller may see it. */
  view(u: DemoUser, viewer: DemoUser | null): User {
    const maySee = viewer?.role === 'nana' || viewer?.id === u.id;
    return {
      id: u.id,
      username: u.username,
      display_name: u.display_name,
      role: u.role,
      status: u.status,
      account: u.account,
      created_at: u.created_at,
      balance: maySee ? this.balanceOf(u.account) : undefined,
      usd_cents: maySee ? (this.usdBalances.get(u.account) ?? 0) : undefined,
    };
  }

  allUsers(viewer: DemoUser | null): User[] {
    return this.users.map((u) => this.view(u, viewer));
  }

  history(account: string, limit: number): AccountHistory {
    const txns = this.transactions
      .filter((t) => t.postings.some((p) => p.account === account))
      .slice(-limit)
      .reverse();
    return { account, balance: this.balanceOf(account), transactions: this.named(txns) };
  }

  ledger(limit: number): { transactions: Transaction[]; circulation: number } {
    return {
      transactions: this.named(this.transactions.filter(t => t.kind !== 'MESSAGE').slice(-limit).reverse()),
      circulation: -this.balanceOf(SYSTEM_ISSUANCE),
    };
  }

  /** Postings carry the other party's display name, as the real views do. */
  private named(txns: Transaction[]): Transaction[] {
    return txns.map((t) => ({
      ...t,
      postings: t.postings.map((p) => ({
        ...p,
        name:
          p.account === SYSTEM_ISSUANCE
            ? 'Issuance'
            : p.account === NICKLE_RESERVE ? 'Nana-nickle reserve' : (this.userByAccount(p.account)?.display_name ?? p.account),
      })),
      reversed_by: this.transactions.find((r) => r.reverses === t.id)?.id,
    }));
  }

  // --- writing ---

  provision(username: string, displayName: string, password: string, household: string): User {
    if (this.provisioned) {
      throw new DemoError(409, 'already_provisioned', 'This household already exists.');
    }
    this.household = household || this.household;
    const nana = this.addUser(username, displayName, password, 'nana');
    this.provisioned = true;
    return this.view(nana, nana);
  }

  addUser(username: string, displayName: string, password: string, role: Role): DemoUser {
    if (this.userByName(username)) {
      throw new DemoError(409, 'username_taken', `There is already a ${username}.`);
    }
    const id = `user-${this.nextId++}`;
    const u: DemoUser = {
      id,
      username,
      display_name: displayName || username,
      role,
      status: 'ACTIVE',
      account: this.account(id),
      created_at: this.now(),
      password,
    };
    this.users.push(u);
    return u;
  }

  setUserStatus(actor: DemoUser, id: string, status: 'ACTIVE' | 'DISABLED'): User {
    this.requireNana(actor);
    const u = this.userById(id);
    if (!u) throw new DemoError(404, 'not_found', 'No such member.');
    if (u.id === actor.id) {
      throw new DemoError(400, 'bad_request', 'Nana cannot disable herself.');
    }
    u.status = status;
    return this.view(u, actor);
  }

  setUserPassword(actor: DemoUser, id: string, password: string): User {
    const u = this.userById(id);
    if (!u) throw new DemoError(404, 'not_found', 'No such member.');
    if (actor.role !== 'nana' && actor.id !== u.id) {
      throw new DemoError(403, 'forbidden', 'You can only change your own password.');
    }
    if (password.length < 4) {
      throw new DemoError(400, 'bad_request', 'The password must be at least 4 characters.');
    }
    u.password = password;
    return this.view(u, actor);
  }

  /**
   * Appends a transaction, after checking it balances and leaves no ordinary
   * account overdrawn.
   *
   * Validation happens before the append rather than after, for the same
   * reason it does on the real server: a refused transfer must leave no trace
   * at all, or the ledger fills with attempts.
   */
  private append(
    kind: TransactionKind,
    actor: string,
    description: string,
    postings: Posting[],
    opts: {
      allowOverdraft?: boolean;
      reference?: string;
      reverses?: string;
      economic_kind?: EconomicKind;
      thing?: string;
      thing_name?: string;
      quantity_milli?: number;
      unit?: EconomicUnit;
    } = {},
  ): Transaction {
    const sum = postings.reduce((a, p) => a + p.amount, 0);
    if (sum !== 0) {
      throw new DemoError(500, 'unbalanced', 'That transaction does not balance.');
    }

    if (!opts.allowOverdraft) {
      for (const p of postings) {
        if (p.account === SYSTEM_ISSUANCE) continue;
        if (p.amount < 0 && this.balanceOf(p.account) + p.amount < 0) {
          throw new DemoError(400, 'insufficient_funds', 'There are not enough coins.');
        }
      }
    }

    const txn: Transaction = {
      id: `txn-${this.nextTxn++}`,
      kind,
      created_at: this.now(),
      actor,
      description,
      reference: opts.reference,
      reverses: opts.reverses,
      economic_kind: opts.economic_kind,
      thing: opts.thing,
      thing_name: opts.thing_name,
      quantity_milli: opts.quantity_milli,
      unit: opts.unit,
      postings,
    };
    const original = opts.reverses ? this.transactions.find(t=>t.id===opts.reverses) : undefined;
    const user = this.users.find(u=>u.id===actor);
    if (original?.fulfillment) {
      original.fulfillment.status='REVERSED';
      original.fulfillment.updates.push({id:txn.id,at:txn.created_at,actor:user?.account ?? actor,actor_name:user?.display_name ?? actor,status:'REVERSED',reason:''});
      original.fulfillment.updates=original.fulfillment.updates.slice(-4);
    } else if (kind !== 'REVERSAL' && (kind === 'PURCHASE' || opts.economic_kind === 'LABOR' || opts.economic_kind === 'GOOD')) {
      const provider=postings.find(p=>p.amount>0),recipient=postings.find(p=>p.amount<0);
      const listing=this.listings.find(l=>l.id===opts.reference);
      if (provider && recipient) txn.fulfillment={transaction:txn.id,provider:provider.account,recipient:recipient.account,
        provider_name:this.userByAccount(provider.account)?.display_name ?? provider.name,recipient_name:this.userByAccount(recipient.account)?.display_name ?? recipient.name,
        description,kind:listing?.kind==='currency'?'CASH':opts.economic_kind==='LABOR' || listing?.kind==='service'?'WORK':'GOODS',status:'TODO',
        updates:[{id:txn.id,at:txn.created_at,actor:user?.account ?? actor,actor_name:user?.display_name ?? actor,status:'TODO',reason:''}]};
    }
    this.transactions.push(txn);
    this.revision++;
    this.lending.cashChanged(postings.map(p=>p.account), opts.reference?.startsWith('loan-') ?? false);
    return txn;
  }

  transfer(
    actor: DemoUser,
    to: string,
    amount: number,
    memo: string,
    economic?: { economic_kind: EconomicKind; thing?: string; quantity_milli: number; unit: EconomicUnit },
  ): Transaction {
    this.requireActive(actor);
    if (amount === 0) { if (!memo.trim() || new TextEncoder().encode(memo).length > 96) throw new DemoError(400, 'invalid_input', 'Write a message of at most 96 bytes.'); }
    else this.requireAmount(amount);
    if (to === actor.account) {
      throw new DemoError(400, 'self_deal', 'You cannot pay yourself.');
    }
    this.requireUserAccount(to);
    if (economic?.economic_kind === 'LABOR' && this.userByAccount(to)?.role === 'nana') {
      throw new DemoError(403, 'forbidden', 'Nana cannot sell labor.');
    }
    return this.append(amount === 0 ? 'MESSAGE' : 'TRANSFER', actor.id, memo, [
      { account: actor.account, name: '', amount: -amount },
      { account: to, name: '', amount },
    ], economic);
  }

  issue(actor: DemoUser, to: string, amount: number, reason: string): Transaction {
    this.requireNana(actor);
    this.requireAmount(amount);
    this.requireUserAccount(to);
    // Issuance is an ordinary balanced transaction against the one account
    // allowed to go negative, not an exception to the rules.
    return this.append('ISSUE', actor.id, reason, [
      { account: SYSTEM_ISSUANCE, name: '', amount: -amount },
      { account: to, name: '', amount },
    ]);
  }

  retire(actor: DemoUser, from: string, amount: number, reason: string): Transaction {
    this.requireNana(actor);
    this.requireAmount(amount);
    this.requireUserAccount(from);
    return this.append('RETIRE', actor.id, reason, [
      { account: from, name: '', amount: -amount },
      { account: SYSTEM_ISSUANCE, name: '', amount },
    ]);
  }

  // --- foreign exchange ---

  issueUSD(actor: DemoUser, to: string, cents: number, reason: string): Transaction {
    this.requireNana(actor);
    this.requireAmount(cents);
    this.requireUserAccount(to);
    this.usdBalances.set(to, (this.usdBalances.get(to) ?? 0) + cents);
    return {
      id: `usd-txn-${this.nextTxn++}`,
      kind: 'ISSUE',
      created_at: this.now(),
      actor: actor.id,
      description: reason,
      postings: [
        { account: 'account:usd-issuance', name: 'USD issuance', amount: -cents },
        { account: to, name: this.userByAccount(to)?.display_name ?? to, amount: cents },
      ],
    };
  }

  allQuotes(): Quote[] {
    return [...this.quotes].sort((a, b) => {
      if (a.side !== b.side) return a.side === 'ASK' ? -1 : 1;
      return a.side === 'ASK'
        ? a.cents_per_coin - b.cents_per_coin
        : b.cents_per_coin - a.cents_per_coin;
    });
  }

  postQuote(actor: DemoUser, side: QuoteSide, centsPerCoin: number, coins: number): Quote {
    this.requireActive(actor);
    this.requireAmount(centsPerCoin);
    this.requireAmount(coins);
    const total = BigInt(centsPerCoin) * BigInt(coins), scale = 10n ** BigInt(this.decimals);
    if ((side !== 'ASK' && side !== 'BID') || total % scale !== 0n || total / scale <= 0n || total / scale > BigInt(MAX_MONEY)) {
      throw new DemoError(400, 'bad_request', 'That exchange rate or quantity is invalid.');
    }
    const created = this.now();
    const quote: Quote = {
      id: `quote-${this.nextId++}`,
      maker: actor.account,
      maker_name: actor.display_name,
      side,
      cents_per_coin: centsPerCoin,
      coins,
      cents: Number(total / scale),
      status: 'OPEN',
      created_at: created,
      updated_at: created,
      live: true,
    };
    this.quotes.push(quote);
    return quote;
  }

  takeQuote(actor: DemoUser, id: string): TradeResult {
    this.requireActive(actor);
    const quote = this.quotes.find((candidate) => candidate.id === id);
    if (!quote) throw new DemoError(404, 'not_found', 'No such exchange rate.');
    if (!quote.live || quote.status !== 'OPEN') throw new DemoError(409, 'quote_closed', 'That rate is no longer open.');
    if (quote.maker === actor.account) throw new DemoError(400, 'self_deal', 'You cannot take your own rate.');

    const coinSeller = quote.side === 'ASK' ? quote.maker : actor.account;
    const coinBuyer = quote.side === 'ASK' ? actor.account : quote.maker;
    const dollarBuyer = coinSeller;
    const dollarSeller = coinBuyer;
    if (this.balanceOf(coinSeller) < quote.coins) throw new DemoError(400, 'insufficient_funds', 'The coin seller no longer has enough coins.');
    if ((this.usdBalances.get(dollarSeller) ?? 0) < quote.cents) throw new DemoError(400, 'insufficient_funds', 'The buyer no longer has enough dollars.');

    this.usdBalances.set(dollarSeller, (this.usdBalances.get(dollarSeller) ?? 0) - quote.cents);
    this.usdBalances.set(dollarBuyer, (this.usdBalances.get(dollarBuyer) ?? 0) + quote.cents);
    const coinTransaction = this.append('TRANSFER', actor.id, 'Exchange', [
      { account: coinSeller, name: '', amount: -quote.coins },
      { account: coinBuyer, name: '', amount: quote.coins },
    ], { reference: quote.id });
    const cashTransaction: Transaction = {
      id: `usd-txn-${this.nextTxn++}`,
      kind: 'TRANSFER',
      created_at: coinTransaction.created_at,
      actor: actor.id,
      description: 'Exchange',
      reference: quote.id,
      postings: [
        { account: dollarSeller, name: this.userByAccount(dollarSeller)?.display_name ?? dollarSeller, amount: -quote.cents },
        { account: dollarBuyer, name: this.userByAccount(dollarBuyer)?.display_name ?? dollarBuyer, amount: quote.cents },
      ],
    };
    quote.status = 'FILLED';
    quote.live = false;
    quote.taker = actor.account;
    quote.taker_name = actor.display_name;
    quote.updated_at = coinTransaction.created_at;
    quote.coin_tx = coinTransaction.id;
    quote.cash_tx = cashTransaction.id;
    return { quote, coin_transaction: this.named([coinTransaction])[0], cash_transaction: cashTransaction };
  }

  cancelQuote(actor: DemoUser, id: string): Quote {
    const quote = this.quotes.find((candidate) => candidate.id === id);
    if (!quote) throw new DemoError(404, 'not_found', 'No such exchange rate.');
    if (quote.maker !== actor.account && actor.role !== 'nana') throw new DemoError(403, 'forbidden', 'That rate is not yours to withdraw.');
    if (quote.status !== 'OPEN') throw new DemoError(409, 'quote_closed', 'That rate is no longer open.');
    quote.status = 'CANCELLED';
    quote.live = false;
    quote.updated_at = this.now();
    return quote;
  }

  /**
   * Undoes a transaction by appending its mirror image.
   *
   * Overdraft is allowed here: if the money has already been spent, the
   * correction still happens and the balance goes negative where the
   * household can see it. Blocking it would leave the wrong version of events
   * as the permanent record.
   */
  reverse(actor: DemoUser, id: string, reason: string): Transaction {
    const original = this.transactions.find((t) => t.id === id);
    if (original?.kind === 'MESSAGE' || original?.reference?.startsWith('nickle:') || original?.reference?.startsWith('loan-') || original?.reference?.startsWith('lotto-')) {
      throw new DemoError(409, 'voucher_transaction', 'Bearer voucher transfers cannot be reversed independently of their voucher.');
    }
    if (!original) throw new DemoError(404, 'not_found', 'No such transaction.');
    const received = original.postings.find((posting) => posting.amount > 0)?.account;
    const paid = original.postings.find((posting) => posting.amount < 0)?.account;
    const memberRefund = actor.role !== 'nana';
    if (memberRefund && (received !== actor.account || paid === SYSTEM_ISSUANCE || original.reference?.startsWith('quote-'))) {
      throw new DemoError(403, 'forbidden', 'You can only refund money you received.');
    }
    if (this.transactions.some((t) => t.reverses === id)) {
      throw new DemoError(409, 'already_reversed', 'That has already been reversed.');
    }
    if (original.kind === 'REVERSAL') {
      throw new DemoError(400, 'bad_request', 'A correction cannot itself be reversed.');
    }

    return this.append(
      'REVERSAL',
      actor.id,
      reason,
      original.postings.map((p) => ({ ...p, amount: -p.amount })),
      {
        allowOverdraft: !memberRefund,
        reverses: id,
        economic_kind: original.economic_kind,
        thing: original.thing,
        thing_name: original.thing_name,
        quantity_milli: original.quantity_milli,
        unit: original.unit,
      },
    );
  }

  fulfillments(actor: DemoUser): {fulfillments: Fulfillment[]} {
    this.requireActive(actor);
    return {fulfillments:this.transactions.flatMap(t=>t.fulfillment && (actor.role==='nana' || t.fulfillment.provider===actor.account || t.fulfillment.recipient===actor.account)?[t.fulfillment]:[])};
  }
  setFulfillment(actor: DemoUser, id: string, action: FulfillmentAction, reason: string): Fulfillment {
    this.requireActive(actor);
    const f=this.transactions.find(t=>t.id===id)?.fulfillment;
    if (!f) throw new DemoError(404,'not_found','No delivery for this transaction.');
    if (!['COMPLETE','DISPUTE','WITHDRAW_DISPUTE'].includes(action) || typeof reason!=='string' || /[\x00-\x1f\x7f]/.test(reason) || new TextEncoder().encode(reason).length>96 || (action==='DISPUTE' && !reason.trim())) throw new DemoError(400,'invalid_input','Enter a dispute reason of at most 96 bytes.');
    if (actor.account!==(action==='COMPLETE'?f.provider:f.recipient)) throw new DemoError(403,'forbidden','Only the responsible participant can do that.');
    if (!(action==='COMPLETE'?f.status==='TODO':action==='DISPUTE'?f.status==='TODO'||f.status==='DONE':f.status==='DISPUTED')) throw new DemoError(409,'conflict','The fulfillment status has changed.');
    f.status=action==='DISPUTE'?'DISPUTED':'DONE';
    f.updates.push({id:`fulfillment-${++this.revision}`,at:this.now(),actor:actor.account,actor_name:actor.display_name,status:f.status,reason});
    f.updates=f.updates.slice(-4);
    return f;
  }

  // --- marketplace ---

  createListing(
    actor: DemoUser,
    input: {
      title: string;
      description: string;
      price: number;
      side?: ListingSide;
      kind?: string;
      currency?: string;
      minor_units?: number;
      economic_kind?: EconomicKind;
      thing?: string;
      quantity_milli?: number;
      unit?: EconomicUnit;
      standard?: boolean;
    },
  ): Listing {
    this.requireActive(actor);
    this.requireAmount(input.price);
    if (!input.title.trim()) {
      throw new DemoError(400, 'bad_request', 'A listing needs a title.');
    }
    if (input.side !== 'BUY' && input.economic_kind === 'LABOR' && actor.role === 'nana') {
      throw new DemoError(403, 'forbidden', 'Nana cannot sell labor.');
    }
    const l: Listing = {
      id: `listing-${this.nextId++}`,
      seller: actor.account,
      seller_name: actor.display_name,
      title: input.title.trim(),
      description: input.description.trim(),
      price: input.price,
      status: 'ACTIVE',
      created_at: this.now(),
      updated_at: this.now(),
      side: input.side ?? 'SELL',
      kind: input.kind,
      currency: input.currency,
      minor_units: input.minor_units,
      economic_kind: input.economic_kind,
      thing: input.thing ?? (input.economic_kind ? `thing-demo-${input.title.trim().toLocaleLowerCase()}` : undefined),
      quantity_milli: input.quantity_milli,
      unit: input.unit,
      standard: input.standard,
    };
    this.listings.push(l);
    return l;
  }

  allListings(status?: ListingStatus): Listing[] {
    const out = status ? this.listings.filter((l) => l.status === status) : [...this.listings];
    return out.sort((a, b) => b.created_at - a.created_at);
  }

  allThings(): Thing[] {
    const things = new Map<string, Thing>();
    for (const listing of this.listings) {
      if (!listing.thing || !listing.economic_kind || !listing.unit) continue;
      const previous = things.get(listing.thing);
      things.set(listing.thing, {
        id: listing.thing,
        name: listing.title,
        economic_kind: listing.economic_kind,
        unit: listing.unit,
        standard: Boolean(previous?.standard || listing.standard),
        updated_at: Math.max(previous?.updated_at ?? 0, listing.updated_at),
      });
    }
    return [...things.values()].sort((a, b) => b.updated_at - a.updated_at);
  }

  listingById(id: string): Listing {
    const l = this.listings.find((x) => x.id === id);
    if (!l) throw new DemoError(404, 'not_found', 'No such listing.');
    return l;
  }

  cancelListing(actor: DemoUser, id: string): Listing {
    const l = this.listingById(id);
    if (l.seller !== actor.account && actor.role !== 'nana') {
      throw new DemoError(403, 'forbidden', 'That is not yours to cancel.');
    }
    if (l.status !== 'ACTIVE') {
      throw new DemoError(409, 'listing_closed', 'That listing is already closed.');
    }
    l.status = 'CANCELLED';
    l.updated_at = this.now();
    return l;
  }

  /** Paying and closing the listing are one operation, as on the real server. */
  purchase(actor: DemoUser, id: string): { listing: Listing; transaction: Transaction } {
    this.requireActive(actor);
    const l = this.listingById(id);
    if (l.status !== 'ACTIVE') {
      throw new DemoError(409, 'listing_closed', 'That listing is already closed.');
    }
    if (l.seller === actor.account) {
      throw new DemoError(400, 'self_deal', 'You cannot buy your own listing.');
    }
    if (l.economic_kind === 'LABOR' && this.userByAccount(l.seller)?.role === 'nana') {
      throw new DemoError(403, 'forbidden', 'Nana cannot sell labor.');
    }

    const txn = this.append(
      'PURCHASE',
      actor.id,
      l.title,
      [
        { account: actor.account, name: '', amount: -l.price },
        { account: l.seller, name: '', amount: l.price },
      ],
      {
        reference: l.id,
        economic_kind: l.economic_kind,
        thing: l.thing,
        thing_name: l.title,
        quantity_milli: l.quantity_milli,
        unit: l.unit,
      },
    );

    l.status = 'SOLD';
    l.buyer = actor.account;
    l.buyer_name = actor.display_name;
    l.sold_tx = txn.id;
    l.updated_at = this.now();
    return { listing: l, transaction: txn };
  }

  // --- offers ---

  makeOffer(actor: DemoUser, listingId: string, amount: number, message: string): Offer {
    this.requireActive(actor);
    this.requireAmount(amount);
    const l = this.listingById(listingId);
    if (l.status !== 'ACTIVE') {
      throw new DemoError(409, 'listing_closed', 'That listing is already closed.');
    }
    if (l.seller === actor.account) {
      throw new DemoError(400, 'self_deal', 'You cannot offer on your own listing.');
    }
    if (this.offers.some((offer) =>
      offer.listing === listingId && offer.offerer === actor.account && offer.status === 'OPEN')) {
      throw new DemoError(409, 'conflict', 'You already have an open offer on this listing.');
    }

    const offer: Offer = {
      id: `offer-${this.nextId++}`,
      listing: l.id,
      listing_title: l.title,
      listing_owner: l.seller,
      listing_owner_name: this.userByAccount(l.seller)?.display_name ?? l.seller,
      listing_side: l.side ?? 'SELL',
      offerer: actor.account,
      offerer_name: actor.display_name,
      amount,
      message,
      status: 'OPEN',
      created_at: this.now(),
      updated_at: this.now(),
    };
    this.offers.push(offer);
    return offer;
  }

  /** Offers the caller made, plus any on listings they own. */
  offersFor(actor: DemoUser): Offer[] {
    const mine = new Set(
      this.listings.filter((l) => l.seller === actor.account).map((l) => l.id),
    );
    return this.offers
      .filter((o) => o.offerer === actor.account || mine.has(o.listing))
      .sort((a, b) => b.created_at - a.created_at);
  }

  offerById(id: string): Offer {
    const o = this.offers.find((x) => x.id === id);
    if (!o) throw new DemoError(404, 'not_found', 'No such offer.');
    return o;
  }

  /**
   * Accepting is what moves the money - the step the marketplace was missing.
   *
   * Which way the coins go depends on the listing's side. On a SELL the
   * offerer is buying, so they pay. On a BUY - a want-ad - the poster is
   * paying someone to do the thing, so the money goes the other way.
   */
  acceptOffer(
    actor: DemoUser,
    id: string,
  ): { offer: Offer; transaction: Transaction; listing: Listing } {
    const offer = this.offerById(id);
    const listing = this.listingById(offer.listing);

    if (listing.seller !== actor.account) {
      throw new DemoError(403, 'forbidden', 'Only the person who posted it can accept.');
    }
    if (offer.status !== 'OPEN') {
      throw new DemoError(409, 'offer_closed', 'That offer is no longer open.');
    }

    const wantAd = listing.side === 'BUY';
    const payer = wantAd ? listing.seller : offer.offerer;
    const payee = wantAd ? offer.offerer : listing.seller;
    if (listing.economic_kind === 'LABOR' && this.userByAccount(payee)?.role === 'nana') {
      throw new DemoError(403, 'forbidden', 'Nana cannot sell labor.');
    }

    const txn = this.append(
      'PURCHASE',
      actor.id,
      listing.title,
      [
        { account: payer, name: '', amount: -offer.amount },
        { account: payee, name: '', amount: offer.amount },
      ],
      {
        reference: listing.id,
        economic_kind: listing.economic_kind,
        thing: listing.thing,
        thing_name: listing.title,
        quantity_milli: listing.quantity_milli,
        unit: listing.unit,
      },
    );

    offer.status = 'ACCEPTED';
    offer.settled_tx = txn.id;
    offer.updated_at = this.now();

    listing.status = 'SOLD';
    listing.buyer = wantAd ? listing.seller : offer.offerer;
    listing.buyer_name = this.userByAccount(listing.buyer)?.display_name;
    listing.sold_tx = txn.id;
    listing.updated_at = this.now();

    // Every other offer on a sold listing is dead; saying so beats leaving
    // them open against something nobody can buy.
    for (const other of this.offers) {
      if (other.listing === listing.id && other.status === 'OPEN' && other.id !== offer.id) {
        other.status = 'NOT_SELECTED';
        other.updated_at = this.now();
      }
    }

    return { offer, transaction: txn, listing };
  }

  declineOffer(actor: DemoUser, id: string): Offer {
    const offer = this.offerById(id);
    const listing = this.listingById(offer.listing);
    if (listing.seller !== actor.account) {
      throw new DemoError(403, 'forbidden', 'Only the person who posted it can decline.');
    }
    if (offer.status !== 'OPEN') {
      throw new DemoError(409, 'offer_closed', 'That offer is no longer open.');
    }
    offer.status = 'DECLINED';
    offer.updated_at = this.now();
    return offer;
  }

  withdrawOffer(actor: DemoUser, id: string): Offer {
    const offer = this.offerById(id);
    if (offer.offerer !== actor.account) {
      throw new DemoError(403, 'forbidden', 'That is not your offer.');
    }
    if (offer.status !== 'OPEN') {
      throw new DemoError(409, 'offer_closed', 'That offer is no longer open.');
    }
    offer.status = 'WITHDRAWN';
    offer.updated_at = this.now();
    return offer;
  }

  // --- guards, matching the server's ---

  private requireNana(u: DemoUser): void {
    if (u.role !== 'nana') {
      throw new DemoError(403, 'forbidden', 'Only Nana can do that.');
    }
  }

  private requireActive(u: DemoUser): void {
    if (u.status !== 'ACTIVE') {
      throw new DemoError(403, 'disabled', 'That account is disabled.');
    }
  }

  private requireAmount(n: number): void {
    if (!Number.isSafeInteger(n) || n <= 0) {
      throw new DemoError(400, 'bad_request', 'Amounts must be positive integer minor units.');
    }
    if (n > MAX_MONEY) {
      throw new DemoError(400, 'bad_request', 'That amount exceeds the transaction limit.');
    }
  }

  private requireUserAccount(account: string): void {
    if (account === SYSTEM_ISSUANCE) {
      throw new DemoError(400, 'system_account', 'That is not a spendable account.');
    }
    const owner = this.userByAccount(account);
    if (!owner) throw new DemoError(404, 'not_found', 'No such account.');
    if (owner.status !== 'ACTIVE') {
      throw new DemoError(403, 'disabled', 'That account is disabled.');
    }
  }

  /**
   * Seconds, like the real server, and advanced by one on every call.
   *
   * Monotonic rather than Date.now(): a seeded history needs transactions in a
   * believable order, and several landing in the same second is exactly the
   * case that made the economy charts collapse onto one point.
   */
  private clock = Math.floor(Date.now() / 1000) - 60 * 60 * 24 * 395;
  private now(): number {
    if (this.liveClock) return Math.floor(Date.now()/1000);
    return (this.clock += 1);
  }

  /** Moves the clock forward, so a seeded history spreads over real days. */
  advance(seconds: number): void {
    this.clock += seconds;
  }
}
