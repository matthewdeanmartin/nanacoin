import { Component, computed, inject, resource, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute } from '@angular/router';

import { User } from '../api/models';
import { Mastodon } from '../api/mastodon';
import { NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Toasts } from '../ui/toasts';
import { toggleCaps } from './message-caps';

interface RecentSend {
  id: string;
  description: string;
  recipient: string;
  amount: number;
  createdAt: number;
}

@Component({
  selector: 'app-send',
  imports: [FormsModule],
  template: `
    <h1>Send</h1>

    <section class="panel" aria-labelledby="mastodon-title">
      <h2 id="mastodon-title">Private Mastodon messages</h2>
      @if (mastodon.connected()) {
        <p>Connected as <strong>{{ mastodon.account() }}</strong>.</p>
        <button class="btn btn--quiet" type="button" (click)="disconnectMastodon()">Disconnect this browser</button>
      } @else {
        <p class="muted small">Your token stays in this browser. NanaCoin stores only your registered Mastodon ID.</p>
        <form (ngSubmit)="connectMastodon()">
          <label>
            Your Mastodon server
            <input name="mastodonServer" [(ngModel)]="mastodonServer" placeholder="mastodon.social" required />
          </label>
          <button class="btn btn--quiet" type="submit" [disabled]="connecting()">
            {{ connecting() ? 'Connecting…' : 'Connect Mastodon' }}
          </button>
        </form>
      }
    </section>

    @if (session.recipients().length === 0) {
      <p class="muted">
        There is nobody else in the household yet. Nana adds members from the
        Household tab.
      </p>
    } @else {
      <form (ngSubmit)="send()">
        <label>
          To
          <select name="to" [(ngModel)]="to" required>
            <option value="" disabled>Choose someone</option>
            @for (u of session.recipients(); track u.id) {
              <option [value]="u.account">{{ u.display_name }}</option>
            }
          </select>
        </label>
        <label>
          Amount
          <input name="amount" type="number" min="1" step="1" [(ngModel)]="amount"
                 placeholder="Optional for a message" />
        </label>
        <label>
          Message / what for?
          <input name="memo" [ngModel]="memo" (ngModelChange)="memo = allCaps ? $event.toLocaleUpperCase() : $event" maxlength="140"
                 placeholder="Taking out the trash" />
        </label>
        <label class="checkbox">
          <input name="allCaps" type="checkbox" [ngModel]="allCaps" (ngModelChange)="setAllCaps($event)" />
          ALL CAPS
        </label>
        @if (amount !== null) {
          <label class="checkbox">
            <input name="sendDm" type="checkbox" [(ngModel)]="sendDm" />
            Also send this as a private Mastodon message
          </label>
        }
        <button class="btn" title="Send coins, a private Mastodon message, or both" type="submit" [disabled]="busy()">
          {{ busy() ? 'Sending…' : (amount === null ? 'Send private message' : 'Send') }}
        </button>
      </form>

      <p class="muted small">
        You have {{ session.balance() }} {{ session.balance() === 1 ? 'coin' : 'coins' }}.
      </p>
    }

    <section class="panel" aria-labelledby="share-title">
      <h2 id="share-title">Tell friends and family</h2>
      <label>
        Post text
        <input [ngModel]="shareText" (ngModelChange)="shareText = shareAllCaps ? $event.toLocaleUpperCase() : $event" maxlength="300" />
      </label>
      <label class="checkbox">
        <input type="checkbox" [ngModel]="shareAllCaps" (ngModelChange)="setShareAllCaps($event)" />
        ALL CAPS
      </label>
      <button class="btn btn--quiet" type="button" (click)="shareFacebook()">Share on Facebook</button>
      <p class="muted small">The draft is copied, then Facebook opens its posting dialog. Other platform intents remain on the roadmap.</p>
    </section>

    <section aria-labelledby="recent-sends-title">
      <h2 id="recent-sends-title">Recently sent</h2>
      @if (history.isLoading()) {
        <p class="muted" role="status">Loading recent sends…</p>
      } @else if (recentSends().length === 0) {
        <p class="muted">Nothing sent yet.</p>
      } @else {
        <div class="history">
          @for (sent of recentSends(); track sent.id) {
            <article class="txn txn--out" data-keyboard-row tabindex="-1">
              <div class="txn__main">
                <span class="txn__desc">{{ sent.description || 'Transfer' }}</span>
                <span class="txn__who">to {{ sent.recipient }}</span>
              </div>
              <div class="txn__side">
                <span class="txn__amount">−{{ sent.amount }}</span>
                <time class="txn__when" [attr.datetime]="isoTime(sent.createdAt)">{{ when(sent.createdAt) }}</time>
              </div>
            </article>
          }
        </div>
      }
    </section>
  `,
})
export class SendPage {
  private readonly api = inject(NanacoinService);
  private readonly toasts = inject(Toasts);
  protected readonly session = inject(Session);
  protected readonly mastodon = inject(Mastodon);

  protected to = '';
  protected amount: number | null = null;
  protected memo = '';
  protected sendDm = false;
  protected allCaps = false;
  private memoBeforeCaps = '';
  protected mastodonServer = 'mastodon.social';
  protected readonly connecting = signal(false);
  protected shareText = "I'm using NanaCoin with my friends and family.";
  protected shareAllCaps = false;
  private shareBeforeCaps = '';
  protected readonly busy = signal(false);
  protected readonly history = resource({
    params: () => ({ account: this.session.me()?.account }),
    loader: ({ params }) => params.account
      ? this.api.accountHistory(params.account, 100)
      : Promise.resolve({ account: '', balance: 0, transactions: [] }),
  });

  protected readonly recentSends = computed<RecentSend[]>(() => {
    const account = this.session.me()?.account;
    if (!account) return [];
    return (this.history.value()?.transactions ?? [])
      .flatMap((txn) => {
        if (txn.kind !== 'TRANSFER' || txn.reference || txn.reversed_by) return [];
        const delta = txn.postings
          .filter((posting) => posting.account === account)
          .reduce((sum, posting) => sum + posting.amount, 0);
        const recipient = txn.postings.find((posting) => posting.account !== account);
        if (delta >= 0 || !recipient) return [];
        return [{
          id: txn.id,
          description: txn.description,
          recipient: recipient.name,
          amount: -delta,
          createdAt: txn.created_at,
        }];
      })
      .slice(0, 5);
  });

  /**
   * Repeating a past transaction fills the form in and stops.
   *
   * Deliberately not sent automatically. "Repeat" reads as a shortcut for
   * typing the same thing again, not as a second payment already on its way,
   * and a mis-tap on a history row must not move money. The query parameters
   * are a suggestion: the amount is still validated below and the recipient
   * still has to be someone this user can pay.
   */
  constructor() {
    const q = inject(ActivatedRoute).snapshot.queryParamMap;

    const to = q.get('to');
    // Only accept a recipient who is actually payable now. A repeat of a
    // transfer to someone since disabled would otherwise preselect an option
    // that is not in the list and submit an account the server refuses.
    if (to && this.session.recipients().some((u) => u.account === to)) this.to = to;

    const amount = Number(q.get('amount'));
    if (Number.isInteger(amount) && amount > 0) this.amount = amount;

    this.memo = q.get('memo') ?? '';
    const code = q.get('mastodon_code');
    const state = q.get('mastodon_state');
    if (code && state) {
      history.replaceState(null, '', location.pathname + location.search + '#/send');
      void this.finishMastodon(code, state);
    }
  }

  protected async send(): Promise<void> {
    if (this.busy()) return;

    if (!this.to) {
      // An empty <select required> passes browser validation when it has no
      // options, so this is checked rather than assumed.
      this.toasts.error('Choose who the coins are for.');
      return;
    }
    const amount = this.amount === null ? null : Number(this.amount);
    if (amount !== null && (!Number.isInteger(amount) || amount <= 0)) {
      this.toasts.error('Enter a whole number of coins, or leave the amount empty for a message.');
      return;
    }
    const recipient = this.recipient();
    const wantsDm = amount === null || this.sendDm;
    if (wantsDm && !this.memo.trim()) { this.toasts.error('Write a message first.'); return; }
    if (wantsDm && !recipient?.mastodon_id) {
      this.toasts.error('That person has not registered a Mastodon ID with NanaCoin.'); return;
    }
    if (wantsDm && !this.mastodon.connected()) { this.toasts.error('Connect your Mastodon account first.'); return; }

    this.busy.set(true);
    // Generated once per attempted transfer, before the request. A retry with
    // this same key returns the original transaction instead of sending twice.
    const key = newIdempotencyKey();
    let paymentSent = false;
    try {
      if (amount !== null) {
        await this.api.transfer(this.to, amount, this.memo.trim(), key);
        paymentSent = true;
        this.amount = null;
        await this.session.refresh();
        this.history.reload();
      }
      if (wantsDm && recipient) {
        try { await this.mastodon.sendDirect(recipient, this.memo); }
        catch (e) {
          const detail = e instanceof Error ? e.message : 'Mastodon error';
          this.toasts.error(paymentSent ? `Coins sent, but the private message failed: ${detail}` : `Private message failed: ${detail}`);
          return;
        }
      }
      this.memo = '';
      this.sendDm = false;
      this.allCaps = false;
      this.toasts.ok(amount === null ? 'Private message sent.' : wantsDm ? 'Coins and private message sent.' : 'Coins sent.');
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.busy.set(false);
    }
  }

  private recipient(): User | undefined {
    return this.session.recipients().find((u) => u.account === this.to);
  }

  protected setAllCaps(on: boolean): void {
    const next = toggleCaps(this.memo, this.allCaps, on, this.memoBeforeCaps);
    this.memo = next.text;
    this.memoBeforeCaps = next.saved;
    this.allCaps = on;
  }

  protected setShareAllCaps(on: boolean): void {
    const next = toggleCaps(this.shareText, this.shareAllCaps, on, this.shareBeforeCaps);
    this.shareText = next.text;
    this.shareBeforeCaps = next.saved;
    this.shareAllCaps = on;
  }

  protected async connectMastodon(): Promise<void> {
    if (this.connecting()) return;
    this.connecting.set(true);
    try { await this.mastodon.connect(this.mastodonServer); }
    catch (e) { this.connecting.set(false); this.toasts.error(e instanceof Error ? e.message : 'Could not connect Mastodon.'); }
  }

  private async finishMastodon(code: string, state: string): Promise<void> {
    this.connecting.set(true);
    try { this.toasts.ok(`Connected Mastodon as ${await this.mastodon.finish(code, state)}.`); }
    catch (e) { this.toasts.error(e instanceof Error ? e.message : 'Could not finish Mastodon sign-in.'); }
    finally { this.connecting.set(false); }
  }

  protected disconnectMastodon(): void { this.mastodon.disconnect(); this.toasts.ok('Mastodon disconnected from this browser.'); }

  protected shareFacebook(): void {
    const textarea = document.createElement('textarea');
    textarea.value = this.shareText.trim();
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand('copy');
    textarea.remove();
    const params = new URLSearchParams({
      u: 'https://github.com/matthewdeanmartin/nanacoin', quote: this.shareText.trim(),
    });
    window.open(`https://www.facebook.com/sharer/sharer.php?${params}`, '_blank', 'noopener,noreferrer');
  }

  protected when(unixSeconds: number): string {
    return new Date(unixSeconds * 1000).toLocaleString(undefined, {
      month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit',
    });
  }

  protected isoTime(unixSeconds: number): string {
    return new Date(unixSeconds * 1000).toISOString();
  }
}
