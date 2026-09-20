// The household economy: what the money supply has done, what changed hands,
// and how each person's balance moved.
//
// The ledger is shared with the household, like the paper notebook in the
// NanaCoin premise. Everyone can audit the supply, GDP, and balance movement.
//
// All the arithmetic runs here rather than on the board. See series.ts.

import { Component, computed, inject, resource, signal } from '@angular/core';

import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { LineChart } from '../economy/line-chart';
import {
  Bucket,
  Series,
  balanceSeries,
  gdpSeries,
  moneySupplySeries,
} from '../economy/series';

/** How many transactions to ask for. The server caps this itself. */
const LEDGER_LIMIT = 365;

@Component({
  selector: 'app-economy',
  imports: [LineChart],
  template: `
    <h1>Economy</h1>

    @if (data.isLoading()) {
      <p class="muted">Loading…</p>
    } @else if (data.error()) {
      <p class="muted">Could not load the economy.</p>
    } @else {
      @if (truncated()) {
        <!--
          The ledger keeps a bounded number of transactions and overwrites the
          oldest, so these charts describe the retained window. Saying so is
          not a footnote: a reader who assumes the left edge is the beginning
          of the household would misread every one of them.
        -->
        <p class="muted small">
          Showing the most recent {{ retained() }} transactions. Older history has
          left the board's memory, so these charts begin partway through.
        </p>
      }

      <div class="chart-controls">
          <label>
            Group by
            <select [value]="bucket()" (change)="bucket.set($any($event.target).value)">
              <option value="day">Day</option>
              <option value="week">Week</option>
            </select>
          </label>
      </div>

        <app-line-chart
          title="Money supply"
          subtitle="Total NanaCoin in circulation. Only Nana issuing or retiring coin moves this line."
          [series]="supply()"
        />

        <app-line-chart
          title="GDP"
          [subtitle]="gdpSubtitle()"
          [series]="gdp()"
        />

        <app-line-chart
          title="Balances"
          subtitle="What each person holds over time."
          [series]="balances()"
        />
    }
  `,
})
export class EconomyPage {
  private readonly api = inject(NanacoinService);
  protected readonly session = inject(Session);

  protected readonly bucket = signal<Bucket>('day');

  /**
   * Every signed-in household member reads the shared ledger.
   */
  protected readonly data = resource({
    params: () => ({
      account: this.session.me()?.account,
    }),
    loader: async ({ params }) => {
      if (params.account) {
        const page = await this.api.ledger(LEDGER_LIMIT);
        return {
          transactions: page.transactions,
          circulation: page.circulation,
          balance: 0,
        };
      }
      return { transactions: [], circulation: 0, balance: 0 };
    },
  });

  private readonly txns = computed(() => this.data.value()?.transactions ?? []);

  protected readonly retained = computed(() => this.txns().length);

  /** True when the ledger holds more than the window this page can see. */
  protected readonly truncated = computed(() => {
    const status = this.session.status();
    if (!status) return false;
    return status.transactions > this.txns().length;
  });

  protected readonly supply = computed<Series[]>(() => [
    moneySupplySeries(this.txns(), this.data.value()?.circulation ?? 0),
  ]);

  protected readonly gdp = computed<Series[]>(() => [
    gdpSeries(this.txns(), this.bucket()),
  ]);

  protected readonly gdpSubtitle = computed(
    () =>
      `Value changing hands per ${this.bucket()}. Transfers and purchases only —` +
      ' issuing coin is not economic activity.',
  );

  /**
   * One line per household member.
   *
   * Capped at the number of distinct hues, because a generated ninth colour is
   * indistinguishable from an existing one. A household past the cap keeps the
   * people with the most history, which are the lines with something to show.
   */
  protected readonly balances = computed<Series[]>(() => {
    const txns = this.txns();
    const people = this.session
      .household()
      .filter((u) => u.status === 'ACTIVE')
      .map((u) => ({
        user: u,
        activity: txns.filter((t) => t.postings.some((p) => p.account === u.account))
          .length,
      }))
      .sort((a, b) => b.activity - a.activity)
      .slice(0, 6);

    return people
      .map(({ user }) =>
        balanceSeries(txns, user.account, user.balance ?? 0, user.display_name),
      )
      .filter((s) => s.points.length > 0);
  });

}
