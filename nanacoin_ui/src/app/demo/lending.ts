import { Loan, LoanBook, LoanOfferInput, User } from '../api/models';
import { MAX_MONEY } from '../api/money';

export interface LendingHost {
  balance(account: string): number;
  user(account: string): User;
  post(from: string, to: string, amount: number, loan: number, interest: boolean): void;
}
interface Stored extends Loan { remainder: bigint; accruedAt: number; principalDue: number; interestDue: number; attemptedBalance: number }
const DAY = 86400;
const terminal = (loan: Stored) => ['PAID','DECLINED','CANCELLED'].includes(loan.status);

/** Tab-local equivalent of Rust loans. No persistence claims. */
export class DemoLending {
  private loans: Stored[] = [];
  private nextId = 1;
  private blocked = new Set<string>();
  constructor(private readonly host: LendingHost) {}
  cashChanged(accounts: string[], loan: boolean): void { for (const a of accounts) { if (loan) this.blocked.add(a); else this.blocked.delete(a); } }
  private active(account: string): User { const u = this.host.user(account); if (!u || u.status !== 'ACTIVE') throw new Error('An account is disabled.'); return u; }
  private find(id: number): Stored { const l = this.loans.find(l => l.id === id); if (!l) throw new Error('Loan not found.'); return l; }
  private checkFunds(from: string, to: string, amount: number): void {
    this.active(from); this.active(to);
    if (this.host.balance(from) < amount) throw new Error('Insufficient funds.');
    if (!Number.isSafeInteger(this.host.balance(to) + amount)) throw new Error('Balance limit exceeded.');
  }
  private view(l: Stored, now: number): Loan {
    const copy = { ...l };
    let reason = '';
    try {
      this.active(l.borrower); this.active(l.lender);
      if (l.status === 'ACTIVE') this.accrue(copy, now);
      if (l.status === 'ARMED') reason = this.host.balance(l.borrower) !== 0 ? 'Waiting for a zero balance'
        : this.blocked.has(l.borrower) ? 'Credit does not fund loan payments'
        : this.host.balance(l.lender) < l.amount ? 'Waiting for lender funds' : 'Ready for automatic funding';
    } catch (e) { reason = e instanceof Error ? e.message : String(e); }
    const { remainder: _r, accruedAt: _a, principalDue: _p, interestDue: _i, attemptedBalance: _b, ...visible } = copy;
    return { ...visible, overdue: l.principalDue + l.interestDue, waiting_reason: reason };
  }
  book(actor: User, decimals: number, epoch: number, sequence: number, now: number): LoanBook {
    const active = this.loans.filter(l => l.status === 'ACTIVE');
    const outstanding = active.reduce((n,l) => n + BigInt(l.principal), 0n);
    const overdue = active.reduce((n,l) => n + BigInt(l.principalDue + l.interestDue), 0n);
    const weighted = active.reduce((n,l) => n + l.principal * l.rate_bps / 100 * 365 / l.rate_days, 0);
    return { loans: this.loans.filter(l => actor.role === 'nana' || l.borrower === actor.account || l.lender === actor.account).slice().reverse().map(l => this.view(l, now)),
      summary: { outstanding: String(outstanding), overdue: String(overdue), active: active.length, weighted_annual_percent: outstanding ? weighted / Number(outstanding) : null },
      decimals, money_epoch: epoch, sequence };
  }
  offer(actor: User, input: LoanOfferInput, now: number): Loan {
    this.active(actor.account); const borrower = this.active(input.borrower);
    if (actor.account === input.borrower || !Number.isSafeInteger(input.amount) || input.amount <= 0 || input.amount > MAX_MONEY
      || !Number.isSafeInteger(input.installment) || input.installment <= 0 || input.installment > input.amount
      || !Number.isInteger(input.rate_bps) || input.rate_bps < 0 || input.rate_bps > 4294967295
      || ![1,7,30,365].includes(input.rate_days) || ![1,7,30].includes(input.payment_days)
      || new TextEncoder().encode(input.memo).length > 96 || /[\u0000-\u001f\u007f]/.test(input.memo)) throw new Error('Invalid loan terms.');
    if (this.loans.length === 32) { const i = this.loans.findIndex(terminal); if (i < 0) throw new Error('Loan capacity reached.'); this.loans.splice(i,1); }
    const loan: Stored = { ...input, id: this.nextId++, lender: actor.account, lender_name: actor.display_name, borrower_name: borrower.display_name,
      status: 'OFFERED', principal: 0, interest: 0, overdue: 0, next_due_at: 0, created_at: now, updated_at: now,
      waiting_reason: '', remainder: 0n, accruedAt: 0, principalDue: 0, interestDue: 0, attemptedBalance: 0 };
    this.loans.push(loan); return this.view(loan, now);
  }
  private start(l: Stored, now: number): void {
    this.checkFunds(l.lender, l.borrower, l.amount);
    this.host.post(l.lender, l.borrower, l.amount, l.id, false);
    l.status = 'ACTIVE'; l.principal = l.amount; l.accruedAt = now; l.next_due_at = now + l.payment_days * DAY; l.updated_at = now;
  }
  accept(actor: User, id: number, now: number): Loan {
    const l = this.find(id); this.active(actor.account); this.active(l.lender);
    if (actor.account !== l.borrower || l.status !== 'OFFERED') throw new Error('This loan cannot be accepted.');
    if (l.credit) { l.status = 'ARMED'; l.updated_at = now; } else this.start(l, now);
    return this.view(l,now);
  }
  close(actor: User, id: number, now: number): Loan {
    this.active(actor.account); const l = this.find(id);
    if (![l.borrower,l.lender].includes(actor.account) || !['OFFERED','ARMED'].includes(l.status)) throw new Error('This loan cannot be cancelled.');
    l.status = actor.account === l.borrower && l.status === 'OFFERED' ? 'DECLINED' : 'CANCELLED'; l.updated_at = now;
    return this.view(l,now);
  }
  private accrue(l: Stored, now: number): void {
    if (now < l.accruedAt) throw new Error('Clock is unavailable.');
    const denominator = 10000n * BigInt(l.rate_days * DAY);
    const numerator = BigInt(l.principal) * BigInt(l.rate_bps) * BigInt(now - l.accruedAt) + l.remainder;
    const interest = BigInt(l.interest) + numerator / denominator;
    if (interest + BigInt(l.principal) > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error('Interest exceeds the accounting limit.');
    l.interest = Number(interest); l.remainder = numerator % denominator; l.accruedAt = now;
  }
  repay(actor: User, id: number, amount: number, now: number): Loan {
    const l = this.find(id); this.active(actor.account);
    if (l.borrower !== actor.account || !Number.isSafeInteger(amount) || amount <= 0 || amount > MAX_MONEY) throw new Error('Invalid repayment.');
    this.settle(l,now,amount); return this.view(l,now);
  }
  private settle(original: Stored, now: number, amount?: number): void {
    if (original.status !== 'ACTIVE') throw new Error('Loan is not active.');
    this.active(original.borrower); this.active(original.lender);
    const l = { ...original }; this.accrue(l,now);
    if (now >= l.next_due_at) {
      const periods = Math.floor((now - l.next_due_at) / (l.payment_days * DAY)) + 1;
      l.principalDue = Number([BigInt(l.principal), BigInt(l.principalDue) + BigInt(periods) * BigInt(l.installment)].reduce((a,b) => a < b ? a : b));
      l.interestDue = l.interest; l.next_due_at += periods * l.payment_days * DAY;
    }
    const requested = amount === undefined ? Math.min(MAX_MONEY,l.principalDue + l.interestDue) : Math.min(amount,l.principal + l.interest);
    const balance = this.host.balance(l.borrower);
    if (amount !== undefined && balance < requested) throw new Error('Insufficient funds.');
    const paid = Math.min(requested,Math.max(0,balance));
    if (paid > 0) this.checkFunds(l.borrower,l.lender,paid);
    const interest = Math.min(paid,l.interest), principal = paid - interest;
    if (principal) this.host.post(l.borrower,l.lender,principal,l.id,false);
    if (interest) this.host.post(l.borrower,l.lender,interest,l.id,true);
    l.principal -= principal; l.interest -= interest;
    l.principalDue = Math.max(0,l.principalDue-principal); l.interestDue = Math.max(0,l.interestDue-interest);
    l.attemptedBalance = balance-paid; l.updated_at = now;
    if (!l.principal && !l.interest) { l.status = 'PAID'; l.remainder = 0n; l.next_due_at = 0; }
    Object.assign(original,l);
  }
  tick(now: number): number {
    let done = 0;
    for (const l of this.loans.slice().sort((a,b) => (a.next_due_at || a.updated_at) - (b.next_due_at || b.updated_at) || a.id-b.id)) {
      if (done === 4) break;
      try {
        if (l.status === 'ARMED' && this.host.balance(l.borrower) === 0 && !this.blocked.has(l.borrower)) { this.start(l,now); done++; }
        else if (l.status === 'ACTIVE' && (now >= l.next_due_at || (l.principalDue+l.interestDue > 0 && this.host.balance(l.borrower)>0 && this.host.balance(l.borrower)!==l.attemptedBalance))) { this.settle(l,now); done++; }
      } catch { /* Arithmetic/disabled/funding failures leave the loan unchanged. */ }
    }
    return done;
  }
  reform(exponent: number): () => void {
    const factor = 10n ** BigInt(Math.abs(exponent));
    const convert = (n: bigint) => { if (exponent < 0 && n % factor) throw new Error('Reform would lose loan fractions.'); return exponent < 0 ? n/factor : n*factor; };
    const amount = (n: number) => { const v = convert(BigInt(n)); if (v > BigInt(Number.MAX_SAFE_INTEGER) || v < -BigInt(Number.MAX_SAFE_INTEGER)) throw new Error('Reform exceeds amount limit.'); return Number(v); };
    const changed = this.loans.map(l => {
      const denominator = 10000n * BigInt(l.rate_days * DAY), accrued = convert(BigInt(l.interest)*denominator+l.remainder);
      const updated = { ...l, amount: amount(l.amount), installment: amount(l.installment), principal: amount(l.principal),
        principalDue: amount(l.principalDue), interestDue: amount(l.interestDue), attemptedBalance: amount(l.attemptedBalance),
        interest: Number(accrued / denominator), remainder: accrued % denominator };
      if (updated.amount > MAX_MONEY || updated.installment > MAX_MONEY || BigInt(updated.principal) + accrued/denominator > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error('Reform exceeds loan limit.');
      return updated;
    });
    return () => { this.loans = changed; };
  }
}
