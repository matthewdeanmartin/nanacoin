import { Component, DestroyRef, computed, inject, input, resource, signal } from '@angular/core';
import { RouterLink } from '@angular/router';
import { FormsModule } from '@angular/forms';
import { DatePipe } from '@angular/common';
import { Lotto, LottoKind, LottoTerms } from '../api/models';
import { InputError, MAX_MONEY, Money, MoneyPipe, moneyText, parseMoney } from '../api/money';
import { Log } from '../api/log';
import { NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session, reloadOnLedgerChange } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { Toasts } from '../ui/toasts';
import { lottoOutcome } from './account-commitments';

@Component({
  selector: 'app-lotto', imports: [FormsModule, MoneyPipe, DatePipe, RouterLink],
  template: `
    @if (administration()) { <h2>Household lottos</h2> } @else { <h1>Lotto</h1> }
    <p>Every ticket has an equal chance. Ticket money is held safely in the pool. Nana is the house and cannot buy tickets in her own draw.</p>
    <p>Simple lotto pays the whole pool when sales close. Delayed lotto draws and pays the pool plus interest 30 days after sales close. Savings lotto returns everyone's ticket money then, and one winner gets all the pool's interest.</p>
    <p>Interest is a fixed simple rate for those 30 days, rounded down to the smallest currency unit. Nana pays interest from her balance; any shortfall is newly issued coins.</p>
    @if (!session.signedIn()) { <p>Sign in to buy tickets.</p> }
    @else {
      @if (session.isNana() && !administration()) { <p><a routerLink="/nana" [queryParams]="{tab:'lotto-admin'}">Manage lottos in Household</a></p> }
      @if (session.isNana() && administration()) {
        <section class="panel"><h2>Create a lotto</h2>
          <form (ngSubmit)="create()" novalidate>
            <label>Name <input name="title" [(ngModel)]="title" (ngModelChange)="fix('title')" required maxlength="80" [attr.aria-invalid]="!!problems().title" aria-describedby="lotto-title-problem" /></label>
            @if (problems().title; as problem) { <p class="field-problem" id="lotto-title-problem">{{problem}}</p> }
            <label>Kind <select name="kind" [(ngModel)]="kind" (ngModelChange)="fix('rate')"><option value="SIMPLE">Simple: winner takes the pool</option><option value="DELAYED">Delayed: winner takes pool + interest</option><option value="SAVINGS">Savings: everyone gets principal back</option></select></label>
            <label>Ticket price in NC <input name="price" [(ngModel)]="price" (ngModelChange)="fix('price')" inputmode="decimal" required [attr.aria-invalid]="!!problems().price" aria-describedby="lotto-price-hint lotto-price-problem" /></label>
            <p class="field-hint" id="lotto-price-hint">Just the number, like 1 or 2.50.</p>
            @if (problems().price; as problem) { <p class="field-problem" id="lotto-price-problem">{{problem}}</p> }
            <label>Sales close (your local time) <input name="closes" type="datetime-local" [(ngModel)]="closes" (ngModelChange)="fix('closes')" required [attr.aria-invalid]="!!problems().closes" aria-describedby="lotto-closes-hint lotto-closes-problem" /></label>
            <p class="field-hint" id="lotto-closes-hint">Pick a day and a time. After this, nobody can buy tickets.</p>
            @if (problems().closes; as problem) { <p class="field-problem" id="lotto-closes-problem">{{problem}}</p> }
            @if (kind !== 'SIMPLE') {
              <label>Interest for 30 days (%) <input name="rate" [(ngModel)]="rate" (ngModelChange)="fix('rate')" inputmode="decimal" required [attr.aria-invalid]="!!problems().rate" aria-describedby="lotto-rate-hint lotto-rate-problem" /></label>
              <p class="field-hint" id="lotto-rate-hint">{{rateHint()}}</p>
              @if (problems().rate; as problem) { <p class="field-problem" id="lotto-rate-problem">{{problem}}</p> }
            }
            @if (problemCount(); as count) { <p class="form-problems" role="alert">{{count === 1 ? 'One box needs' : count + ' boxes need'}} fixing. Read the red words, fix the box, then press Create lotto again.</p> }
            <button class="btn" [disabled]="busy() || !book.hasValue()">Create lotto</button>
          </form>
        </section>
      }
      @if (book.isLoading()) { <p role="status">Loading lotto…</p> }
      @if (book.error()) { <p role="alert">Could not load lotto. <button class="btn btn--quiet" (click)="book.reload()">Retry</button></p> }
      @for (lotto of book.value()?.lottos ?? []; track lotto.id) {
        <article class="card"><h2>{{lotto.terms.title}}</h2>
          <p>{{label(lotto.terms.kind)}} · {{lotto.status.toLowerCase()}}</p>
          <p>Ticket: {{lotto.terms.ticket_price | nc}} NC · Pool: {{lotto.pool | nc}} NC · {{lotto.tickets}} tickets</p>
          <p>Your tickets: {{lotto.my_tickets}}@if (lotto.tickets) { · Your chance: {{(100 * lotto.my_tickets / lotto.tickets).toFixed(2)}}% }</p>
          <p>Sales close: {{lotto.terms.closes_at * 1000 | date:'medium'}}</p>
          <p>Draw and payments due: {{lotto.due_at * 1000 | date:'medium'}}</p>
          @if (lotto.terms.kind !== 'SIMPLE') { <p>30-day interest: {{lotto.terms.rate_bps / 100}}% · Pool interest: {{lotto.interest | nc}} NC</p> }
          @if (lotto.winner_name) { <p>Winner: <strong>{{lotto.winner_name}}</strong></p> }
          @if (lotto.my_tickets) {
            <section class="your-result" aria-label="Your lotto result">
              <h3>{{lotto.status === 'SETTLED' ? outcome(lotto).label : 'Your entry'}}</h3>
              <p>Ticket cost: {{lotto.my_tickets * lotto.terms.ticket_price | nc}} NC</p>
              @if (lotto.status === 'SETTLED') {
                <p>{{lotto.terms.kind === 'SAVINGS' ? 'Principal returned' : 'Prize paid'}}: {{principal(lotto) | nc}} NC</p>
                <p>Interest paid to you: {{(lotto.winner === session.me()?.account ? lotto.interest : 0) | nc}} NC</p>
                <p><strong>Net {{outcome(lotto).net < 0 ? 'loss' : 'gain'}}: {{outcome(lotto).net | nc}} NC</strong></p>
              } @else if (lotto.terms.kind === 'SAVINGS') {
                <p>Principal due back: {{lotto.my_tickets * lotto.terms.ticket_price | nc}} NC. The winner receives all the pool interest.</p>
              }
            </section>
          }
          @if (lotto.status === 'SETTLED' && !lotto.tickets) { <p>No tickets were sold.</p> }
          @if (lotto.status === 'WAITING' || lotto.status === 'PAYING') { <p>Automatic settlement resumes when the server is online. Payments may pause at an accounting limit.</p> }
          @if (lotto.status === 'OPEN' && lotto.house !== session.me()?.account) {
            <button class="btn" [disabled]="busy()" (click)="buy(lotto)">Buy tickets</button>
          } @else if (lotto.status === 'OPEN') {
            <p>Ticket sales are open to household members. The house cannot buy tickets.</p>
          }
        </article>
      } @empty { @if (!book.isLoading() && !book.error()) { <p>No lottos yet.</p> } }
    }
  `,
  styles: [`.card {margin-block:1rem;} .your-result {border-left:3px solid currentColor;padding-left:1rem;margin-block:1rem;} form {display:grid;gap:.75rem;max-width:32rem;} input,select {max-width:100%;box-sizing:border-box;} [aria-invalid=true] {border-color:var(--warn-ink);outline:2px solid var(--warn-ink);} .field-hint,.field-problem {margin:-.5rem 0 0;font-size:.9rem;} .field-problem,.form-problems {color:var(--warn-ink);font-weight:600;} .form-problems {background:var(--warn-bg);padding:.5rem .75rem;border-radius:8px;margin:0;} .btn {min-height:44px;padding:.6rem 1rem;}`],
})
export class LottoPage {
  readonly administration=input(false);
  protected readonly session = inject(Session);
  private readonly api = inject(NanacoinService);
  private readonly money = inject(Money);
  private readonly dialogs = inject(Dialogs);
  private readonly toasts = inject(Toasts);
  private readonly log = inject(Log);
  protected readonly problems = signal<LottoProblems>({});
  protected readonly problemCount = computed(() => Object.keys(this.problems()).length);
  protected readonly busy = signal(false);
  protected readonly book = resource({ params: () => this.session.me()?.account, loader: () => this.api.lottos() });
  /** Catch up when someone else changes the ledger while this page is open. */
  private readonly followLedger = reloadOnLedgerChange(this.book);
  protected outcome(lotto: Lotto) { return lottoOutcome(lotto,this.session.me()?.account ?? ''); }
  protected principal(lotto: Lotto): number { return lotto.terms.kind === 'SAVINGS' ? lotto.my_tickets*lotto.terms.ticket_price : lotto.winner === this.session.me()?.account ? lotto.pool : 0; }
  protected title = ''; protected kind: LottoKind = 'SIMPLE'; protected price = '1'; protected rate = '1'; protected closes = '';
  private readonly keys = new Map<string,string>();
  constructor() { const timer = setInterval(() => this.book.reload(), 10_000); inject(DestroyRef).onDestroy(() => clearInterval(timer)); }
  protected label(kind: LottoKind): string { return kind === 'SIMPLE' ? 'Simple lotto' : kind === 'DELAYED' ? 'Delayed lotto' : 'Savings lotto'; }
  private async run(identity: string, action: (key: string) => Promise<unknown>): Promise<void> {
    if (this.busy()) return;
    this.busy.set(true);
    const key = this.keys.get(identity) ?? newIdempotencyKey(); this.keys.set(identity,key);
    try { await action(key); this.keys.delete(identity); this.book.reload(); await this.session.refresh(); this.toasts.ok('Lotto updated.'); }
    catch (e) { this.toasts.fromError(e); }
    finally { this.busy.set(false); }
  }
  protected fix(field: LottoField): void {
    if (!this.problems()[field]) return;
    this.problems.update(({ [field]: _gone, ...rest }) => rest);
  }
  protected rateHint(): string { return rateHint(this.rate, this.money.locale); }
  protected async create(): Promise<void> {
    if (this.money.changed()) { this.toasts.fromError(new InputError('NanaCoin money changed size. Reload the page, then try again.')); return; }
    const typed: LottoDraft = { title: this.title, kind: this.kind, price: this.price, closes: this.closes, rate: this.rate };
    const { terms, problems } = checkLotto(typed, this.money.decimals(), this.money.locale);
    this.problems.set(problems);
    if (!terms) {
      // What was typed is not secret, and without it a report of "the form
      // said no" cannot be diagnosed.
      this.log.warn('lotto', 'the create-lotto form has problems', { problems, typed });
      return;
    }
    this.log.info('lotto', 'creating a lotto', { terms, typed });
    await this.run(JSON.stringify(terms), key => this.api.createLotto(terms,key));
  }
  protected async buy(lotto: Lotto): Promise<void> {
    const value = await this.dialogs.prompt({ title: 'Buy tickets', message: `How many tickets at ${this.money.format(lotto.terms.ticket_price)} NC each?`, required: true });
    if (value === null) return;
    const count = Number(value), cost = count * lotto.terms.ticket_price;
    if (!Number.isInteger(count) || count < 1 || count > 4294967295 || !Number.isSafeInteger(cost)) { this.log.warn('lotto', 'ticket count was not usable', { typed: value }); this.toasts.error(`Type how many tickets you want as a whole number, like 1 or 5. You typed "${value}".`); return; }
    const answer = await this.dialogs.confirm({ title: 'Confirm ticket purchase', message: `${count} tickets cost ${this.money.format(cost)} NC.`, detail: [lotto.terms.kind === 'SAVINGS' ? 'Your ticket money is returned 30 days after sales close. One winner receives all the interest.' : 'Only the winner receives the prize. Other ticket buyers lose their ticket money.', 'Ticket purchases are final. Each ticket has an equal chance.'], confirmLabel: 'Buy tickets' });
    if (answer !== null) await this.run(`buy:${lotto.id}:${count}`, key => this.api.buyTickets(lotto.id,count,key));
  }
}

export type LottoField = 'title' | 'price' | 'closes' | 'rate';
export type LottoProblems = Partial<Record<LottoField, string>>;
export interface LottoDraft { title: string; kind: LottoKind; price: string; closes: string; rate: string; }

/** "10", "10%" and "10 %" all mean ten percent. */
function percentText(text: string): string { return text.trim().replace(/\s*%$/, ''); }

/**
 * Checks each box of the create-lotto form on its own, so the person is told
 * which box is wrong, what they typed, and what to type instead. Written for
 * kids and parents: short words and an example in every message.
 */
export function checkLotto(draft: LottoDraft, decimals: number, locale: string, now = Date.now()): { terms?: LottoTerms; problems: LottoProblems } {
  const problems: LottoProblems = {};
  const title = draft.title.trim();
  if (!title) problems.title = 'Give the lotto a name, like "Summer Savings".';

  let ticket_price = 0;
  const price = draft.price.trim().replace(/\s*NC$/i, '');
  if (!price) problems.price = 'Type how much one ticket costs, like 1.';
  else {
    try { ticket_price = parseMoney(price, decimals, locale); }
    catch (e) { problems.price = e instanceof InputError ? e.message : 'Type the ticket price as a plain number, like 1 or 2.50.'; }
    if (!problems.price && ticket_price <= 0) problems.price = 'A ticket has to cost more than 0. Try 1.';
    if (!problems.price && ticket_price > MAX_MONEY) problems.price = 'That price is too big. Try a smaller number.';
  }

  const closes_at = Math.floor(new Date(draft.closes).getTime() / 1000);
  if (!draft.closes || !Number.isSafeInteger(closes_at)) problems.closes = 'Pick the day and the time when ticket sales stop. Fill in both the date and the time.';
  else if (closes_at <= now / 1000) problems.closes = `That time has already passed. You picked ${new Date(closes_at * 1000).toLocaleString(locale)}. Pick a time later than now.`;

  let rate_bps = 0;
  if (draft.kind !== 'SIMPLE') {
    const raw = draft.rate.trim(), rate = percentText(raw);
    if (!rate) problems.rate = 'Type the interest as a percent. For ten percent, type 10.';
    else {
      try { rate_bps = parseMoney(rate, 2, locale); }
      catch { problems.rate = /^\d+[.,]\d{3,}$/.test(rate) ? `Use no more than 2 numbers after the dot, like 2.25. You typed "${raw}".` : `Interest has to be a number from 0 to 100. For ten percent, type 10. You typed "${raw}".`; }
      if (!problems.rate && rate_bps > 10_000) problems.rate = `Interest can be 100 at most. For ten percent, type 10. You typed "${raw}".`;
    }
  }

  if (Object.keys(problems).length) return { problems };
  return { terms: { kind: draft.kind, title, ticket_price, closes_at, rate_bps }, problems };
}

/** Says in plain words what the typed interest rate will do. */
export function rateHint(text: string, locale: string): string {
  const help = 'Type 10 for ten percent. The % sign is optional.';
  let bps: number;
  try { bps = parseMoney(percentText(text), 2, locale); } catch { return help; }
  if (bps > 10_000) return help;
  const pct = moneyText(bps, 2, locale);
  const meaning = `${pct}% means Nana adds ${pct} NC for every 100 NC in the pool.`;
  // "0.10" reads like ten percent to many people, but here it is a tenth of one percent.
  if (bps > 0 && bps < 100) { const guess = moneyText(bps * 100, 2, locale); return `${meaning} Did you mean ${guess}%? Then type ${guess}.`; }
  return meaning;
}
