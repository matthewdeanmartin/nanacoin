// Foreign exchange: trading NanaCoin for dollars at a stated rate.
//
// # Why a book rather than a rate
//
// The simplest version of this feature would be Nana setting one number - "a
// coin is worth a quarter" - and everyone trading at it. That was rejected
// because a single rate is not a market: it cannot express "I would sell at
// 30 but not at 25", which is the entire thing a child learns from being
// allowed to trade. So anyone may post a rate, and what emerges is a book
// with two sides.
//
// # Bid and ask, and why the words stay
//
// A bid is someone offering to *buy* coins with dollars. An ask is someone
// offering to *sell* coins for dollars. "Buy" and "sell" on their own are
// ambiguous here - buying which of the two? - so the market's own words are
// used, with the plain-English meaning spelled out beside every number.
//
// # The handover is offline
//
// The ledger records that Alice now holds 250 fewer cents and Bob 250 more.
// Nobody's actual dollar bill moves; the dollars are an entry in the same
// double-entry book as the coins, tagged with a different currency. That is
// the whole design, and it is why a trade can settle instantly without any
// payment rail existing.

import { Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { Quote, QuoteSide } from '../api/models';
import { ApiError, NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { Toasts } from '../ui/toasts';
import { Mastodon } from '../api/mastodon';

@Component({
  selector: 'app-forex',
  imports: [FormsModule],
  template: `
    <h1>Exchange</h1>

    @if (mastodon.connected()) {
      <label class="checkbox">
        <input type="checkbox" [(ngModel)]="notifyTrade" />
        Send a private Mastodon message when I accept an exchange
      </label>
      @if (notifyTrade) {
        <label class="checkbox"><input type="checkbox" [(ngModel)]="notificationAllCaps" /> ALL CAPS</label>
      }
    }

    @if (unsupported()) {
      <!--
        Same convention as the offers page: a 404 on /quotes means this board
        is older than the feature, which is a fact about the server rather
        than a fault anybody can act on.
      -->
      <p class="muted">
        This connected server does not support currency exchange. The Rust
        service and current client do; update the server to use this screen.
      </p>
    } @else {
      <p class="lede">
        Trade coins for dollars at a rate you choose. Posting a rate moves no
        money — it waits until somebody takes it.
      </p>

      <!-- What you are holding, in both currencies, side by side. -->
      <div class="stats">
        <div class="stat">
          <span>{{ session.balance() }}</span>
          <span class="stat__label">{{ session.balance() === 1 ? 'coin' : 'coins' }}</span>
        </div>
        <div class="stat">
          <span>{{ dollars(myCents()) }}</span>
          <span class="stat__label">dollars</span>
        </div>
        @if (spread(); as s) {
          <div class="stat">
            <span>{{ cents(s.bid) }}–{{ cents(s.ask) }}</span>
            <span class="stat__label">bid / ask</span>
          </div>
        }
      </div>

      @if (loading()) {
        <p class="muted">Loading…</p>
      }

      <!--
        Asks first, cheapest first, because that is the one a buyer reads:
        "what is the least I can pay for a coin right now?"
      -->
      <h3>Coins for sale <span class="muted small">(asks — pay dollars, get coins)</span></h3>
      @if (asks().length === 0) {
        <p class="empty">Nobody is selling coins right now.</p>
      } @else {
        <div class="cards">
          @for (q of asks(); track q.id) {
            <article class="card">
              <h3>{{ q.coins }} {{ q.coins === 1 ? 'coin' : 'coins' }}</h3>
              <p class="card__meta">
                <strong>{{ cents(q.cents_per_coin) }} each</strong>
                · {{ dollars(q.cents) }} in total
              </p>
              <p class="card__desc">
                {{ q.maker_name }} will sell {{ q.coins }}
                {{ q.coins === 1 ? 'coin' : 'coins' }} for {{ dollars(q.cents) }}.
              </p>
              @if (q.expires_at) {
                <p class="card__status">Expires {{ when(q.expires_at) }}.</p>
              }
              <div class="card__actions">
                @if (isMine(q)) {
                  <button class="btn btn--quiet" title="Remove your exchange rate without making a trade" (click)="cancel(q)" [disabled]="busy() !== null">
                    Withdraw
                  </button>
                } @else {
                  <button class="btn" title="Trade dollars for the listed coins after confirmation" (click)="take(q)" [disabled]="busy() !== null">
                    {{ busy() === q.id ? 'Trading…' : 'Buy coins' }}
                  </button>
                }
              </div>
            </article>
          }
        </div>
      }

      <h3>Coins wanted <span class="muted small">(bids — give coins, get dollars)</span></h3>
      @if (bids().length === 0) {
        <p class="empty">Nobody is buying coins right now.</p>
      } @else {
        <div class="cards">
          @for (q of bids(); track q.id) {
            <article class="card">
              <h3>{{ q.coins }} {{ q.coins === 1 ? 'coin' : 'coins' }}</h3>
              <p class="card__meta">
                <strong>{{ cents(q.cents_per_coin) }} each</strong>
                · {{ dollars(q.cents) }} in total
              </p>
              <p class="card__desc">
                {{ q.maker_name }} will pay {{ dollars(q.cents) }} for {{ q.coins }}
                {{ q.coins === 1 ? 'coin' : 'coins' }}.
              </p>
              @if (q.expires_at) {
                <p class="card__status">Expires {{ when(q.expires_at) }}.</p>
              }
              <div class="card__actions">
                @if (isMine(q)) {
                  <button class="btn btn--quiet" title="Remove your exchange rate without making a trade" (click)="cancel(q)" [disabled]="busy() !== null">
                    Withdraw
                  </button>
                } @else {
                  <button class="btn" title="Trade coins for the listed dollars after confirmation" (click)="take(q)" [disabled]="busy() !== null">
                    {{ busy() === q.id ? 'Trading…' : 'Sell coins' }}
                  </button>
                }
              </div>
            </article>
          }
        </div>
      }

      <details class="disclosure">
        <summary>Post your own rate</summary>
        <form (ngSubmit)="post()">
          <label>
            I want to
            <select name="side" [(ngModel)]="side">
              <option value="ASK">sell coins for dollars</option>
              <option value="BID">buy coins with dollars</option>
            </select>
          </label>
          <label>
            How many coins
            <input name="coins" type="number" min="1" step="1" [(ngModel)]="coins" required />
          </label>
          <label>
            Cents per coin
            <input
              name="rate"
              type="number"
              min="1"
              step="1"
              [(ngModel)]="rate"
              required
              placeholder="25"
            />
          </label>
          @if (preview(); as p) {
            <p class="muted small">{{ p }}</p>
          }
          <button class="btn" title="Publish this rate for another household member to take" type="submit" [disabled]="posting()">
            {{ posting() ? 'Posting…' : 'Post rate' }}
          </button>
        </form>
      </details>

      @if (settled().length > 0) {
        <details class="disclosure">
          <summary>Rates that are no longer live ({{ settled().length }})</summary>
          <div class="cards">
            @for (q of settled(); track q.id) {
              <article class="card card--sold">
                <h3>{{ q.coins }} at {{ cents(q.cents_per_coin) }}</h3>
                <p class="card__meta">
                  {{ q.side === 'ASK' ? 'sale' : 'purchase' }} ·
                  <span class="tag">{{ q.status.toLowerCase() }}</span>
                </p>
                @if (q.taker_name) {
                  <p class="card__desc">Traded with {{ q.taker_name }}.</p>
                }
              </article>
            }
          </div>
        </details>
      }
    }
  `,
})
export class ForexPage {
  private readonly api = inject(NanacoinService);
  private readonly toasts = inject(Toasts);
  private readonly dialogs = inject(Dialogs);
  protected readonly mastodon = inject(Mastodon);
  protected readonly session = inject(Session);

  protected readonly quotes = signal<Quote[]>([]);
  protected readonly loading = signal(false);
  protected readonly posting = signal(false);

  /** True once the server has told us it has no /quotes endpoint. */
  protected readonly unsupported = signal(false);

  /** The quote being acted on, so only its own button goes busy. */
  protected readonly busy = signal<string | null>(null);

  protected side: QuoteSide = 'ASK';
  protected coins: number | null = null;
  protected rate: number | null = null;
  protected notifyTrade = false;
  protected notificationAllCaps = false;

  /**
   * Dollars held, in cents.
   *
   * Absent on a server predating the currency tags, which is not zero - it is
   * "this server does not track dollars". Both render as $0.00 and that is
   * fine: a server without dollars has none to show.
   */
  protected readonly myCents = computed(() => this.session.me()?.usd_cents ?? 0);

  private readonly live = computed(() => this.quotes().filter((q) => q.live));

  /** Sellers, cheapest first. The server already orders these; re-sorted here
   *  so a cached or reordered response still reads correctly. */
  protected readonly asks = computed(() =>
    this.live()
      .filter((q) => q.side === 'ASK')
      .sort((a, b) => a.cents_per_coin - b.cents_per_coin),
  );

  /** Buyers, best price first. */
  protected readonly bids = computed(() =>
    this.live()
      .filter((q) => q.side === 'BID')
      .sort((a, b) => b.cents_per_coin - a.cents_per_coin),
  );

  /** Everything filled, cancelled or expired: history, folded away. */
  protected readonly settled = computed(() => this.quotes().filter((q) => !q.live));

  /**
   * The best bid and the best ask, when both sides exist.
   *
   * Shown as one stat because the gap between them is the number that says
   * whether this is a market at all. Two asks and no bids is a shop.
   */
  protected readonly spread = computed(() => {
    const ask = this.asks()[0];
    const bid = this.bids()[0];
    if (!ask || !bid) return null;
    return { ask: ask.cents_per_coin, bid: bid.cents_per_coin };
  });

  /**
   * What the form would do, in words, before it is submitted.
   *
   * A method rather than a computed(): it reads `side`, `coins` and `rate`,
   * which are plain ngModel fields and not signals. A computed over
   * non-signals memoises its first answer and then never changes - so this
   * line would have frozen on whatever it said first while the person typed.
   */
  protected preview(): string | null {
    const coins = Number(this.coins);
    const rate = Number(this.rate);
    if (!Number.isInteger(coins) || coins <= 0) return null;
    if (!Number.isInteger(rate) || rate <= 0) return null;
    const total = this.dollars(coins * rate);
    const unit = coins === 1 ? 'coin' : 'coins';
    return this.side === 'ASK'
      ? `You give ${coins} ${unit}, you get ${total}.`
      : `You pay ${total}, you get ${coins} ${unit}.`;
  }

  constructor() {
    void this.load();
  }

  protected isMine(q: Quote): boolean {
    return q.maker === this.session.me()?.account;
  }

  /** Cents as a price tag: 25 -> 25c, 150 -> $1.50. Under a dollar reads
   *  better in cents, which is where most of these rates will sit. */
  protected cents(n: number): string {
    return n < 100 ? `${n}c` : this.dollars(n);
  }

  protected dollars(n: number): string {
    const sign = n < 0 ? '-' : '';
    const abs = Math.abs(n);
    return `${sign}$${Math.floor(abs / 100)}.${String(abs % 100).padStart(2, '0')}`;
  }

  protected when(seconds: number): string {
    return new Date(seconds * 1000).toLocaleString();
  }

  protected async load(): Promise<void> {
    this.loading.set(true);
    try {
      const page = await this.api.quotes();
      this.quotes.set(page.quotes);
      this.unsupported.set(false);
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) {
        this.unsupported.set(true);
      } else {
        this.toasts.fromError(e);
      }
    } finally {
      this.loading.set(false);
    }
  }

  protected async post(): Promise<void> {
    if (this.posting()) return;

    const coins = Number(this.coins);
    const rate = Number(this.rate);
    if (!Number.isInteger(coins) || coins <= 0) {
      this.toasts.error('Enter a whole number of coins.');
      return;
    }
    if (!Number.isInteger(rate) || rate <= 0) {
      this.toasts.error('Enter a rate in whole cents.');
      return;
    }
    // Checked here as a courtesy, not as the rule: the server refuses an
    // overdraft either way, and it is the one that knows the real balance.
    // Catching it here just saves a round trip and gives a clearer sentence.
    if (this.side === 'ASK' && coins > this.session.balance()) {
      this.toasts.error(`You only have ${this.session.balance()} coins to sell.`);
      return;
    }

    this.posting.set(true);
    try {
      await this.api.postQuote({ side: this.side, cents_per_coin: rate, coins });
      this.coins = null;
      this.rate = null;
      this.toasts.ok('Rate posted.');
      await this.load();
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.posting.set(false);
    }
  }

  /**
   * Takes a quote, which is the step that moves both currencies.
   *
   * Confirmed first, with both sides of the trade spelled out. A rate is two
   * numbers multiplied together and it is genuinely easy to read "25" as the
   * total rather than the price per coin - so the dialog states the total in
   * dollars and the count in coins, in the direction this person is going.
   */
  protected async take(q: Quote): Promise<void> {
    if (this.busy()) return;

    // Taking an ask means buying: they hand over coins, you hand over cash.
    const buying = q.side === 'ASK';
    const unit = q.coins === 1 ? 'coin' : 'coins';
    const ok = await this.dialogs.confirm({
      title: buying ? 'Buy these coins?' : 'Sell these coins?',
      message: `At ${this.cents(q.cents_per_coin)} per coin.`,
      detail: buying
        ? [
            `You pay ${this.dollars(q.cents)}`,
            `You get ${q.coins} ${unit}`,
            `From ${q.maker_name}`,
          ]
        : [
            `You give ${q.coins} ${unit}`,
            `You get ${this.dollars(q.cents)}`,
            `To ${q.maker_name}`,
          ],
      confirmLabel: buying ? 'Buy' : 'Sell',
    });
    if (ok === null) return;

    this.busy.set(q.id);
    try {
      await this.api.takeQuote(q.id, newIdempotencyKey());
      this.toasts.ok(buying ? 'Bought.' : 'Sold.');
      if (this.notifyTrade) {
        const recipient = this.session.household().find((u) => u.account === q.maker);
        if (!recipient?.mastodon_id) {
          this.toasts.error(`${q.maker_name} has no registered Mastodon ID; the exchange still completed.`);
        } else {
          let message = `Your NanaCoin exchange for ${q.coins} ${unit} at ${this.cents(q.cents_per_coin)} each was accepted.`;
          if (this.notificationAllCaps) message = message.toLocaleUpperCase();
          try { await this.mastodon.sendDirect(recipient, message); }
          catch (e) { this.toasts.error(`Exchange completed, but the private message failed: ${e instanceof Error ? e.message : 'Mastodon error'}`); }
        }
      }
      await this.session.refresh();
      await this.load();
    } catch (e) {
      this.toasts.fromError(e);
      // Whatever went wrong, the book is now stale - somebody else may have
      // taken it first, which is the commonest failure here by far.
      await this.load();
    } finally {
      this.busy.set(null);
    }
  }

  protected async cancel(q: Quote): Promise<void> {
    if (this.busy()) return;
    this.busy.set(q.id);
    try {
      await this.api.cancelQuote(q.id);
      this.toasts.ok('Withdrawn.');
      await this.load();
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.busy.set(null);
    }
  }
}
