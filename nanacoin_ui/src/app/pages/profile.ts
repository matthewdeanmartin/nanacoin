import { Component, computed, inject, linkedSignal, resource, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { DatePipe } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { toSignal } from '@angular/core/rxjs-interop';
import { map } from 'rxjs';
import { Transaction } from '../api/models';
import { MoneyPipe } from '../api/money';
import { NanacoinService } from '../api/nanacoin.service';
import { Toasts } from '../ui/toasts';
import { Session, reloadOnLedgerChange } from '../api/session';
import { ArtPicture } from '../art/art-picture';
import { memberNumber } from '../people/people';
import { publicReport } from '../people/public-report';

/** Payments read per request: small enough for the board's response buffer. */
const PAGE = 50;

/**
 * A read-only look at one member, built only from what every member may see:
 * the public ledger, the market, art, gift requests and open offers. Private
 * messages and other people's offer notes never reach this page.
 */
@Component({
  selector: 'app-profile', imports: [DatePipe, FormsModule, MoneyPipe, RouterLink, ArtPicture],
  template: `
    <p><a routerLink="/people">← Demographics</a></p>
    @if (!session.signedIn()) { <p>Sign in to see household profiles.</p> }
    @else if (!person()) { <p role="status">{{ session.household().length ? 'There is no such member.' : 'Loading…' }}</p> }
    @else {
      @let p = person()!;
      <header class="profile-head">
        @if (badge(); as art) { <div class="profile-badge"><app-art-picture [art]="art" [controls]="false" /></div> }
        <div>
          <h1>{{ p.display_name }}@if (isMe()) { <span class="muted small"> (you)</span> }</h1>
          <p class="muted">&#64;{{ p.username }} · {{ p.role === 'nana' ? 'Nana (central bank)' : 'Member' }} · {{ p.status === 'ACTIVE' ? 'Active' : 'Disabled' }} · joined {{ p.created_at * 1000 | date:'mediumDate' }}@if (p.mastodon_id) { · Mastodon {{ p.mastodon_id }} }</p>
          @if (editingBio()) {
            <form class="profile-bio-form" (ngSubmit)="saveBio()">
              <label>About you <input name="bio" [(ngModel)]="bioDraft" maxlength="96" placeholder="I bake lemon bars and fix bikes" /></label>
              <button class="btn btn--small" type="submit" [disabled]="savingBio()">Save</button>
              <button class="btn btn--quiet btn--small" type="button" (click)="editingBio.set(false)">Cancel</button>
            </form>
          } @else {
            @if (p.bio) { <p class="profile-bio">“{{ p.bio }}”</p> }
            @if (isMe()) { <button class="btn btn--quiet btn--small" type="button" (click)="editBio()">{{ p.bio ? 'Edit your bio' : 'Write a bio' }}</button> }
          }
        </div>
      </header>

      <div class="stats" aria-label="Standing">
        @if (p.balance !== undefined) { <div class="stat"><span>{{ p.balance | nc }}</span><span class="stat__label">NC balance</span></div> }
        <div class="stat"><span>{{ report()?.count ?? '…' }}</span><span class="stat__label">{{ cursor() ? 'recent payments' : 'payments' }}</span></div>
        <div class="stat"><span>{{ owned().length }}</span><span class="stat__label">art owned</span></div>
        <div class="stat"><span>{{ listings().length }}</span><span class="stat__label">for sale / wanted</span></div>
        <div class="stat"><span>{{ offers.value()?.offers?.length ?? '…' }}</span><span class="stat__label">open offers</span></div>
      </div>

      <section class="panel" aria-labelledby="ledger-heading">
        <h2 id="ledger-heading">From the public ledger</h2>
        @if (history.error()) { <p role="alert">Could not read the ledger. <button class="btn btn--quiet btn--small" (click)="history.reload()">Retry</button></p> }
        @else if (report(); as r) {
          @if (r.count === 0) { <p class="muted">No money has moved in or out of this account recently.</p> }
          @else {
            <p class="muted small">{{ coverage(r.count) }}, {{ r.first! * 1000 | date:'mediumDate' }} to {{ r.last! * 1000 | date:'mediumDate' }}. Messages are private and never shown.</p>
            <div class="table-scroll"><table>
              <thead><tr><th>Kind</th><th>Received</th><th>Paid</th></tr></thead>
              <tbody>
                @for (row of r.rows; track row.label) { <tr><td>{{ row.label }}</td><td>{{ row.received | nc }}</td><td>{{ row.paid | nc }}</td></tr> }
                <tr class="profile-total"><td>Total</td><td>{{ r.received | nc }}</td><td>{{ r.paid | nc }}</td></tr>
              </tbody>
            </table></div>
            @if (r.partners.length) {
              <p>Trades most with:
                @for (partner of r.partners.slice(0, 5); track partner.account; let last = $last) {
                  @if (userIdFor(partner.account); as id) { <a [routerLink]="['/people', id]">{{ partner.name }}</a> } @else { {{ partner.name }} } ({{ partner.count }}){{ last ? '' : ', ' }}
                }
              </p>
            }
            @if (cursor()) {
              <p><button class="btn btn--quiet btn--small" type="button" [disabled]="loadingOlder()" (click)="loadOlder()">{{ loadingOlder() ? 'Reading…' : 'Include older payments' }}</button></p>
            }
            <h3>Recent activity</h3>
            <ul class="profile-activity">
              @for (t of recent(); track t.id) {
                <li><time>{{ t.created_at * 1000 | date:'MMM d' }}</time> <span>{{ t.description || t.kind }}</span> <strong [class.credit]="t.delta > 0" [class.debit]="t.delta < 0">{{ t.delta > 0 ? '+' : '' }}{{ t.delta | nc }}</strong></li>
              }
            </ul>
          }
        } @else { <p role="status">Reading the ledger…</p> }
      </section>

      <section class="panel" aria-labelledby="loans-heading">
        <h2 id="loans-heading">Loans and lotto</h2>
        @if (loans.error()) { <p role="alert">Could not load loans.</p> }
        @for (l of loans.value()?.loans ?? []; track l.id) {
          <p>{{ l.lender === p.account ? 'Lent ' + (l.amount | nc) + ' NC to ' + l.borrower_name : 'Borrowed ' + (l.amount | nc) + ' NC from ' + l.lender_name }}
            · {{ l.status === 'PAID' ? 'repaid' : (l.principal | nc) + ' NC still owed' }}@if (l.memo) { · “{{ l.memo }}” }</p>
        } @empty { @if (loans.hasValue()) { <p class="muted">No loans.</p> } }
        @for (d of draws(); track d.id) {
          <p>{{ d.my_tickets }} ticket{{ d.my_tickets === 1 ? '' : 's' }} in “{{ d.terms.title }}” · {{ drawState(d.status, d.winner === p.account) }}</p>
        } @empty { @if (lottos.hasValue()) { <p class="muted">No lotto tickets.</p> } }
      </section>

      <section class="panel" aria-labelledby="art-heading">
        <h2 id="art-heading">Art collection</h2>
        @if (owned().length) {
          <div class="cards profile-art">
            @for (a of owned(); track a.id) {
              <article class="card"><app-art-picture [art]="a" [controls]="false" /><h3>{{ a.title }}</h3><p class="card__meta">Edition #{{ a.id }}@if (a.equipped) { · on profile }@if (a.price !== null) { · for sale at {{ a.price | nc }} NC }</p></article>
            }
          </div>
        } @else { <p class="muted">No art yet.</p> }
        @if (made() > 0) { <p class="muted small">Made {{ made() }} edition{{ made() === 1 ? '' : 's' }}. <a routerLink="/art">Visit the gallery</a></p> }
      </section>

      <section class="panel" aria-labelledby="offers-heading">
        <h2 id="offers-heading">Outstanding offers</h2>
        @if (offers.error()) { <p role="alert">Could not load offers.</p> }
        @for (o of offers.value()?.offers ?? []; track o.id) {
          <p><strong>{{ o.amount | nc }} NC</strong> for “{{ o.listing_title }}” from {{ o.listing_owner_name }} · {{ o.created_at * 1000 | date:'MMM d' }}@if (o.message) { <br><span class="muted">“{{ o.message }}”</span> }</p>
        } @empty { @if (offers.hasValue()) { <p class="muted">No open offers.</p> } }
        <h3>Listed in the market</h3>
        @for (l of listings(); track l.id) {
          <p>{{ l.side === 'BUY' ? 'Wants' : 'Offers' }} “{{ l.title }}” for {{ l.price | nc }} NC</p>
        } @empty { <p class="muted">Nothing listed.</p> }
      </section>

      <section class="panel" aria-labelledby="gifts-heading">
        <h2 id="gifts-heading">Gift requests</h2>
        @for (r of requests(); track r.id) {
          <p>“{{ r.title }}”: {{ r.received | nc }} NC received{{ r.target !== null ? ' of ' + (r.target | nc) + ' NC' : '' }} <a routerLink="/gifts">Give</a></p>
        } @empty { <p class="muted">Not asking for anything.</p> }
      </section>
    }`,
  styles: `
    .profile-head{display:flex;gap:1rem;align-items:center}.profile-head h1{margin:0}.profile-badge{width:5.5rem;flex:none}
    table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:.5rem;border-bottom:1px solid var(--line)}.table-scroll{overflow:auto}
    .profile-total td{font-weight:700}.profile-activity{list-style:none;padding:0;margin:0}
    .profile-activity li{display:grid;grid-template-columns:4rem 1fr auto;gap:.5rem;padding:.35rem 0;border-bottom:1px solid var(--line)}
    .profile-activity span{overflow-wrap:anywhere}.credit{color:var(--credit)}.debit{color:var(--debit)}
    .profile-art{grid-template-columns:repeat(auto-fill,minmax(min(100%,10rem),1fr))}
    .profile-bio{margin:.25rem 0;font-style:italic}.profile-bio-form{display:flex;flex-wrap:wrap;gap:.5rem;align-items:end}.profile-bio-form label{flex:1 1 14rem}`,
})
export class ProfilePage {
  protected readonly session = inject(Session);
  private readonly api = inject(NanacoinService);
  private readonly id = toSignal(inject(ActivatedRoute).paramMap.pipe(map((p) => p.get('id') ?? '')), { initialValue: '' });

  protected readonly person = computed(() => this.session.household().find((u) => u.id === this.id()) ?? null);
  protected readonly isMe = computed(() => this.person()?.id === this.session.me()?.id);
  private readonly number = computed(() => memberNumber(this.person()));

  private readonly toasts = inject(Toasts);
  protected readonly history = resource({
    params: () => this.person()?.account,
    loader: ({ params }) => this.api.accountHistory(params, PAGE),
  });
  /** Older pages read on request; a fresh first page starts over. */
  private readonly older = linkedSignal<unknown, Transaction[]>({ source: () => this.history.value(), computation: () => [] });
  protected readonly cursor = linkedSignal(() => this.history.value()?.next_cursor ?? null);
  protected readonly loadingOlder = signal(false);
  private readonly transactions = computed(() => [...(this.history.value()?.transactions ?? []), ...this.older()]);
  protected readonly loans = resource({
    params: () => (this.session.signedIn() ? this.person()?.id : undefined),
    loader: ({ params }) => this.api.memberLoans(params),
  });
  protected readonly lottos = resource({
    params: () => (this.session.signedIn() ? this.person()?.id : undefined),
    loader: ({ params }) => this.api.memberLottos(params),
  });
  protected readonly draws = computed(() => (this.lottos.value()?.lottos ?? []).filter((d) => d.my_tickets > 0));
  protected readonly editingBio = signal(false);
  protected readonly savingBio = signal(false);
  protected bioDraft = '';
  protected readonly offers = resource({
    params: () => (this.session.signedIn() ? this.person()?.id : undefined),
    loader: ({ params }) => this.api.outstandingOffers(params),
  });
  protected readonly commerce = resource({
    params: () => (this.session.signedIn() ? this.id() : undefined),
    loader: () => this.api.commerceBook(),
  });
  private readonly followLedger = reloadOnLedgerChange(this.history, this.offers, this.commerce, this.loans, this.lottos);

  protected readonly report = computed(() => {
    const p = this.person();
    return this.history.value() && p ? publicReport(p.account, this.transactions()) : null;
  });
  protected readonly recent = computed(() => {
    const account = this.person()?.account;
    return this.transactions()
      .map((t) => ({ ...t, delta: t.postings.filter((p) => p.account === account).reduce((s, p) => s + p.amount, 0) }))
      .filter((t) => t.kind !== 'MESSAGE' && t.delta !== 0)
      .slice(0, 10);
  });
  protected readonly owned = computed(() => (this.commerce.value()?.artworks ?? []).filter((a) => a.owner === this.number()));
  protected readonly made = computed(() => (this.commerce.value()?.artworks ?? []).filter((a) => a.creator === this.number()).length);
  protected readonly badge = computed(() => this.owned().find((a) => a.equipped) ?? null);
  protected readonly requests = computed(() => (this.commerce.value()?.requests ?? [])
    .filter((r) => r.owner === this.number() && !r.closed && (r.deadline === null || r.deadline * 1000 > Date.now())));
  protected readonly listings = computed(() => this.session.forSale().filter((l) => l.seller === this.person()?.account));

  protected coverage(count: number): string {
    if (this.cursor()) return `The latest ${count} payments`;
    return this.history.value()?.history_truncated ? `All ${count} payments the board still keeps` : `All ${count} payments`;
  }

  protected drawState(status: string, won: boolean): string {
    return status === 'SETTLED' ? (won ? 'won' : 'drawn') : status === 'OPEN' ? 'open' : 'waiting for the draw';
  }

  protected async loadOlder(): Promise<void> {
    const account = this.person()?.account, cursor = this.cursor();
    if (!account || !cursor || this.loadingOlder()) return;
    this.loadingOlder.set(true);
    try {
      const page = await this.api.accountHistory(account, PAGE, cursor);
      this.older.update((rows) => [...rows, ...page.transactions]);
      this.cursor.set(page.next_cursor ?? null);
    } catch (e) {
      // A reform or archive rotation invalidates the cursor: start over.
      this.toasts.fromError(e);
      this.history.reload();
    } finally { this.loadingOlder.set(false); }
  }

  protected editBio(): void { this.bioDraft = this.person()?.bio ?? ''; this.editingBio.set(true); }

  protected async saveBio(): Promise<void> {
    const me = this.session.me(), bio = this.bioDraft.trim();
    if (!me) return;
    if (new TextEncoder().encode(bio).length > 96 || /[\u0000-\u001f\u007f]/.test(bio)) { this.toasts.error('Keep it to one line of at most 96 bytes.'); return; }
    this.savingBio.set(true);
    try {
      await this.api.setUserBio(me.id, bio);
      this.editingBio.set(false);
      this.toasts.ok('Bio saved.');
      await this.session.refresh();
    } catch (e) { this.toasts.fromError(e); }
    finally { this.savingBio.set(false); }
  }

  protected userIdFor(account: string): string | null {
    return this.session.household().find((u) => u.account === account)?.id ?? null;
  }
}
