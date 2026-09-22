import { Component, DestroyRef, inject, resource, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { DatePipe } from '@angular/common';
import { Lotto, LottoKind, LottoTerms } from '../api/models';
import { Money, MoneyPipe, parseMoney } from '../api/money';
import { NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { Toasts } from '../ui/toasts';
import { IS_DEMO } from '../demo/demo';

@Component({
  selector: 'app-lotto', imports: [FormsModule, MoneyPipe, DatePipe],
  template: `
    <h1>Lotto</h1>
    <p>Every ticket has an equal chance. Ticket money is held safely in the pool. Nana is the house and cannot buy tickets in her own draw.</p>
    <p>Simple lotto pays the whole pool when sales close. Delayed lotto draws and pays the pool plus interest 30 days after sales close. Savings lotto returns everyone's ticket money then, and one winner gets all the pool's interest.</p>
    <p>Interest is a fixed simple rate for those 30 days, rounded down to the smallest currency unit. Nana pays interest from her balance; any shortfall is newly issued coins.</p>
    @if (demo) { <p>Lotto is available when connected to the Rust server. The browser demo does not run lotto draws.</p> }
    @else if (!session.signedIn()) { <p>Sign in to buy tickets.</p> }
    @else {
      @if (session.isNana()) {
        <section class="panel"><h2>Create a lotto</h2>
          <form (ngSubmit)="create()">
            <label>Name <input name="title" [(ngModel)]="title" required maxlength="80" /></label>
            <label>Kind <select name="kind" [(ngModel)]="kind"><option value="SIMPLE">Simple: winner takes the pool</option><option value="DELAYED">Delayed: winner takes pool + interest</option><option value="SAVINGS">Savings: everyone gets principal back</option></select></label>
            <label>Ticket price in NC <input name="price" [(ngModel)]="price" inputmode="decimal" required /></label>
            <label>Sales close (your local time) <input name="closes" type="datetime-local" [(ngModel)]="closes" required /></label>
            @if (kind !== 'SIMPLE') { <label>Interest for 30 days (%) <input name="rate" [(ngModel)]="rate" inputmode="decimal" required /></label> }
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
          @if (lotto.status === 'SETTLED' && !lotto.tickets) { <p>No tickets were sold.</p> }
          @if (lotto.status === 'WAITING' || lotto.status === 'PAYING') { <p>Automatic settlement resumes when the server is online. Payments may pause at an accounting limit.</p> }
          @if (lotto.status === 'OPEN' && lotto.house !== session.me()?.account) {
            <button class="btn" [disabled]="busy()" (click)="buy(lotto)">Buy tickets</button>
          }
        </article>
      } @empty { @if (!book.isLoading() && !book.error()) { <p>No lottos yet.</p> } }
    }
  `,
})
export class LottoPage {
  protected readonly session = inject(Session);
  protected readonly demo = IS_DEMO;
  private readonly api = inject(NanacoinService);
  private readonly money = inject(Money);
  private readonly dialogs = inject(Dialogs);
  private readonly toasts = inject(Toasts);
  protected readonly busy = signal(false);
  protected readonly book = resource({ params: () => this.session.signedIn() && !this.demo ? true : undefined, loader: () => this.api.lottos() });
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
  protected async create(): Promise<void> {
    try {
      const terms: LottoTerms = { kind: this.kind, title: this.title.trim(), ticket_price: this.money.parse(this.price), closes_at: Math.floor(new Date(this.closes).getTime()/1000), rate_bps: this.kind === 'SIMPLE' ? 0 : parseMoney(this.rate,2,this.money.locale) };
      if (!terms.title || terms.ticket_price <= 0 || !Number.isSafeInteger(terms.closes_at) || terms.closes_at <= Date.now()/1000 || terms.rate_bps < 0 || terms.rate_bps > 10000) throw new Error('Enter a name, positive ticket price, future closing time, and a rate between 0% and 100%.');
      await this.run(JSON.stringify(terms), key => this.api.createLotto(terms,key));
    } catch (e) { this.toasts.fromError(e); }
  }
  protected async buy(lotto: Lotto): Promise<void> {
    const value = await this.dialogs.prompt({ title: 'Buy tickets', message: `How many tickets at ${this.money.format(lotto.terms.ticket_price)} NC each?`, required: true });
    if (value === null) return;
    const count = Number(value), cost = count * lotto.terms.ticket_price;
    if (!Number.isInteger(count) || count < 1 || count > 4294967295 || !Number.isSafeInteger(cost)) { this.toasts.fromError(new Error('Enter a positive whole number of tickets.')); return; }
    const answer = await this.dialogs.confirm({ title: 'Confirm ticket purchase', message: `${count} tickets cost ${this.money.format(cost)} NC.`, detail: [lotto.terms.kind === 'SAVINGS' ? 'Your ticket money is returned 30 days after sales close. One winner receives all the interest.' : 'Only the winner receives the prize. Other ticket buyers lose their ticket money.', 'Ticket purchases are final. Each ticket has an equal chance.'], confirmLabel: 'Buy tickets' });
    if (answer !== null) await this.run(`buy:${lotto.id}:${count}`, key => this.api.buyTickets(lotto.id,count,key));
  }
}
