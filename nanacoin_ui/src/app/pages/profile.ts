import { Component, computed, inject, resource } from '@angular/core';
import { DatePipe } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { toSignal } from '@angular/core/rxjs-interop';
import { map } from 'rxjs';
import { MoneyPipe } from '../api/money';
import { NanacoinService } from '../api/nanacoin.service';
import { Session, reloadOnLedgerChange } from '../api/session';
import { ArtPicture } from '../art/art-picture';
import { memberNumber } from '../people/people';
import { publicReport } from '../people/public-report';

/**
 * A read-only look at one member, built only from what every member may see:
 * the public ledger, the market, art, gift requests and open offers. Private
 * messages and other people's offer notes never reach this page.
 */
@Component({
  selector: 'app-profile', imports: [DatePipe, MoneyPipe, RouterLink, ArtPicture],
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
        </div>
      </header>

      <div class="stats" aria-label="Standing">
        @if (p.balance !== undefined) { <div class="stat"><span>{{ p.balance | nc }}</span><span class="stat__label">NC balance</span></div> }
        <div class="stat"><span>{{ report()?.count ?? '…' }}</span><span class="stat__label">recent payments</span></div>
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
            <p class="muted small">The latest {{ r.count }} payments, {{ r.first! * 1000 | date:'mediumDate' }} to {{ r.last! * 1000 | date:'mediumDate' }}. Messages are private and never shown.</p>
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
            <h3>Recent activity</h3>
            <ul class="profile-activity">
              @for (t of recent(); track t.id) {
                <li><time>{{ t.created_at * 1000 | date:'MMM d' }}</time> <span>{{ t.description || t.kind }}</span> <strong [class.credit]="t.delta > 0" [class.debit]="t.delta < 0">{{ t.delta > 0 ? '+' : '' }}{{ t.delta | nc }}</strong></li>
              }
            </ul>
          }
        } @else { <p role="status">Reading the ledger…</p> }
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
    .profile-art{grid-template-columns:repeat(auto-fill,minmax(min(100%,10rem),1fr))}`,
})
export class ProfilePage {
  protected readonly session = inject(Session);
  private readonly api = inject(NanacoinService);
  private readonly id = toSignal(inject(ActivatedRoute).paramMap.pipe(map((p) => p.get('id') ?? '')), { initialValue: '' });

  protected readonly person = computed(() => this.session.household().find((u) => u.id === this.id()) ?? null);
  protected readonly isMe = computed(() => this.person()?.id === this.session.me()?.id);
  private readonly number = computed(() => memberNumber(this.person()));

  protected readonly history = resource({
    params: () => this.person()?.account,
    loader: ({ params }) => this.api.accountHistory(params, 100),
  });
  protected readonly offers = resource({
    params: () => (this.session.signedIn() ? this.person()?.id : undefined),
    loader: ({ params }) => this.api.outstandingOffers(params),
  });
  protected readonly commerce = resource({
    params: () => (this.session.signedIn() ? this.id() : undefined),
    loader: () => this.api.commerceBook(),
  });
  private readonly followLedger = reloadOnLedgerChange(this.history, this.offers, this.commerce);

  protected readonly report = computed(() => {
    const h = this.history.value(), p = this.person();
    return h && p ? publicReport(p.account, h.transactions) : null;
  });
  protected readonly recent = computed(() => {
    const account = this.person()?.account;
    return (this.history.value()?.transactions ?? [])
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

  protected userIdFor(account: string): string | null {
    return this.session.household().find((u) => u.account === account)?.id ?? null;
  }
}
