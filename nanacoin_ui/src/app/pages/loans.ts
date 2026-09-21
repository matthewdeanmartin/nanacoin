import { Component, DestroyRef, inject, resource, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { DatePipe } from '@angular/common';
import { Loan, LoanOfferInput } from '../api/models';
import { Money, MoneyPipe, parseMoney } from '../api/money';
import { NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { Toasts } from '../ui/toasts';

@Component({
  selector: 'app-loans', imports: [FormsModule, MoneyPipe, DatePipe],
  template: `
    <h1>Loans & credit</h1>
    <p>Lend coins you own. The borrower accepts the terms before money moves. Nana follows the same funding rule.</p>
    @if (!session.signedIn()) { <p>Sign in to view or offer loans.</p> }
    @else if (book.error()) { <p role="alert">Could not load loans. <button class="btn btn--quiet" (click)="book.reload()">Retry</button></p> }
    @else {
      <section class="panel">
        <h2>Offer a loan</h2>
        <form (ngSubmit)="offer()">
          <label>Borrower <select name="borrower" [(ngModel)]="borrower" required><option value="">Choose someone</option>@for (user of session.recipients(); track user.account) { <option [value]="user.account">{{user.display_name}}</option> }</select></label>
          <label>Amount in NC <input name="amount" inputmode="decimal" [(ngModel)]="amount" required /></label>
          <label>Interest rate (%) <input name="rate" inputmode="decimal" [(ngModel)]="rate" required /></label>
          <label>Rate period <select name="rateDays" [(ngModel)]="rateDays"><option [ngValue]="1">Day</option><option [ngValue]="7">Week</option><option [ngValue]="30">30 days</option><option [ngValue]="365">Year (365 days)</option></select></label>
          <label>Principal per payment in NC <input name="installment" inputmode="decimal" [(ngModel)]="installment" required /></label>
          <label>Payment frequency <select name="paymentDays" [(ngModel)]="paymentDays"><option [ngValue]="1">Daily</option><option [ngValue]="7">Weekly</option><option [ngValue]="30">Every 30 days</option></select></label>
          <label><input type="checkbox" name="credit" [(ngModel)]="credit" /> Draw once when the borrower's balance reaches exactly zero</label>
          <label>Note <input name="memo" [(ngModel)]="memo" maxlength="96" /></label>
          <p class="muted small">Simple interest on outstanding principal, collected in addition to the principal installment. No negative rates, compounding, or late fees. A credit offer reserves no funds; it waits if the lender cannot fund it.</p>
          <button class="btn" type="submit" [disabled]="!!busy() || !borrower">Offer {{credit ? 'credit' : 'loan'}}</button>
        </form>
      </section>
      @if (book.isLoading()) { <p role="status">Loading loans…</p> }
      @for (loan of book.value()?.loans ?? []; track loan.id) {
        <article class="card">
          <h2>{{loan.lender_name}} → {{loan.borrower_name}}</h2>
          <p><strong>{{loan.amount | nc}} NC</strong> · {{loan.rate_bps / 100}}% per {{period(loan.rate_days)}} · {{loan.status.toLocaleLowerCase()}}</p>
          <p>{{loan.installment | nc}} NC principal + accrued interest every {{loan.payment_days}} days{{loan.credit ? ' · credit at zero' : ''}}.</p>
          @if (loan.memo) { <p>{{loan.memo}}</p> }
          @if (loan.status === 'ACTIVE') {
            <p>Principal: <strong>{{loan.principal | nc}} NC</strong> · Interest: {{loan.interest | nc}} NC · Overdue: {{loan.overdue | nc}} NC</p>
            <p>Next automatic payment: {{loan.next_due_at * 1000 | date:'medium'}}</p>
          }
          @if (loan.waiting_reason) { <p class="muted">{{loan.waiting_reason}}</p> }
          @if (loan.status === 'OFFERED' && loan.borrower === session.me()?.account) {
            <button class="btn" [disabled]="!!busy()" (click)="accept(loan)">Review & accept</button>
          }
          @if ((loan.status === 'OFFERED' || loan.status === 'ARMED') && (loan.borrower === session.me()?.account || loan.lender === session.me()?.account)) {
            <button class="btn btn--quiet" [disabled]="!!busy()" (click)="close(loan)">{{loan.status === 'ARMED' ? 'Cancel undrawn credit' : loan.borrower === session.me()?.account ? 'Decline' : 'Withdraw'}}</button>
          }
          @if (loan.status === 'ACTIVE' && loan.borrower === session.me()?.account) {
            <button class="btn" [disabled]="!!busy()" (click)="repay(loan)">Make a payment</button>
          }
        </article>
      } @empty { @if (!book.isLoading()) { <p>No loans yet.</p> } }
    }
  `,
})
export class LoansPage {
  protected readonly session = inject(Session);
  protected readonly money = inject(Money);
  private readonly api = inject(NanacoinService);
  private readonly dialogs = inject(Dialogs);
  private readonly toasts = inject(Toasts);
  protected readonly busy = signal('');
  protected readonly book = resource({ loader: () => this.api.loans() });
  protected borrower = ''; protected amount = '10'; protected rate = '5';
  protected rateDays = 365; protected paymentDays = 7; protected installment = '1';
  protected credit = false; protected memo = '';
  private readonly keys = new Map<string, string>();
  constructor() {
    const timer = setInterval(() => this.book.reload(), 10_000);
    inject(DestroyRef).onDestroy(() => clearInterval(timer));
  }
  protected period(days: number): string { return days === 1 ? 'day' : days === 7 ? 'week' : days === 365 ? 'year (365 days)' : `${days} days`; }
  private async run(identity: string, action: (key: string) => Promise<unknown>): Promise<void> {
    if (this.busy()) return;
    this.busy.set(identity);
    const key = this.keys.get(identity) ?? newIdempotencyKey(); this.keys.set(identity, key);
    try { await action(key); this.keys.delete(identity); this.book.reload(); await this.session.refresh(); this.toasts.ok('Loan updated.'); }
    catch (e) { this.toasts.fromError(e); }
    finally { this.busy.set(''); }
  }
  protected async offer(): Promise<void> {
    try {
      const input: LoanOfferInput = { borrower: this.borrower, amount: this.money.parse(this.amount),
        rate_bps: parseMoney(this.rate, 2, this.money.locale), rate_days: this.rateDays, payment_days: this.paymentDays,
        installment: this.money.parse(this.installment), credit: this.credit, memo: this.memo.trim() };
      if (input.amount <= 0 || input.installment <= 0 || input.installment > input.amount || input.rate_bps > 4_294_967_295) throw new Error('Check the positive amount, principal installment, and nonnegative rate.');
      await this.run(JSON.stringify(input), key => this.api.offerLoan(input, key));
    } catch (e) { this.toasts.fromError(e); }
  }
  protected async accept(loan: Loan): Promise<void> {
    const answer = await this.dialogs.confirm({ title: 'Accept automatic loan payments?',
      message: `${this.money.format(loan.amount)} NC from ${loan.lender_name}, at ${loan.rate_bps / 100}% per ${this.period(loan.rate_days)}.`,
      detail: [`Every ${loan.payment_days} days: ${this.money.format(loan.installment)} NC principal plus accrued simple interest.`,
        'Payments use your available coins. Unpaid amounts stay overdue; no overdraft, compounding, or late fees.',
        'Early repayment is allowed. The final fraction below the smallest currency unit is waived.',
        loan.credit ? 'Draws once at zero, only if the lender has the funds. Either party can cancel before funding. Loan payments never trigger another automatic loan.' : 'Accepting transfers existing coins from the lender now.'], confirmLabel: 'Accept & authorize payments' });
    if (answer !== null) await this.run(`accept:${loan.id}`, key => this.api.acceptLoan(loan.id, key));
  }
  protected async close(loan: Loan): Promise<void> { await this.run(`close:${loan.id}`, key => this.api.closeLoan(loan.id, key)); }
  protected async repay(loan: Loan): Promise<void> {
    const value = await this.dialogs.prompt({ title: 'Repay loan', message: 'Amount in NC. Interest is paid first; the rest reduces principal.', required: true });
    if (value === null) return;
    try { const amount = this.money.parse(value); if (amount <= 0) throw new Error('Enter a positive amount.'); await this.run(`repay:${loan.id}:${amount}`, key => this.api.repayLoan(loan.id, amount, key)); }
    catch (e) { this.toasts.fromError(e); }
  }
}
