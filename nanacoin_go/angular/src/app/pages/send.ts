import { Component, computed, inject, resource, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute } from '@angular/router';

import { EconomicKind, EconomicUnit, User } from '../api/models';
import { Mastodon } from '../api/mastodon';
import { NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Toasts } from '../ui/toasts';
import { Dialogs } from '../ui/dialog';
import { toggleCaps } from './message-caps';
import { validQuantity } from '../catalog/economics';

interface RecentSend {
  id: string;
  description: string;
  recipient: string;
  recipientAccount: string;
  amount: number;
  createdAt: number;
}

@Component({
  selector: 'app-send',
  imports: [FormsModule],
  template: `
    <h1>Send</h1>

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
        @if (amount !== null) {
          <label>
            What kind of exchange is this?
            <select name="economicKind" [(ngModel)]="economicKind" required>
              <option value="" disabled>Choose one</option>
              <option value="LABOR">Labor</option>
              <option value="GOOD">Good</option>
              <option value="GIFT">Gift</option>
              <option value="OTHER">Other</option>
            </select>
          </label>
          <label>
            Quantity
            <input name="quantity" [(ngModel)]="quantity" inputmode="decimal" required />
          </label>
          <label>
            Unit
            <select name="unit" [(ngModel)]="unit">
              <option value="EACH">each</option><option value="BATCH">batch</option>
              <option value="TASK">task</option><option value="MINUTE">minute</option>
              <option value="HOUR">hour</option><option value="GRAM">gram</option>
              <option value="KILOGRAM">kilogram</option><option value="MILLILITER">milliliter</option>
              <option value="LITER">liter</option><option value="LOAD">load</option>
              <option value="OTHER">other</option>
            </select>
          </label>
        }
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
  private readonly dialogs = inject(Dialogs);
  protected readonly session = inject(Session);
  protected readonly mastodon = inject(Mastodon);

  protected to = '';
  protected amount: number | null = null;
  protected memo = '';
  protected sendDm = false;
  protected economicKind: EconomicKind | '' = '';
  protected quantity = '1';
  protected unit: EconomicUnit = 'EACH';
  protected allCaps = false;
  private memoBeforeCaps = '';
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
          recipientAccount: recipient.account,
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
    if (amount !== null && !this.economicKind) {
      this.toasts.error('Choose what kind of exchange this payment is for.');
      return;
    }
    if (amount !== null && !validQuantity(this.quantity)) {
      this.toasts.error('Quantity must be a positive decimal with at most three places.');
      return;
    }
    const recipient = this.recipient();
    const wantsDm = amount === null || this.sendDm;
    if (wantsDm && !this.memo.trim()) { this.toasts.error('Write a message first.'); return; }
    if (wantsDm && !recipient?.mastodon_id) {
      this.toasts.error('That person has not registered a Mastodon ID with NanaCoin.'); return;
    }
    if (wantsDm && !this.mastodon.connected()) { this.toasts.error('Connect your Mastodon account first.'); return; }

    if (amount !== null) {
      const duplicate = this.recentSends().find((sent) =>
        sent.recipientAccount === this.to
        && sent.amount === amount
        && sent.description.trim() === this.memo.trim());
      if (duplicate) {
        const answer = await this.dialogs.confirm({
          title: 'Send the same payment again?',
          message: 'This matches a recent transaction. Continue only if you mean to pay twice.',
          detail: [
            `${amount} ${amount === 1 ? 'coin' : 'coins'} to ${duplicate.recipient}`,
            this.memo.trim() || 'No description',
          ],
          confirmLabel: 'Send again',
        });
        if (answer === null) return;
      }
    }

    this.busy.set(true);
    // Generated once per attempted transfer, before the request. A retry with
    // this same key returns the original transaction instead of sending twice.
    const key = newIdempotencyKey();
    let paymentSent = false;
    try {
      if (amount !== null) {
        await this.api.transfer(this.to, amount, this.memo.trim(), key, {
          economic_kind: this.economicKind as EconomicKind,
          quantity: this.quantity,
          unit: this.unit,
        });
        paymentSent = true;
        this.amount = null;
        this.economicKind = '';
        this.quantity = '1';
        this.unit = 'EACH';
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

  private async finishMastodon(code: string, state: string): Promise<void> {
    try { this.toasts.ok(`Connected Mastodon as ${await this.mastodon.finish(code, state)}.`); }
    catch (e) { this.toasts.error(e instanceof Error ? e.message : 'Could not finish Mastodon sign-in.'); }
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
