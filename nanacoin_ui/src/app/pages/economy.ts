import { Money, MoneyPipe } from '../api/money';
import { inject as moneyInject } from '@angular/core';
// The household economy: what the money supply has done, what changed hands,
// and how each person's balance moved.
//
// The ledger is shared with the household, like the paper notebook in the
// NanaCoin premise. Everyone can audit the supply, GDP, and balance movement.
//
// All the arithmetic runs here rather than on the board. See series.ts.

import { Component, computed, inject, resource, signal } from '@angular/core';

import { NanacoinService } from '../api/nanacoin.service';
import { Session, reloadOnLedgerChange } from '../api/session';
import { Quote } from '../api/models';
import { actualRates } from '../economy/forex-series';
import { ForexChart } from '../economy/forex-chart';
import { LineChart } from '../economy/line-chart';
import { EconomyStat } from '../ui/economy-stat';
import {
  Bucket,
  Series,
  balanceSeries,
  gdpSeries,
  moneySupplySeries,
  employmentSnapshot,
  employmentSeries,
  giftsThisWeek,
  repeatPriceChanges,
  inflationSeries,
  interestSeries,
} from '../economy/series';

/** How many transactions to ask for. The server caps this itself. */
const LEDGER_LIMIT = 365;

@Component({
  selector: 'app-economy',
  imports: [MoneyPipe, LineChart, EconomyStat, ForexChart],
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

      <div class="stats economy-stats" aria-label="Economic indicators">
        <app-economy-stat id="employment-help" label="employment"
          [value]="(yearEmployment() * 100).toFixed(0) + '%'"
          help="Share of currently active member accounts other than Nana that received a payment classified as labor. Uses up to 365 days ending at the latest retained transaction; available history may be shorter. Accounts currently have no pet or labor-eligibility classification." />
        <app-economy-stat id="inflation-help" label="inflation" [value]="inflationLabel()"
          help="Average price change for goods with repeat sales in up to 365 days ending at the latest retained transaction. Available history may be shorter. This is a household repeat-sale measure, not a consumer price index. A dash means there are no comparable sales." />
        <app-economy-stat id="gdp-help" label="GDP" [value]="money.chart(yearGdp()).toLocaleString() + ' NC'"
          help="Recorded goods and labor payments in up to 365 days ending at the latest retained transaction; available history may be shorter. Reversals subtract production. Gifts, issuance, loan principal, pure interest, and unclassified transfers are excluded." />
        <app-economy-stat id="supply-help" label="money supply" [value]="money.format(data.value()?.circulation ?? 0) + ' NC'"
          help="Current NanaCoin in circulation across all accounts, including Nana. Issuing and retiring coins change the supply; transfers and lending move existing coins." />
        <app-economy-stat id="exchange-help" label="exchange rate" [value]="currentRateLabel()"
          help="Rate from the most recent completed exchange in retained quotes or paired transaction records. Live bids and asks are shown separately in the exchange chart." />
        <app-economy-stat id="interest-help" label="interest rate" [value]="interestLabel()"
          help="Current simple interest rate weighted by outstanding loan principal, including zero-rate loans. Each loan's rate is normalized to 365 days. This is not a compounded yield or an inflation forecast. Undrawn credit and unaccepted offers are excluded." />
      </div>

      <section class="panel">
        <h2>Lending</h2>
        @if (data.value()?.loanSummary; as loans) {
          <p>Outstanding principal: <strong>{{loans.outstanding | nc}} NC</strong> · Overdue: {{loans.overdue | nc}} NC</p>
        } @else { <p>Lending statistics are unavailable.</p> }
      </section>

      <div class="chart-controls">
          <label>
            Group by
            <select [value]="bucket()" (change)="bucket.set($any($event.target).value)">
              <option value="day">Day</option>
              <option value="week">Week</option>
              <option value="month">Month</option>
              <option value="year">Year</option>
            </select>
          </label>
      </div>

      <div class="cards">
        <article class="card">
          <h3>Employment this week</h3>
          <p class="card__meta"><strong>{{ employment().employed }} of {{ employment().laborPool }}</strong> people · {{ (employment().rate * 100).toFixed(0) }}%</p>
          <p class="muted small">{{ money.chart(employment().laborPayments).toLocaleString() }} NC paid for labor; {{ money.chart(employment().averagePayment).toLocaleString() }} per labor payment. Nana is not in the labor pool.</p>
        </article>
        <article class="card">
          <h3>Gifts this week</h3>
          <p class="card__meta"><strong>{{ money.chart(gifts()).toLocaleString() }} NC</strong></p>
          <p class="muted small">Gifts are tracked separately and do not count as production.</p>
        </article>
      </div>

      <section class="panel">
        <h2>Repeat-sale prices</h2>
        @if (priceChanges().length === 0) {
          <p class="muted">A good needs two completed sales before its price can be compared.</p>
        } @else {
          @for (change of priceChanges(); track change.thing) {
            <p><strong>{{ change.thing }}</strong>: {{ money.chart(change.previous).toLocaleString() }} → {{ money.chart(change.latest).toLocaleString() }} NC per {{ change.unit.toLocaleLowerCase() }} ({{ change.percent >= 0 ? '+' : '' }}{{ (change.percent * 100).toFixed(1) }}%)</p>
          }
        }
      </section>

        <app-line-chart
          title="Employment rate"
          [subtitle]="'Share of the labor pool paid for labor per ' + bucket() + '. Nana is excluded.'"
          [series]="employmentChart()"
        />

        <app-line-chart
          title="Repeat-sale inflation"
          [subtitle]="'Average price change when the same good sells again, grouped by ' + bucket() + '. Sparse households may need Month or Year.'"
          [series]="inflationChart()"
        />

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
          title="Interest paid"
          subtitle="Recorded interest payments in the available history. Includes loan and lotto interest, including newly issued lotto interest. Principal and pure interest are excluded from GDP."
          [series]="interest()"
        />

        <app-line-chart
          title="Balances"
          subtitle="What each person holds over time."
          [series]="balances()"
        />

        <app-forex-chart [quotes]="quotes()" [transactions]="txns()" [decimals]="money.decimals()" />
    }
  `,
})
export class EconomyPage {
  protected readonly money = moneyInject(Money);
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
        const [page, quotes, lending] = await Promise.all([
          this.api.ledger(LEDGER_LIMIT),
          this.api.quotes().then((result) => result.quotes).catch(() => [] as Quote[]),
          this.api.loans().catch(() => null),
        ]);
        return {
          transactions: page.transactions,
          circulation: page.circulation,
          quotes,
          balance: 0,
          loanSummary: lending?.summary ?? null,
        };
      }
      return { transactions: [], circulation: 0, quotes: [] as Quote[], balance: 0, loanSummary: null };
    },
  });

  /** Catch up when someone else changes the ledger while this page is open. */
  private readonly followLedger = reloadOnLedgerChange(this.data);
  protected readonly txns = computed(() => this.data.value()?.transactions ?? []);
  protected readonly quotes = computed(() => this.data.value()?.quotes ?? []);
  private readonly yearTxns = computed(() => {
    const txns = this.txns();
    if (!txns.length) return [];
    const latest = Math.max(...txns.map((txn) => txn.created_at));
    return txns.filter((txn) => txn.created_at >= latest - 365 * 24 * 60 * 60);
  });

  protected readonly yearEmployment = computed(() => {
    const eligible = new Set(this.session.household().filter((u) => u.status === 'ACTIVE' && u.role !== 'nana').map((u) => u.account));
    const earners = new Set<string>();
    for (const txn of this.yearTxns()) if (txn.economic_kind === 'LABOR' && !txn.reversed_by && txn.kind !== 'REVERSAL') {
      const payee = txn.postings.find((posting) => posting.amount > 0 && eligible.has(posting.account));
      if (payee) earners.add(payee.account);
    }
    return eligible.size ? earners.size / eligible.size : 0;
  });
  protected readonly yearInflation = computed(() => {
    const changes = repeatPriceChanges(this.yearTxns());
    return changes.length ? changes.reduce((sum, change) => sum + change.percent, 0) / changes.length : 0;
  });
  protected readonly yearGdp = computed(() => gdpSeries(this.yearTxns(), 'year').points.reduce((sum, point) => sum + point.value, 0));
  protected inflationLabel(): string {
    if (!repeatPriceChanges(this.yearTxns()).length) return '—';
    return `${this.yearInflation() >= 0 ? '+' : ''}${(this.yearInflation() * 100).toFixed(1)}%`;
  }
  protected interestLabel(): string {
    const loans = this.data.value()?.loanSummary;
    if (!loans) return 'Unavailable';
    const rate = loans.weighted_annual_percent;
    return rate === null ? 'No loans' : `${rate.toLocaleString(undefined, { maximumFractionDigits: 4 })}%/yr`;
  }

  protected currentRateLabel(): string {
    const cents = actualRates(this.quotes(),this.txns(),this.money.decimals()).at(-1)?.cents;
    return cents === undefined ? 'No trades yet' : `$${(cents/100).toLocaleString(undefined,{maximumFractionDigits:6})}/NC`;
  }

  protected readonly retained = computed(() => this.txns().length);

  /** True when the ledger holds more than the window this page can see. */
  protected readonly truncated = computed(() => {
    const status = this.session.status();
    if (!status) return false;
    return status.transactions > this.txns().length;
  });

  protected readonly supply = computed<Series[]>(() => [
    this.moneySeries(moneySupplySeries(this.txns(), this.data.value()?.circulation ?? 0)),
  ]);

  protected readonly gdp = computed<Series[]>(() => [
    this.moneySeries(gdpSeries(this.txns(), this.bucket())),
  ]);

  protected readonly interest = computed<Series[]>(() => [
    this.moneySeries(interestSeries(this.txns(), this.bucket())),
  ]);

  protected readonly gdpSubtitle = computed(
    () =>
      `Labor and goods produced per ${this.bucket()}. Loan principal, pure interest, gifts, unclassified transfers, and coin issuance are excluded.`,
  );

  protected readonly employment = computed(() => employmentSnapshot(
    this.txns(),
    this.session.household().filter((u) => u.status === 'ACTIVE' && u.role !== 'nana').map((u) => u.account),
  ));
  protected readonly gifts = computed(() => giftsThisWeek(this.txns()));
  protected readonly priceChanges = computed(() => repeatPriceChanges(this.txns()));
  protected readonly employmentChart = computed<Series[]>(() => [employmentSeries(
    this.txns(),
    this.session.household().filter((u) => u.status === 'ACTIVE' && u.role !== 'nana').map((u) => u.account),
    this.bucket(),
  )]);
  protected readonly inflationChart = computed<Series[]>(() => [
    inflationSeries(this.txns(), this.bucket()),
  ]);

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
        this.moneySeries(balanceSeries(txns, user.account, user.balance ?? 0, user.display_name)),
      )
      .filter((s) => s.points.length > 0);
  });

  private moneySeries(series: Series): Series { return { ...series, points: series.points.map(p => ({ ...p, value: this.money.chart(p.value) })) }; }
}
