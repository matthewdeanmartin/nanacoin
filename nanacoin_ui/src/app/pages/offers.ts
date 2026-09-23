import { RouterLink } from '@angular/router';
import { Money, MoneyPipe } from '../api/money';
import { inject as moneyInject } from '@angular/core';
import { Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { offerGroup, offerPayment, offerSentence } from './offer-language';
import { Offer } from '../api/models';
import { ApiError, NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { Toasts } from '../ui/toasts';
import { Mastodon } from '../api/mastodon';

@Component({
  selector: 'app-offers',
  imports: [MoneyPipe, FormsModule, RouterLink],
  template: `
    <h1>Offers</h1>
    <p class="muted">Offers move money only when the listing owner accepts.</p>
    @if (mastodon.connected()) {
      <label class="checkbox"><input type="checkbox" [(ngModel)]="notifyAccepted" /> Also send a Mastodon copy when accepting</label>
      @if (notifyAccepted) { <label class="checkbox"><input type="checkbox" [(ngModel)]="notificationAllCaps" /> ALL CAPS</label> }
    }
    @if (unsupported()) { <p>This server does not support offers.</p> }
    @else {
      @if (loading()) { <p role="status">Loading offers…</p> }
      @for (group of groups(); track group.id) {
        <section class="offer-section" [attr.aria-labelledby]="group.id">
          <h2 [id]="group.id">{{group.title}} <span class="muted small">({{group.offers.length}})</span></h2>
          <p class="muted small">{{group.description}}</p>
          <div class="cards">
            @for (o of group.offers; track o.id) {
              <article class="card">
                <h3>{{o.listing_title || 'Listing no longer available'}}</h3>
                <p>{{sentence(o)}}</p>
                @if (o.settled_tx && o.status !== 'REVERSED') { <p><a routerLink="/history" fragment="my-todos">Track work or delivery in My Account TODO</a></p> }
                <p class="card__meta"><strong>{{o.amount | nc}} NC</strong> · {{payment(o)}} · {{o.status.toLowerCase().replaceAll('_', ' ')}}</p>
                @if (o.message) { <p class="card__desc">{{o.message}}</p> }
                <div class="offer-actions">
                  @if (o.status === 'OPEN' && o.listing_owner === session.me()?.account) {
                    <button class="btn" (click)="accept(o)" [disabled]="busy() !== null">Accept</button>
                    <button class="btn btn--quiet" (click)="decline(o)" [disabled]="busy() !== null">Decline</button>
                  }
                  @if (o.status === 'OPEN' && o.offerer === session.me()?.account) {
                    <button class="btn btn--quiet" (click)="withdraw(o)" [disabled]="busy() !== null">Withdraw</button>
                  }
                  @if (o.reversible) { <button class="btn btn--quiet" (click)="undo(o)" [disabled]="busy() !== null">Undo acceptance</button> }
                </div>
              </article>
            } @empty { <p class="muted small">No offers in this section.</p> }
          </div>
        </section>
      }
    }
  `,
  styles: [`.offer-section { margin-block: 1.5rem; } .offer-actions { display:flex; flex-wrap:wrap; gap:.75rem; margin-top:1rem; } .offer-actions .btn { padding:.65rem 1rem; min-height:44px; }`],
})
export class OffersPage {
  protected readonly money = moneyInject(Money);
  private readonly api = inject(NanacoinService);
  private readonly toasts = inject(Toasts);
  private readonly dialogs = inject(Dialogs);
  protected readonly mastodon = inject(Mastodon);
  protected readonly session = inject(Session);

  protected readonly offers = signal<Offer[]>([]);
  protected readonly loading = signal(false);

  /** True once the server has told us it has no /offers endpoint. */
  protected readonly unsupported = signal(false);

  /** The offer currently being acted on, so only its buttons go busy. */
  protected readonly busy = signal<string | null>(null);
  protected notifyAccepted = false;
  protected notificationAllCaps = false;

  protected readonly sentence = offerSentence;
  protected readonly payment = offerPayment;
  protected readonly groups = computed(() => {
    const account = this.session.me()?.account ?? '';
    const name = this.session.me()?.display_name ?? 'this account';
    return [
      { id: 'received-buy', title: 'Received · offers to buy', description: `Other people have offered to buy from ${name}.` },
      { id: 'received-sell', title: 'Received · offers to sell', description: `Other people have offered to sell to ${name}.` },
      { id: 'sent-buy', title: 'Sent · offers to buy', description: `${name} has offered to buy from other people.` },
      { id: 'sent-sell', title: 'Sent · offers to sell', description: `${name} has offered to sell to other people.` },
    ].map(g => ({ ...g, offers: this.offers().filter(o => offerGroup(o,account) === g.id)
      .sort((a,b) => Number(b.status === 'OPEN') - Number(a.status === 'OPEN') || b.updated_at-a.updated_at) }));
  });
  protected readonly toDecide = computed(() => this.offers().filter(o => o.status === 'OPEN' && o.listing_owner === this.session.me()?.account));

  constructor() {
    void this.load();
  }

  protected async load(): Promise<void> {
    this.loading.set(true);
    try {
      const page = await this.api.offers();
      this.offers.set(page.offers);
      this.unsupported.set(false);
    } catch (e) {
      // A 404 here is the board being older than this feature, which is the
      // expected state today rather than a failure worth shouting about.
      if (e instanceof ApiError && e.status === 404) {
        this.unsupported.set(true);
      } else {
        this.toasts.fromError(e);
      }
    } finally {
      this.loading.set(false);
    }
  }

  /**
   * Accepts an offer, which is the step that moves the money.
   *
   * Confirmed first: this is the irreversible half of the whole feature, and
   * the amount is worth seeing spelled out before a tap commits it.
   */
  protected async accept(offer: Offer): Promise<void> {
    if (this.busy()) return;
    const competing = this.toDecide().filter((candidate) =>
      candidate.listing === offer.listing && candidate.id !== offer.id).length;
    const ok = await this.dialogs.confirm({
      title: 'Accept this offer?',
      message: 'This moves the money now and closes the listing.',
      detail: [
        `${offerPayment(offer)} ${this.money.format(offer.amount)} NC.`,
        offer.listing_title || 'a listing that no longer exists',
        ...(competing > 0
          ? [`This closes ${competing} competing ${competing === 1 ? 'offer' : 'offers'} as not selected.`]
          : []),
        'Undo acceptance remains available until the server’s settlement deadline.',
      ],
      confirmLabel: 'Accept',
    });
    if (ok === null) return;

    this.busy.set(offer.id);
    try {
      await this.api.acceptOffer(offer.id, newIdempotencyKey());
      this.toasts.ok(competing > 0
        ? `Accepted. ${competing} competing ${competing === 1 ? 'offer was' : 'offers were'} closed as not selected.`
        : 'Accepted. Work or delivery is now listed in My Account TODO.');
      if (this.notifyAccepted) {
        const recipient = this.session.household().find((u) => u.account === offer.offerer);
        if (!recipient?.mastodon_id) {
          this.toasts.error(`${offer.offerer_name} has no registered Mastodon ID; the offer was still accepted.`);
        } else {
          let message = `Your NanaCoin offer for ${offer.listing_title} was accepted for ${this.money.format(offer.amount)} coins.`;
          if (this.notificationAllCaps) message = message.toLocaleUpperCase();
          try { await this.mastodon.sendDirect(recipient, message); }
          catch (e) { this.toasts.error(`Offer accepted, but the private message failed: ${e instanceof Error ? e.message : 'Mastodon error'}`); }
        }
      }
      await this.session.refresh();
      await this.load();
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.busy.set(null);
    }
  }

  protected async undo(offer: Offer): Promise<void> {
    if (this.busy()) return;
    const reason = await this.dialogs.prompt({ title: 'Undo acceptance?', message: 'Return the payment and reopen the listing.', required: true });
    if (reason === null) return;
    this.busy.set(offer.id);
    try { await this.api.unacceptOffer(offer.id,reason,newIdempotencyKey()); await this.session.refresh(); await this.load(); }
    catch (e) { this.toasts.fromError(e); } finally { this.busy.set(null); }
  }

  protected async decline(offer: Offer): Promise<void> {
    if (this.busy()) return;
    this.busy.set(offer.id);
    try {
      await this.api.declineOffer(offer.id);
      this.toasts.ok('Declined.');
      await this.load();
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.busy.set(null);
    }
  }

  protected async withdraw(offer: Offer): Promise<void> {
    if (this.busy()) return;
    this.busy.set(offer.id);
    try {
      await this.api.withdrawOffer(offer.id);
      this.toasts.ok('Withdrawn.');
      await this.load();
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.busy.set(null);
    }
  }
}
