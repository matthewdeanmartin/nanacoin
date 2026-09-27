import { Component, computed, inject, resource } from '@angular/core';
import { DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MoneyPipe } from '../api/money';
import { NanacoinService } from '../api/nanacoin.service';
import { Session, reloadOnLedgerChange } from '../api/session';
import { ArtPicture } from '../art/art-picture';
import { memberNumber } from '../people/people';
import { population } from '../people/public-report';

/** Who is on the board: household population and a roster linking to each profile. */
@Component({
  selector: 'app-demographics', imports: [DatePipe, MoneyPipe, RouterLink, ArtPicture],
  template: `
    <h1>Demographics</h1>
    <p class="muted">Everyone on this board. Open a profile to see a read-only report built from the public ledger, their art, and their open offers. Private messages are never shown.</p>
    @if (!session.signedIn()) { <p>Sign in to see the household.</p> }
    @else {
      @let pop = stats();
      <div class="stats" aria-label="Population">
        <div class="stat"><span>{{ pop.members }}</span><span class="stat__label">members</span></div>
        <div class="stat"><span>{{ pop.active }}</span><span class="stat__label">active</span></div>
        <div class="stat"><span>{{ pop.disabled }}</span><span class="stat__label">disabled</span></div>
        <div class="stat"><span>{{ pop.total | nc }}</span><span class="stat__label">NC held by active members</span></div>
        @if (pop.median !== null) { <div class="stat"><span>{{ pop.median | nc }}</span><span class="stat__label">median balance</span></div> }
      </div>
      @if (pop.since !== null) {
        <p class="muted small">Household since {{ pop.since * 1000 | date:'mediumDate' }}@if (pop.newest; as n) {; newest member {{ n.display_name }}, {{ n.created_at * 1000 | date:'mediumDate' }}}.</p>
      }
      <div class="table-scroll"><table class="roster">
        <caption>Who is on the board</caption>
        <thead><tr><th>Member</th><th>Role</th><th>Status</th><th>Joined</th><th>Balance</th><th>Art</th><th>Listings</th></tr></thead>
        <tbody>
          @for (u of roster(); track u.id) {
            <tr [class.roster--disabled]="u.status !== 'ACTIVE'">
              <td><span class="roster-name">
                @if (u.badge; as art) { <app-art-picture [art]="art" [small]="true" [controls]="false" /> }
                <a [routerLink]="['/people', u.id]">{{ u.display_name }}</a>@if (u.id === session.me()?.id) { <span class="muted small">(you)</span> }
              </span></td>
              <td>{{ u.role === 'nana' ? 'Nana' : 'Member' }}</td>
              <td>{{ u.status === 'ACTIVE' ? 'Active' : 'Disabled' }}</td>
              <td>{{ u.created_at * 1000 | date:'mediumDate' }}</td>
              <td>{{ u.balance === undefined ? '—' : (u.balance | nc) }}</td>
              <td>{{ u.art }}</td>
              <td>{{ u.listings }}</td>
            </tr>
          }
        </tbody>
      </table></div>
    }`,
  styles: `table{width:100%;border-collapse:collapse}caption{text-align:left;font-weight:700;padding:.5rem 0}th,td{text-align:left;padding:.5rem;border-bottom:1px solid var(--line);white-space:nowrap}.table-scroll{overflow:auto}.roster-name{display:inline-flex;align-items:center;gap:.5rem}.roster--disabled{opacity:.6}`,
})
export class DemographicsPage {
  protected readonly session = inject(Session);
  private readonly api = inject(NanacoinService);
  private readonly commerce = resource({ params: () => this.session.me()?.account, loader: () => this.api.commerceBook() });
  private readonly followLedger = reloadOnLedgerChange(this.commerce);

  protected readonly stats = computed(() => population(this.session.household()));
  protected readonly roster = computed(() => {
    const art = this.commerce.value()?.artworks ?? [];
    const listings = this.session.forSale();
    return [...this.session.household()]
      .sort((a, b) => (a.role === 'nana' ? 0 : 1) - (b.role === 'nana' ? 0 : 1) || a.display_name.localeCompare(b.display_name))
      .map((u) => {
        const n = memberNumber(u);
        return {
          ...u,
          badge: art.find((a) => a.owner === n && a.equipped) ?? null,
          art: art.filter((a) => a.owner === n).length,
          listings: listings.filter((l) => l.seller === u.account).length,
        };
      });
  });
}
