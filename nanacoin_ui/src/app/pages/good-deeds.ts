import { Component, computed, inject, resource, signal } from '@angular/core';
import { DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { Listing, Offer } from '../api/models';
import { Money, MoneyPipe } from '../api/money';
import { ApiError, NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session, reloadOnLedgerChange } from '../api/session';
import { Toasts } from '../ui/toasts';
import { Dialogs } from '../ui/dialog';
import { GOOD_DEED, GoodDeedIdea, isGoodDeed, missingIdeas } from '../nana/good-deeds';

const bytes = (text: string) => new TextEncoder().encode(text).length;

/**
 * Nana's good deeds: standing rewards paid in newly issued coins. Members
 * claim one with an ordinary offer; accepting it here issues the reward.
 */
@Component({
  selector: 'app-good-deeds', imports: [DatePipe, FormsModule, MoneyPipe, RouterLink],
  template: `
    <h1>Good Deeds</h1>
    <p class="muted">Nana issues new money for good deeds. Each deed stays posted in the <a routerLink="/market">Market</a>: anyone can do it, say “I did this”, and you decide. Accepting a claim creates the reward as brand-new coins; your own balance is untouched. Undoing an accepted claim within the settlement window retires those coins again.</p>
    @if (!session.isNana()) { <p>Only Nana manages good deeds. <a routerLink="/market">See the good deeds on offer</a>.</p> }
    @else {
      <section class="panel" aria-labelledby="claims-heading">
        <h2 id="claims-heading">Claims waiting for you ({{ claims().length }})</h2>
        @if (offers.error()) { <p role="alert">Could not load claims. <button class="btn btn--quiet btn--small" (click)="offers.reload()">Retry</button></p> }
        @for (o of claims(); track o.id) {
          <article class="card deed-claim" [attr.aria-label]="o.offerer_name + ': ' + o.listing_title">
            <p><strong>{{ o.offerer_name }}</strong> did “{{ o.listing_title }}” · {{ o.created_at * 1000 | date:'MMM d' }}</p>
            @if (o.message) { <p class="card__desc">“{{ o.message }}”</p> }
            <p class="card__meta">Reward {{ o.amount | nc }} NC</p>
            <div class="deed-actions">
              <button class="btn" type="button" [disabled]="busy()" (click)="grant(o)">Accept and issue {{ o.amount | nc }} NC</button>
              <button class="btn btn--quiet" type="button" [disabled]="busy()" (click)="decline(o)">Decline</button>
            </div>
          </article>
        } @empty { @if (offers.hasValue()) { <p class="muted">No claims right now.</p> } }
      </section>

      <section class="panel" aria-labelledby="add-heading">
        <h2 id="add-heading">Add good deeds</h2>
        <p>
          <button class="btn" type="button" [disabled]="busy() || missing().length === 0" (click)="addStarterSet()">
            {{ missing().length === 0 ? 'All 25 starter deeds are posted' : 'Add ' + missing().length + ' good deeds' }}
          </button>
        </p>
        <p class="muted small">Adds the starter set of 25 (skipping any already posted). Good deeds share the board’s 48 open-listing slots with everyone’s market listings.</p>
        <form (ngSubmit)="addOne()">
          <label>Good deed <input name="title" [(ngModel)]="title" maxlength="80" required placeholder="Help a neighbor rake leaves" /></label>
          <label>Details (optional) <input name="description" [(ngModel)]="description" maxlength="96" placeholder="The whole yard, bagged" /></label>
          <label>Reward in new coins <input name="reward" type="text" inputmode="decimal" [(ngModel)]="reward" required placeholder="5" /></label>
          <button class="btn" type="submit" [disabled]="busy()">Post good deed</button>
        </form>
      </section>

      <section class="panel" aria-labelledby="posted-heading">
        <h2 id="posted-heading">Posted good deeds ({{ deeds().length }})</h2>
        <div class="table-scroll"><table>
          <thead><tr><th>Good deed</th><th>Reward</th><th>Rewarded</th><th></th></tr></thead>
          <tbody>
            @for (d of deeds(); track d.id) {
              <tr>
                <td>{{ d.title }}@if (d.description) { <br><span class="muted small">{{ d.description }}</span> }</td>
                <td>{{ d.price | nc }} NC</td>
                <td>{{ rewarded(d) }}</td>
                <td><button class="btn btn--quiet btn--small" type="button" [disabled]="busy()" (click)="remove(d)">Remove</button></td>
              </tr>
            } @empty { <tr><td colspan="4" class="muted">No good deeds posted yet.</td></tr> }
          </tbody>
        </table></div>
      </section>
    }`,
  styles: `table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:.5rem;border-bottom:1px solid var(--line);vertical-align:top}.table-scroll{overflow:auto}.deed-claim{margin:.5rem 0}.deed-actions{display:flex;flex-wrap:wrap;gap:.5rem}`,
})
export class GoodDeedsPage {
  protected readonly session = inject(Session);
  private readonly api = inject(NanacoinService);
  private readonly money = inject(Money);
  private readonly toasts = inject(Toasts);
  private readonly dialogs = inject(Dialogs);
  protected readonly busy = signal(false);
  protected title = ''; protected description = ''; protected reward = '';

  protected readonly offers = resource({ params: () => (this.session.isNana() ? this.session.me()?.id : undefined), loader: () => this.api.offers() });
  private readonly followLedger = reloadOnLedgerChange(this.offers);

  protected readonly deeds = computed(() => this.session.forSale().filter(isGoodDeed));
  protected readonly missing = computed(() => missingIdeas(this.session.forSale()));
  private readonly deedIds = computed(() => new Set(this.session.listings().filter(isGoodDeed).map((l) => l.id)));
  protected readonly claims = computed(() => (this.offers.value()?.offers ?? []).filter((o) => o.status === 'OPEN' && this.deedIds().has(o.listing)));
  protected rewarded(d: Listing): number {
    return (this.offers.value()?.offers ?? []).filter((o) => o.listing === d.id && (o.status === 'ACCEPTED' || o.status === 'SETTLED')).length;
  }

  private async post(idea: GoodDeedIdea | { title: string; description: string; price: number }): Promise<void> {
    const price = 'price' in idea ? idea.price : idea.reward * 10 ** this.money.decimals();
    await this.api.createListing({ title: idea.title, description: idea.description, price, side: 'BUY', kind: GOOD_DEED });
  }

  protected async addStarterSet(): Promise<void> {
    const ideas = this.missing();
    if (!ideas.length || this.busy()) return;
    if (await this.dialogs.confirm({ title: `Post ${ideas.length} good deeds?`, message: 'Each appears in the Market for anyone to claim. Rewards are issued only when you accept a claim.', detail: ideas.slice(0, 5).map((i) => `${i.title}: ${i.reward} NC`).concat(ideas.length > 5 ? [`…and ${ideas.length - 5} more`] : []), confirmLabel: 'Post them' }) === null) return;
    this.busy.set(true);
    let added = 0;
    try {
      for (const idea of ideas) { await this.post(idea); added++; }
      this.toasts.ok(`Posted ${added} good deeds.`);
    } catch (e) {
      if (e instanceof ApiError && e.status === 507) this.toasts.error(`Posted ${added}; the board has no more open-listing slots. Remove old listings or deeds to add more.`);
      else this.toasts.fromError(e);
    } finally {
      this.busy.set(false);
      await this.session.refresh();
    }
  }

  protected async addOne(): Promise<void> {
    if (this.busy()) return;
    const title = this.title.trim(), description = this.description.trim();
    if (!title || bytes(title) > 80 || bytes(description) > 96) { this.toasts.error('Name the deed in at most 80 bytes, with details in at most 96.'); return; }
    let price: number;
    try { price = this.money.parse(this.reward); } catch (e) { this.toasts.fromError(e); return; }
    if (price <= 0) { this.toasts.error('The reward must be more than zero.'); return; }
    this.busy.set(true);
    try {
      await this.post({ title, description, price });
      this.title = ''; this.description = ''; this.reward = '';
      this.toasts.ok('Good deed posted.');
      await this.session.refresh();
    } catch (e) { this.toasts.fromError(e); }
    finally { this.busy.set(false); }
  }

  protected async grant(o: Offer): Promise<void> {
    if (this.busy()) return;
    if (await this.dialogs.confirm({ title: 'Reward this good deed?', message: 'New coins are issued to them now.', detail: [`${o.offerer_name}: ${o.listing_title}`, `${this.money.format(o.amount)} new NC`], confirmLabel: 'Issue reward' }) === null) return;
    await this.act(() => this.api.acceptOffer(o.id, newIdempotencyKey()), 'Reward issued.');
  }

  protected async decline(o: Offer): Promise<void> {
    await this.act(() => this.api.declineOffer(o.id), 'Claim declined.');
  }

  protected async remove(d: Listing): Promise<void> {
    if (await this.dialogs.confirm({ title: 'Remove this good deed?', message: 'It leaves the Market. Rewards already issued stay issued.', detail: [d.title], confirmLabel: 'Remove' }) === null) return;
    await this.act(() => this.api.cancelListing(d.id), 'Good deed removed.');
  }

  private async act(action: () => Promise<unknown>, done: string): Promise<void> {
    if (this.busy()) return;
    this.busy.set(true);
    try { await action(); this.toasts.ok(done); }
    catch (e) { this.toasts.fromError(e); }
    finally { this.busy.set(false); this.offers.reload(); await this.session.refresh(); }
  }
}
