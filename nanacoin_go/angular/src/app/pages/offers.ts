// Offers: proposing a deal, and deciding on the ones proposed to you.
//
// The marketplace could only ever do one thing before this - buy a listing at
// its asking price. There was no way to offer 100 coins for peanut butter
// cookies, and no way to haggle over a price. An offer is the missing step:
// a proposal that moves no money until the other person accepts.
//
// # This page runs ahead of the server
//
// The endpoints it calls do not exist on the board yet. That is deliberate:
// the board is memory-constrained and changing it is expensive, so the shape
// of the feature is worth settling here - where a mistake costs a rebuild -
// before any bytes are committed to a fixed-size array in RAM.
//
// So every request here may 404, and a 404 on /offers means "this NanaCoin is
// older than this feature", not "something broke". The page says so plainly
// rather than showing an error, because during this period that is the
// expected state rather than a fault.

import { Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { Listing, Offer } from '../api/models';
import { ApiError, NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { Toasts } from '../ui/toasts';

@Component({
  selector: 'app-offers',
  imports: [FormsModule],
  template: `
    <h1>Offers</h1>

    @if (unsupported()) {
      <!--
        The board has not caught up yet. Not an error: it is the expected
        state while the UI is built first, and saying so is more useful than
        a red toast reporting a 404 nobody can act on.
      -->
      <p class="muted">
        This NanaCoin does not support offers yet. The board needs newer
        firmware; everything else works as before.
      </p>
    } @else {
      @if (loading()) {
        <p class="muted">Loading…</p>
      }

      <!-- Offers waiting on you: the ones with a decision to make. -->
      @if (toDecide().length > 0) {
        <h3>Waiting for you</h3>
        <div class="cards">
          @for (o of toDecide(); track o.id) {
            <article class="card">
              <h3>{{ o.listing_title }}</h3>
              <p class="card__meta">
                <strong>{{ o.amount }} {{ o.amount === 1 ? 'coin' : 'coins' }}</strong>
                · from {{ o.offerer_name }}
              </p>
              @if (o.message) {
                <p class="card__desc">{{ o.message }}</p>
              }
              @if (!o.listing_title) {
                <!--
                  The listing this offer points at is gone - recycled, or
                  written with a truncated ID by an older build. There is
                  nothing to accept, so do not offer a button that cannot
                  work; declining still tidies it away.
                -->
                <p class="card__status">
                  The listing this refers to no longer exists.
                </p>
              }
              <div class="card__actions">
                @if (o.listing_title) {
                  <button
                    class="btn"
                    title="Accept this proposal and settle the transaction after confirmation"
                    (click)="accept(o)"
                    [disabled]="busy() !== null"
                  >
                    {{ busy() === o.id ? 'Accepting…' : 'Accept' }}
                  </button>
                }
                <button
                  class="btn btn--quiet"
                  title="Refuse this proposal without moving money"
                  (click)="decline(o)"
                  [disabled]="busy() !== null"
                >
                  Decline
                </button>
              </div>
            </article>
          }
        </div>
      }

      <!-- Offers you have made and are still waiting on. -->
      @if (mine().length > 0) {
        <h3>Yours</h3>
        <div class="cards">
          @for (o of mine(); track o.id) {
            <article class="card">
              <h3>{{ o.listing_title }}</h3>
              <p class="card__meta">
                <strong>{{ o.amount }} {{ o.amount === 1 ? 'coin' : 'coins' }}</strong>
                · <span class="tag">{{ o.status.toLowerCase() }}</span>
              </p>
              @if (o.message) {
                <p class="card__desc">{{ o.message }}</p>
              }
              @if (o.status === 'OPEN') {
                <button
                  class="btn btn--quiet"
                  title="Take back this open proposal"
                  (click)="withdraw(o)"
                  [disabled]="busy() !== null"
                >
                  Withdraw
                </button>
              }
            </article>
          }
        </div>
      }

      @if (!loading() && toDecide().length === 0 && mine().length === 0) {
        <p class="muted">
          No offers yet. Offer on something in the Market, or post a want-ad
          there for something you would like someone to do.
        </p>
      }
    }
  `,
})
export class OffersPage {
  private readonly api = inject(NanacoinService);
  private readonly toasts = inject(Toasts);
  private readonly dialogs = inject(Dialogs);
  protected readonly session = inject(Session);

  protected readonly offers = signal<Offer[]>([]);
  protected readonly loading = signal(false);

  /** True once the server has told us it has no /offers endpoint. */
  protected readonly unsupported = signal(false);

  /** The offer currently being acted on, so only its buttons go busy. */
  protected readonly busy = signal<string | null>(null);

  /** Open offers on listings this user owns: the ones needing a decision. */
  protected readonly toDecide = computed(() => {
    const me = this.session.me()?.account;
    return this.offers().filter((o) => o.status === 'OPEN' && o.offerer !== me);
  });

  /** Offers this user made, whatever became of them. */
  protected readonly mine = computed(() => {
    const me = this.session.me()?.account;
    return this.offers().filter((o) => o.offerer === me);
  });

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
    const ok = await this.dialogs.confirm({
      title: 'Accept this offer?',
      message: 'This moves the money now and closes the listing.',
      detail: [
        `${offer.amount} ${offer.amount === 1 ? 'coin' : 'coins'} from ${offer.offerer_name}`,
        offer.listing_title || 'a listing that no longer exists',
        'You can undo this for a while afterwards, from the Offers tab.',
      ],
      confirmLabel: 'Accept',
    });
    if (ok === null) return;

    this.busy.set(offer.id);
    try {
      await this.api.acceptOffer(offer.id, newIdempotencyKey());
      this.toasts.ok('Accepted.');
      await this.session.refresh();
      await this.load();
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.busy.set(null);
    }
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
