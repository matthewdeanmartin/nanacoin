import {
  Component,
  DestroyRef,
  computed,
  inject,
  input,
  output,
  resource,
  signal,
} from '@angular/core';
import { DatePipe } from '@angular/common';
import { Fulfillment, FulfillmentAction } from '../api/models';
import { NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { Toasts } from '../ui/toasts';

export function fulfillmentLabel(f: Pick<Fulfillment, 'kind' | 'status'>): string {
  if (f.status === 'REVERSED') return 'Payment reversed';
  const item = f.kind === 'WORK' ? 'Work' : f.kind === 'CASH' ? 'Cash' : 'Goods';
  if (f.status === 'DISPUTED')
    return `Disputed · ${item.toLowerCase()} ${f.kind === 'WORK' ? 'not done' : 'not delivered'}`;
  return f.status === 'TODO'
    ? `TODO · ${item.toLowerCase()} ${f.kind === 'WORK' ? 'to do' : 'to deliver'}`
    : `${item} ${f.kind === 'WORK' ? 'done' : 'delivered'} · claimed`;
}
@Component({
  selector: 'app-fulfillment',
  template: `@if (item(); as f) {
    <p role="status">
      <strong>{{ label(f) }}</strong>
    </p>
    <p class="muted small">{{ f.provider_name }} → {{ f.recipient_name }}</p>
    @if (f.status === 'TODO' && account() === f.provider) {
      <button class="btn btn--small" [disabled]="busy()" (click)="act('COMPLETE')">
        {{
          f.kind === 'WORK'
            ? 'Mark work done'
            : f.kind === 'CASH'
              ? 'Mark cash delivered'
              : 'Mark goods delivered'
        }}
      </button>
    }
    @if ((f.status === 'TODO' || f.status === 'DONE') && account() === f.recipient) {
      <button class="btn btn--quiet btn--small" [disabled]="busy()" (click)="act('DISPUTE')">
        {{
          f.kind === 'WORK'
            ? 'Work not done'
            : f.kind === 'CASH'
              ? 'Cash not delivered'
              : 'Goods not delivered'
        }}
      </button>
    }
    @if (f.status === 'DISPUTED' && isNana()) {
      <button class="btn btn--quiet btn--small" [disabled]="busy()" (click)="reverse()">
        Reverse disputed payment
      </button>
    }
    @if (f.status === 'DISPUTED' && account() === f.recipient) {
      <button class="btn btn--small" [disabled]="busy()" (click)="act('WITHDRAW_DISPUTE')">
        Withdraw dispute · mark done
      </button>
    }
  }`,
})
export class FulfillmentControl {
  readonly item = input<Fulfillment | null>();
  readonly changed = output<void>();
  private readonly session = inject(Session);
  private readonly api = inject(NanacoinService);
  private readonly dialogs = inject(Dialogs);
  private readonly toasts = inject(Toasts);
  protected readonly isNana = this.session.isNana;
  protected readonly account = computed(() => this.session.me()?.account);
  protected readonly busy = signal(false);
  protected readonly label = fulfillmentLabel;
  protected async reverse() {
    const f = this.item();
    if (!f || this.busy()) return;
    const reason = await this.dialogs.prompt({
      title: 'Reverse disputed payment?',
      message: f.description,
      required: true,
      confirmLabel: 'Reverse payment',
    });
    if (reason === null) return;
    this.busy.set(true);
    try {
      await this.api.reverse(f.transaction, reason, newIdempotencyKey());
      await this.session.refresh();
      this.changed.emit();
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.busy.set(false);
    }
  }
  private pending?: { transaction: string; action: FulfillmentAction; reason: string; key: string };
  protected async act(action: FulfillmentAction) {
    const f = this.item();
    if (!f || this.busy()) return;
    let reason = '';
    if (action === 'DISPUTE') {
      const answer = await this.dialogs.prompt({
        title: 'Dispute this transaction',
        message: f.description,
        detail: ['This flags the delivery for review. It does not refund the payment.'],
        required: true,
        placeholder: 'Explain what is missing',
        confirmLabel: 'Submit dispute',
      });
      if (answer === null) return;
      reason = answer.trim();
      if (!reason || new TextEncoder().encode(reason).length > 96) {
        this.toasts.error('Enter a reason of 1–96 bytes.');
        return;
      }
    }
    this.busy.set(true);
    if (
      !this.pending ||
      this.pending.transaction !== f.transaction ||
      this.pending.action !== action ||
      this.pending.reason !== reason
    ) {
      this.pending = { transaction: f.transaction, action, reason, key: newIdempotencyKey() };
    }
    try {
      await this.api.setFulfillment(f.transaction, action, reason, this.pending.key);
      this.pending = undefined;
      this.changed.emit();
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.busy.set(false);
    }
  }
}
@Component({
  selector: 'app-account-todos',
  imports: [FulfillmentControl, DatePipe],
  template: `<section id="my-todos" class="account-section">
    <div class="section-heading">
      <h2>TODO</h2>
      <button class="btn btn--quiet" (click)="book.reload()">Refresh</button>
    </div>
    <p class="muted">Payment is recorded separately from work and delivery.</p>
    @if (book.isLoading()) {
      <p role="status">Loading TODOs…</p>
    }
    @if (book.error()) {
      <p role="alert">Could not load TODOs. Use Refresh to retry.</p>
    }
    @for (group of groups(); track group.title) {
      <h3>{{ group.title }}</h3>
      @for (f of group.items; track f.transaction) {
        <article class="ledger-row">
          <p>
            <strong>{{ f.description }}</strong> · {{ f.transaction }}
          </p>
          <app-fulfillment [item]="f" (changed)="reload()" />
          <details>
            <summary>Recent fulfillment activity</summary>
            @for (u of f.updates; track u.id) {
              <p>
                {{ u.at * 1000 | date: 'medium' }} · {{ u.actor_name }} · {{ u.status }}
                {{ u.reason }}
              </p>
            }
          </details>
        </article>
      } @empty {
        <p class="muted small">None.</p>
      }
    }
  </section>`,
})
export class AccountTodos {
  private readonly api = inject(NanacoinService);
  private readonly session = inject(Session);
  readonly changed = output<void>();
  protected readonly book = resource({
    params: () => this.session.me()?.account,
    loader: () => this.api.fulfillments(),
  });
  protected readonly groups = computed(() => {
    const me = this.session.me()?.account,
      book = this.book.value()?.fulfillments ?? [],
      all = book.filter((f) => f.provider === me || f.recipient === me);
    return [
      ...(this.session.isNana()
        ? [
            {
              title: 'Disputes for Nana to review',
              items: book.filter((f) => f.status === 'DISPUTED'),
            },
          ]
        : []),
      {
        title: 'My work & deliveries',
        items: all.filter(
          (f) => f.provider === me && (f.status === 'TODO' || f.status === 'DISPUTED'),
        ),
      },
      {
        title: 'Waiting for others',
        items: all.filter(
          (f) => f.recipient === me && (f.status === 'TODO' || f.status === 'DISPUTED'),
        ),
      },
      {
        title: 'Completed & reversed',
        items: all
          .filter((f) => f.status === 'DONE' || f.status === 'REVERSED')
          .sort((a, b) => (b.updates.at(-1)?.at ?? 0) - (a.updates.at(-1)?.at ?? 0)),
      },
    ];
  });
  refresh() {
    this.book.reload();
  }
  protected reload() {
    this.refresh();
    this.changed.emit();
  }
  constructor() {
    const timer = setInterval(() => this.book.reload(), 15_000);
    inject(DestroyRef).onDestroy(() => clearInterval(timer));
  }
}
