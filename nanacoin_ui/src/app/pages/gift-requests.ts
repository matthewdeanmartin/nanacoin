import { Component, computed, inject, resource, signal } from '@angular/core';
import { DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { GiftRequest } from '../api/models';
import { Money, MoneyPipe } from '../api/money';
import { NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session, reloadOnLedgerChange } from '../api/session';
import { Toasts } from '../ui/toasts';
import { Dialogs } from '../ui/dialog';

export type GiftRequestState = 'open' | 'ended' | 'closed';

/** Deadlines are Unix seconds; a request past its deadline refuses gifts even before its owner closes it. */
export function requestState(r: GiftRequest, nowSeconds: number): GiftRequestState {
  return r.closed ? 'closed' : r.deadline !== null && r.deadline <= nowSeconds ? 'ended' : 'open';
}

/** Local end of the chosen day, so "until Friday" includes Friday. */
export function deadlineFromDate(date: string): number | null {
  if (!date) return null;
  const [year, month, day] = date.split('-').map(Number);
  return Math.floor(new Date(year, month - 1, day, 23, 59, 59).getTime() / 1000);
}

const bytes = (text: string) => new TextEncoder().encode(text).length;

/**
 * Gift requests ("cyberbegging"): ask the household for gifts toward something.
 * Its own screen because it is expected to grow Patreon- and GoFundMe-like
 * features (recurring supporters, updates, stretch goals).
 */
@Component({
  selector: 'app-gift-requests', imports: [DatePipe, FormsModule, MoneyPipe],
  template: `
    <h1>Gift Requests</h1>
    <p class="muted">Ask the household to chip in for something, or give toward someone else's request. Gifts arrive immediately; there is no escrow and no refund when a target is missed. A request can pass its target.</p>
    @if (!session.signedIn()) { <p>Sign in to see gift requests.</p> }
    @else {
      <div class="tabs" role="tablist" aria-label="Gift request views">
        @for (v of views; track v.id) {
          <button class="tab" type="button" role="tab" [class.tab--active]="view() === v.id" [attr.aria-selected]="view() === v.id" (click)="view.set(v.id)">{{ v.label }} ({{ count(v.id) }})</button>
        }
      </div>
      @if (book.isLoading() && !book.hasValue()) { <p role="status">Loading gift requests…</p> }
      @if (book.error()) { <p role="alert">Could not load gift requests. <button class="btn btn--quiet btn--small" (click)="book.reload()">Retry</button></p> }

      @if (view() === 'new') {
        <form class="panel" (ngSubmit)="create()">
          <h2>Ask for a gift</h2>
          <label>What is it for? <input name="title" [(ngModel)]="title" required maxlength="80" placeholder="Help me buy watercolor paints" /></label>
          <label>Details <input name="description" [(ngModel)]="description" maxlength="96" placeholder="A gift toward my next art project" /></label>
          <label>Target (optional) <input name="target" type="text" inputmode="decimal" [(ngModel)]="target" placeholder="No target" /></label>
          <label>Last day to give (optional) <input name="deadline" type="date" [(ngModel)]="deadline" /></label>
          <button class="btn" type="submit" [disabled]="busy()">{{ busy() ? 'Posting…' : 'Post request' }}</button>
          <p class="muted small">The household can hold 32 requests in total, including closed ones.</p>
        </form>
      } @else {
        <div class="cards gift-requests">
          @for (r of visible(); track r.id) {
            <article class="card" [class.card--closed]="state(r) !== 'open'" [attr.aria-label]="r.title">
              <h3>{{ r.title }}</h3>
              <p class="card__meta">{{ isMine(r) ? 'Your request' : 'From ' + ownerName(r) }} · {{ r.created_at * 1000 | date:'MMM d' }}</p>
              @if (r.description) { <p class="card__desc">{{ r.description }}</p> }
              <p class="card__status">
                {{ r.received | nc }} NC received{{ r.target !== null ? ' of ' + (r.target | nc) + ' NC' : '' }}
              </p>
              @if (r.target !== null) {
                <progress [value]="progress(r)" max="100" [attr.aria-label]="r.title + ' progress'">{{ progress(r) }}%</progress>
              }
              <p class="card__meta">
                @switch (state(r)) {
                  @case ('closed') { Closed }
                  @case ('ended') { Ended {{ r.deadline! * 1000 | date:'MMM d' }} }
                  @default { {{ r.deadline !== null ? 'Open until ' + (r.deadline * 1000 | date:'MMM d') : 'Open' }} }
                }
              </p>
              @if (state(r) === 'open') {
                @if (isMine(r)) {
                  <button class="btn btn--quiet" type="button" [disabled]="busy()" (click)="close(r)">Close request</button>
                } @else {
                  <form class="gift-form" (ngSubmit)="give(r)">
                    <label>Amount <input [name]="'amount-' + r.id" type="text" inputmode="decimal" [(ngModel)]="gifts[r.id]" placeholder="0" /></label>
                    <label>Note (optional) <input [name]="'note-' + r.id" [(ngModel)]="notes[r.id]" maxlength="96" placeholder="Enjoy!" /></label>
                    <button class="btn" type="submit" [disabled]="busy()">Give</button>
                  </form>
                }
              }
            </article>
          } @empty {
            @if (book.hasValue()) { <p class="muted">{{ view() === 'mine' ? 'You have not asked for anything yet.' : view() === 'open' ? 'Nobody is asking for anything right now.' : 'No closed requests.' }}</p> }
          }
        </div>
      }
    }
  `,
  styles: `progress{width:100%;accent-color:var(--accent)}.gift-form{display:flex;flex-direction:column;gap:.4rem;margin-top:auto}.gift-form label{font-size:.875rem}`,
})
export class GiftRequestsPage {
  protected readonly session = inject(Session);
  private readonly api = inject(NanacoinService);
  private readonly money = inject(Money);
  private readonly toasts = inject(Toasts);
  private readonly dialogs = inject(Dialogs);

  protected readonly views = [
    { id: 'open', label: 'Open' }, { id: 'mine', label: 'Mine' }, { id: 'closed', label: 'Closed' }, { id: 'new', label: 'Ask for a gift' },
  ] as const;
  protected readonly view = signal<'open' | 'mine' | 'closed' | 'new'>('open');
  protected readonly busy = signal(false);
  protected title = ''; protected description = ''; protected target = ''; protected deadline = '';
  protected gifts: Record<number, string> = {}; protected notes: Record<number, string> = {};
  /** Retried gifts reuse their key so a dropped response cannot pay twice. */
  private readonly pendingKeys = new Map<string, string>();

  protected readonly book = resource({
    params: () => this.session.me()?.account,
    loader: () => this.api.giftRequests(),
  });
  private readonly followLedger = reloadOnLedgerChange(this.book);

  private readonly requests = computed(() => [...(this.book.value()?.requests ?? [])].sort((a, b) => b.created_at - a.created_at));
  private readonly myNumber = computed(() => Number(this.session.me()?.id.replace('user-', '')));
  private now(): number { return Math.floor(Date.now() / 1000); }
  protected state(r: GiftRequest): GiftRequestState { return requestState(r, this.now()); }
  protected isMine(r: GiftRequest): boolean { return r.owner === this.myNumber(); }
  protected ownerName(r: GiftRequest): string {
    return this.session.household().find(u => u.id === `user-${r.owner}`)?.display_name ?? `Member ${r.owner}`;
  }
  protected progress(r: GiftRequest): number { return r.target ? Math.min(100, Math.floor(r.received * 100 / r.target)) : 0; }
  private filtered(view: string): GiftRequest[] {
    return this.requests().filter(r => view === 'mine' ? this.isMine(r) : view === 'open' ? this.state(r) === 'open' : view === 'closed' ? this.state(r) !== 'open' : false);
  }
  protected count(view: string): number { return view === 'new' ? this.requests().length : this.filtered(view).length; }
  protected readonly visible = computed(() => this.filtered(this.view()));

  protected async create(): Promise<void> {
    if (this.busy()) return;
    const title = this.title.trim(), description = this.description.trim();
    if (!title) { this.toasts.error('Say what the gift is for.'); return; }
    if (bytes(title) > 80 || bytes(description) > 96) { this.toasts.error('Keep the title within 80 bytes and details within 96; some characters use more than one.'); return; }
    let target: number | null = null;
    try { if (this.target.trim()) target = this.money.parse(this.target); } catch (e) { this.toasts.fromError(e); return; }
    if (target !== null && target <= 0) { this.toasts.error('A target must be more than zero, or left empty.'); return; }
    const deadline = deadlineFromDate(this.deadline);
    if (deadline !== null && deadline <= this.now()) { this.toasts.error('Choose a last day that has not passed.'); return; }
    await this.run(JSON.stringify(['create', title, description, target, deadline]),
      { create_request: { title, description, target, deadline } }, 'Gift request posted.', () => {
        this.title = ''; this.description = ''; this.target = ''; this.deadline = ''; this.view.set('mine');
      });
  }

  protected async give(r: GiftRequest): Promise<void> {
    if (this.busy()) return;
    let amount: number;
    try { amount = this.money.parse(this.gifts[r.id] ?? ''); } catch (e) { this.toasts.fromError(e); return; }
    if (amount <= 0) { this.toasts.error('Enter an amount to give.'); return; }
    const memo = (this.notes[r.id] ?? '').trim();
    if (bytes(memo) > 96) { this.toasts.error('Keep the note within 96 bytes.'); return; }
    if (await this.dialogs.confirm({ title: 'Give this gift?', message: 'Gifts are sent immediately.', detail: [`${this.money.format(amount)} NC to ${this.ownerName(r)}`, r.title], confirmLabel: 'Give' }) === null) return;
    await this.run(JSON.stringify(['give', r.id, amount, memo]), { contribute: { request: r.id, amount, memo } }, 'Gift sent. Thank you!', () => {
      delete this.gifts[r.id]; delete this.notes[r.id];
    });
  }

  protected async close(r: GiftRequest): Promise<void> {
    if (this.busy()) return;
    if (await this.dialogs.confirm({ title: 'Close this request?', message: 'Nobody can give to it after it closes. Gifts already received stay yours.', detail: [r.title], confirmLabel: 'Close request' }) === null) return;
    await this.run(JSON.stringify(['close', r.id]), { close_request: { request: r.id } }, 'Request closed.', () => {});
  }

  private async run(identity: string, action: Parameters<NanacoinService['commerce']>[0], done: string, reset: () => void): Promise<void> {
    this.busy.set(true);
    const key = this.pendingKeys.get(identity) ?? newIdempotencyKey(); this.pendingKeys.set(identity, key);
    try {
      await this.api.commerce(action, key);
      this.pendingKeys.delete(identity);
      reset(); this.toasts.ok(done); this.book.reload();
      await this.session.refresh();
    } catch (e) { this.toasts.fromError(e); }
    finally { this.busy.set(false); }
  }
}
