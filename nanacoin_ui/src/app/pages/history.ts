import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { AccountTodos, FulfillmentControl } from './fulfillment';
import { Money, MoneyPipe } from '../api/money';
import { inject as moneyInject } from '@angular/core';
// Your own transactions, newest first.

import { Component, computed, inject, resource, signal } from '@angular/core';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { AccountCommitments } from './account-commitments';
import { Notebook } from '../ui/notebook';

import { Offer, Quote, Transaction } from '../api/models';
import { ApiError, NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { Toasts } from '../ui/toasts';

/** One row, already reduced to what this account actually experienced. */
interface Row {
  txn: Transaction;
  /** This account's net change: what happened to you, not the gross amount. */
  delta: number;
  /** The other party's display name, where there is a single one. */
  other: string;
  label: string;

  /**
   * The other party's account, when this row is one that could be sent again.
   * Empty when it is not - see repeatable below.
   */
  otherAccount: string;

  /**
   * Whether "Repeat" makes sense for this row.
   *
   * Only an ordinary outgoing transfer. Repeating an ISSUE would mean minting
   * more coin, a PURCHASE belongs to a listing that has already sold, and a
   * REVERSAL is a correction to one specific transaction - none of those is
   * "send that again", which is what the button offers.
   */
  repeatable: boolean;
}

@Component({
  selector: 'app-history',
  imports: [MoneyPipe, RouterLink, Notebook, AccountCommitments, AccountTodos, FulfillmentControl],
  template: `
    <h1>My Account</h1>
    <p class="lede">Your balances, loans, debts, lotto results, and financial activity.</p>

    <p><strong>{{session.balance() | nc}} NC available</strong> · <a routerLink="/messages">Open Mail</a></p>
    <nav class="account-tabs" role="tablist" aria-label="My Account sections">
      @for (tab of tabs; track tab.id) {
        <button class="btn btn--quiet" type="button" role="tab" [id]="'tab-'+tab.id"
          [attr.aria-selected]="activeTab()===tab.id" [attr.aria-controls]="'panel-'+tab.id"
          [attr.tabindex]="activeTab()===tab.id ? 0 : -1" (keydown)="tabKey($event,tab.id)" (click)="activeTab.set(tab.id)">
          {{tab.label}} ({{tab.id==='todos' ? todos.count() ?? '…' : tab.id==='loans' ? commitments.loanCount() ?? '…' : tab.id==='lotto' ? commitments.lottoCount() ?? '…' : tab.id==='offers' ? offers.hasValue() ? myOffers().length : '…' : tab.id==='forex' ? quotes.hasValue() ? myBids().length : '…' : history.hasValue() ? rows().length : '…'}})
        </button>
      }
    </nav>
    <div id="panel-todos" role="tabpanel" aria-labelledby="tab-todos" [hidden]="activeTab()!=='todos'" tabindex="0">
      <app-account-todos #todos (changed)="history.reload()" />
    </div>
    <div [hidden]="activeTab()!=='loans' && activeTab()!=='lotto'">
      <app-account-commitments #commitments [active]="activeTab()" />
    </div>

    <section id="panel-offers" class="account-section" role="tabpanel" aria-labelledby="tab-offers" [hidden]="activeTab()!=='offers'" tabindex="0">
      <div class="section-heading">
        <h2>My Offers</h2>
        <a routerLink="/offers">View all offers</a>
      </div>
      @if (offers.isLoading()) {
        <p class="muted">Loading offers…</p>
      } @else if (offers.error()) {
        <p class="muted">Offers are not supported by this server.</p>
      } @else if (myOffers().length === 0) {
        <p class="empty">You have no offers yet.</p>
      } @else {
        <div class="account-summary-list">
          @for (o of myOffers(); track o.id) {
            <a routerLink="/offers">
              <span>{{ o.listing_title || 'Offer' }}</span>
              <span>{{ o.amount | nc }} coins · {{ o.status.toLowerCase() }}</span>
            </a>
          }
        </div>
      }
    </section>

    <section id="panel-forex" class="account-section" role="tabpanel" aria-labelledby="tab-forex" [hidden]="activeTab()!=='forex'" tabindex="0">
      <div class="section-heading">
        <h2>My Forex Bids</h2>
        <a routerLink="/forex">Open Forex</a>
      </div>
      @if (quotes.isLoading()) {
        <p class="muted">Loading bids…</p>
      } @else if (quotes.error()) {
        <p class="muted">Foreign exchange is not supported by this server.</p>
      } @else if (myBids().length === 0) {
        <p class="empty">You have no forex bids.</p>
      } @else {
        <div class="account-summary-list">
          @for (q of myBids(); track q.id) {
            <a routerLink="/forex">
              <span>{{ q.coins | nc }} coins at {{ q.cents_per_coin }}¢ each</span>
              <span>{{ q.status.toLowerCase() }}</span>
            </a>
          }
        </div>
      }
    </section>

    <section id="panel-transactions" class="account-section" role="tabpanel" aria-labelledby="tab-transactions" [hidden]="activeTab()!=='transactions'" tabindex="0">
    <h2>Transactions</h2>

    @if (history.isLoading()) {
      <p class="muted">Loading…</p>
    } @else if (history.error()) {
      <p class="muted">Could not load your history.</p>
    } @else if (rows().length === 0) {
      <p class="muted">No transactions yet.</p>
    } @else {
      <app-notebook><div class="history">
        @for (r of rows(); track r.txn.id) {
          <div class="txn" data-keyboard-row tabindex="-1" [class.txn--in]="r.delta >= 0" [class.txn--out]="r.delta < 0">
            <div class="txn__main">
              <span class="txn__desc">{{ r.txn.description || r.label }}</span>
              @if (r.other) {
                <span class="txn__who">
                  {{ r.delta < 0 ? 'to' : 'from' }} {{ r.other }}
                </span>
              }
            </div>

            <div class="txn__side">
              <span class="txn__amount">{{ r.delta >= 0 ? '+' : '' }}{{ r.delta | nc }}</span>
              <span class="txn__when">{{ when(r.txn.created_at) }}</span>
            </div>

            <app-fulfillment [item]="r.txn.fulfillment" (changed)="history.reload(); todos.refresh()" />
            @if (r.txn.reversed_by) {
              <span class="tag tag--warn">reversed</span>
            }
            @if (r.txn.kind === 'REVERSAL') {
              <span class="tag">correction</span>
            }

            <div class="txn__actions">
              @if (r.repeatable) {
                <a
                  class="btn btn--quiet btn--small"
                  title="Start another transfer with the same recipient, amount, and memo"
                  routerLink="/send"
                  [queryParams]="{ to: r.otherAccount, amount: money.input(-r.delta), memo: r.txn.description }"
                >Repeat</a>
              }

              <!--
                Reversing is Nana's, and the server enforces that regardless of
                this check - which is only here so everyone else is not shown a
                button that would 403. An already-reversed transaction has no
                button at all: the correction exists, and a second one would
                undo the undo.
              -->
              @if (session.isNana() && !r.txn.reversed_by && r.txn.kind !== 'REVERSAL' && !r.txn.reference?.startsWith('loan-') && !r.txn.reference?.startsWith('lotto-') && !r.txn.reference?.startsWith('nickle:')) {
                <button
                  class="btn btn--quiet btn--small"
                  type="button"
                  title="Append a correcting transaction; the original remains in your history"
                  [disabled]="reversing() === r.txn.id"
                  (click)="reverse(r.txn)"
                >{{ reversing() === r.txn.id ? 'Reversing…' : 'Reverse' }}</button>
              }
            </div>
          </div>
        }
      </div></app-notebook>
    }
    </section>
  `,
})
export class HistoryPage {
  protected readonly activeTab=signal('todos');
  constructor() { inject(ActivatedRoute).queryParamMap.pipe(takeUntilDestroyed()).subscribe(params=> { const tab=params.get('tab'); if(tab && this.tabs.some(t=>t.id===tab)) this.activeTab.set(tab); }); }
  protected readonly tabs=[{id:'todos',label:'TODO'},{id:'loans',label:'Loans & debts'},{id:'lotto',label:'Lotto'},{id:'transactions',label:'Transactions'},{id:'offers',label:'My offers'},{id:'forex',label:'My forex bids'}];
  protected tabKey(event: KeyboardEvent, id: string) {
    const i=this.tabs.findIndex(t=>t.id===id);
    const next=event.key==='ArrowRight' ? (i+1)%this.tabs.length : event.key==='ArrowLeft' ? (i+this.tabs.length-1)%this.tabs.length : event.key==='Home' ? 0 : event.key==='End' ? this.tabs.length-1 : -1;
    if(next<0) return; event.preventDefault();this.activeTab.set(this.tabs[next].id);document.getElementById('tab-'+this.tabs[next].id)?.focus();
  }

  protected readonly money = moneyInject(Money);
  private readonly api = inject(NanacoinService);
  private readonly toasts = inject(Toasts);
  private readonly dialogs = inject(Dialogs);
  protected readonly session = inject(Session);

  /** The transaction currently being reversed, so only its button is busy. */
  protected readonly reversing = signal<string | null>(null);

  /**
   * Reloads whenever the signed-in account changes. The balance in the top bar
   * comes from Session; this is the only page that needs the transaction list,
   * so it fetches its own rather than putting it in shared state.
   */
  protected readonly history = resource({
    params: () => ({ account: this.session.me()?.account }),
    loader: ({ params }) =>
      params.account
        ? this.api.accountHistory(params.account, 50)
        : Promise.resolve({ account: '', balance: 0, transactions: [] }),
  });

  protected readonly offers = resource({
    params: () => this.session.me()?.account,
    loader: async ({ params }) => {
      if (!params) return [] as Offer[];
      try { return (await this.api.offers()).offers; }
      catch (e) { if (e instanceof ApiError && e.status === 404) return []; throw e; }
    },
  });

  protected readonly quotes = resource({
    params: () => this.session.me()?.account,
    loader: async ({ params }) => {
      if (!params) return [] as Quote[];
      try { return (await this.api.quotes()).quotes; }
      catch (e) { if (e instanceof ApiError && e.status === 404) return []; throw e; }
    },
  });

  /** Offers made by this account, plus offers awaiting its decision. */
  protected readonly myOffers = computed(() => {
    const account = this.session.me()?.account;
    return (this.offers.value() ?? [])
      .filter((o) => o.offerer === account || o.listing_owner === account)
      .sort((a, b) => b.updated_at - a.updated_at);
  });

  protected readonly myBids = computed(() => {
    const account = this.session.me()?.account;
    return (this.quotes.value() ?? [])
      .filter((q) => q.maker === account && q.side === 'BID')
      .sort((a, b) => b.updated_at - a.updated_at);
  });

  protected readonly rows = computed<Row[]>(() => {
    const account = this.session.me()?.account;
    const txns = (this.history.value()?.transactions ?? []).filter(t => t.kind !== 'MESSAGE');
    if (!account) return [];

    return txns.map((txn) => {
      // Sum this account's own postings: a transaction may touch it more than
      // once, and the net effect is what the user experienced.
      let delta = 0;
      for (const p of txn.postings) if (p.account === account) delta += p.amount;

      const other = txn.postings.find((p) => p.account !== account);
      return {
        txn,
        delta,
        other: other?.name ?? '',
        label: kindLabel(txn.kind),
        otherAccount: other?.account ?? '',
        repeatable: txn.kind === 'TRANSFER' && !txn.reference && delta < 0 && !!other?.account,
      };
    });
  });

  /**
   * Appends the mirror transaction that undoes one, after asking why.
   *
   * The reason is required rather than optional: it becomes the description of
   * a permanent ledger entry, and "Nana reversed it" without the because is
   * the audit trail this project exists to avoid. Cancelling the prompt
   * cancels the reversal.
   */
  protected async reverse(txn: Transaction): Promise<void> {
    if (this.reversing()) return;

    const reason = await this.dialogs.prompt({
      title: 'Reverse this transaction?',
      message: 'This appends a correction. Nothing is deleted, and the original stays in the history.',
      detail: [
        txn.description || kindLabel(txn.kind),
        `${txn.postings.length} postings, ${this.when(txn.created_at)}`,
      ],
      placeholder: 'Why is this being reversed?',
      confirmLabel: 'Reverse',
      required: true,
      danger: true,
    });
    if (reason === null) return;

    this.reversing.set(txn.id);
    try {
      await this.api.reverse(txn.id, reason, newIdempotencyKey());
      this.toasts.ok('Reversed.');
      // Both the balances and this list changed, so refresh the shared state
      // and re-read the page's own resource.
      await this.session.refresh();
      this.history.reload();
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.reversing.set(null);
    }
  }

  /**
   * The application uses hash routing, so a literal href="#section" is a
   * route change rather than an in-page anchor. Keep section navigation out
   * of the URL and scroll explicitly instead.
   */
  protected scrollTo(id: string): void {
    document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  protected when(unixSeconds: number): string {
    return new Date(unixSeconds * 1000).toLocaleString(undefined, {
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit',
    });
  }
}

function kindLabel(kind: Transaction['kind']): string {
  switch (kind) {
    case 'ISSUE':
      return 'Issued';
    case 'RETIRE':
      return 'Retired';
    case 'PURCHASE':
      return 'Purchase';
    case 'REVERSAL':
      return 'Correction';
    default:
      return 'Transfer';
  }
}
