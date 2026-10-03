import { Component, computed, effect, inject, resource, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { NanacoinService } from '../api/nanacoin.service';
import { Session, reloadOnLedgerChange } from '../api/session';
import { ApiBase } from '../api/api-base';
import { Money } from '../api/money';
import { Log } from '../api/log';
import { Toasts } from '../ui/toasts';
import { EconomyStat } from '../ui/economy-stat';
import { LineChart } from '../economy/line-chart';
import { Bucket, Series } from '../economy/series';
import { actualRates } from '../economy/forex-series';
import {
  personalFlows,
  personalFlowSeries,
  personalPositionSeries,
  wealthPosition,
} from '../economy/wealth';

@Component({
  selector: 'app-wealth',
  imports: [FormsModule, EconomyStat, LineChart],
  template: `
    <h1>My wealth</h1>
    <p class="muted">
      Your household finances, {{ session.me()?.display_name }}. Assets, debts, and the money you
      keep.
    </p>
    @if (!session.signedIn()) {
      <p>Sign in to see your wealth.</p>
    } @else if (data.isLoading()) {
      <p role="status">Loading your finances…</p>
    } @else if (data.error()) {
      <p role="alert">
        Could not load your finances. <button class="btn" (click)="data.reload()">Retry</button>
      </p>
    } @else if (data.hasValue()) {
      <div class="stats economy-stats" aria-label="Personal financial indicators">
        <app-economy-stat
          id="wealth-cash"
          label="Cash in NC"
          [value]="money.format(position().cash) + ' NC'"
          help="Your current spendable coin balance. Loans and savings pools are shown separately below."
        />
        <app-economy-stat
          id="wealth-usd"
          label="Cash in USD (est)"
          [value]="cashEstimate()"
          help="Recorded USD cash plus the estimated dollar value of your coins at your selected rate. This is a valuation, not a cash-out offer."
        />
        <app-economy-stat
          id="wealth-net"
          label="Net worth"
          [value]="totalLabel(netCoins(), position().dollars)"
          help="Recorded cash, loan principal and accrued interest owed to you, and savings-pool principal, less your loans and accrued interest owed. Unpriced belongings, artwork, bearer vouchers and uncertain lotto winnings are excluded."
        />
        <app-economy-stat
          id="wealth-income"
          label="Income"
          [value]="ncLabel(income())"
          help="Cash income in the selected period: work and goods, gifts, interest, and other payments. Loan principal, exchanges, savings-pool principal and unclassified issuance are excluded. Refunds reduce the original category."
        />
        <app-economy-stat
          id="wealth-saving"
          label="Savings Rate"
          [value]="savingsRate()"
          help="(Income minus expenses) divided by income for the selected period. A dash means income is zero or negative. Borrowing and capital funding do not count as saving income."
        />
      </div>
      <section class="panel">
        <h2>Your valuation</h2>
        <div class="wealth-controls">
          <label class="checkbox"
            ><input
              type="checkbox"
              [checked]="usd()"
              (change)="usd.set($any($event.target).checked)"
            />
            Show values in USD (est)</label
          >
          <label
            >Your cash-out estimate: USD per NC
            <input
              type="number"
              min="0"
              step="any"
              [ngModel]="customRate() ?? marketRate()"
              (ngModelChange)="setRate($event)"
          /></label>
          <button class="btn btn--quiet" (click)="resetRate()">Use market rate</button>
        </div>
        <p class="muted small">
          {{
            marketRate() === null
              ? 'No completed exchange is available yet. Enter a rate to estimate USD values.'
              : 'Latest completed trade: ' + dollarLabel(marketRate()!) + ' per NC.'
          }}
          {{
            customRate() === null
              ? 'Using the market rate.'
              : 'Using your own rate for this account.'
          }}
        </p>
        <p class="muted small">
          Selling every coin could crash the Nanacoin market. Use a lower cash-out rate if the last
          trade overstates what you could recover. This rate changes this report only; it does not
          change exchange offers. The same rate values every historical point.
        </p>
      </section>
      <div class="wealth-controls">
        <label
          >Income and expenses period
          <select [ngModel]="period()" (ngModelChange)="period.set($event)">
            <option value="30">Last 30 days</option>
            <option value="365">Last 365 days</option>
            <option value="all">All available history</option>
          </select></label
        >
        <label
          >Group charts by
          <select [ngModel]="bucket()" (ngModelChange)="bucket.set($event)">
            <option value="day">Day</option>
            <option value="week">Week</option>
            <option value="month">Month</option>
            <option value="year">Year</option>
          </select></label
        >
      </div>
      <p class="muted small">
        Income and charts cover {{ data.value()!.history.transactions.length }} available account
        transactions.
        {{
          limited()
            ? 'Older history is unavailable in this report; totals are not lifetime figures.'
            : 'Totals describe only the selected period.'
        }}
        Current balances and loan obligations include earlier activity.
      </p>
      <div class="wealth-sheets">
        <section class="panel">
          <h2>Balance sheet</h2>
          <h3>Assets</h3>
          <dl>
            <dt>NanaCoin cash</dt>
            <dd>{{ ncLabel(position().cash) }}</dd>
            <dt>Recorded USD cash</dt>
            <dd>{{ dollarLabel(position().dollars / 100) }}</dd>
            <dt>Loans owed to you</dt>
            <dd>{{ ncLabel(position().receivable) }}</dd>
            <dt>Interest owed to you</dt>
            <dd>{{ ncLabel(position().interestAsset) }}</dd>
            <dt>Savings-pool principal</dt>
            <dd>{{ ncLabel(position().savings) }}</dd>
            <dt>Total assets</dt>
            <dd>
              <strong>{{ totalLabel(assetCoins(), position().dollars) }}</strong>
            </dd>
          </dl>
          <h3>Liabilities</h3>
          <dl>
            <dt>Loan principal you owe</dt>
            <dd>{{ ncLabel(position().payable) }}</dd>
            <dt>Accrued interest you owe</dt>
            <dd>{{ ncLabel(position().interestDebt) }}</dd>
            <dt>Total liabilities</dt>
            <dd>
              <strong>{{ ncLabel(debtCoins()) }}</strong>
            </dd>
            <dt>Net worth</dt>
            <dd>
              <strong>{{ totalLabel(netCoins(), position().dollars) }}</strong>
            </dd>
          </dl>
          <p class="muted small">
            Undrawn credit and pending offers are not debts. Household objects, art, vouchers and
            uncertain lotto prizes have no valuation here. Future unaccrued interest is excluded.
          </p>
        </section>
        <section class="panel">
          <h2>Income and expenses</h2>
          <div class="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Category</th>
                  <th>Income</th>
                  <th>Expenses</th>
                </tr>
              </thead>
              <tbody>
                @for (row of flows(); track row.category) {
                  <tr>
                    <th>{{ row.category }}</th>
                    <td>{{ ncLabel(row.income) }}</td>
                    <td>{{ ncLabel(row.expenses) }}</td>
                  </tr>
                }
                <tr>
                  <th>Total</th>
                  <td>{{ ncLabel(income()) }}</td>
                  <td>{{ ncLabel(expenses()) }}</td>
                </tr>
              </tbody>
            </table>
          </div>
          <p>
            <strong>Saved: {{ ncLabel(income() - expenses()) }}</strong> · {{ savingsRate() }}
          </p>
          <p class="muted small">
            Cash accounting: sales count when paid; interest counts when collected or paid. Goods
            bought count as spending rather than unpriced assets. Loan repayments and forex trades
            move assets without becoming income or spending.
          </p>
        </section>
      </div>
      @if (missingBooks()) {
        <p class="warning">
          Some retained payments refer to loans or lotto books no longer available. Historical
          balance charts are unavailable, and unclassified lotto principal is excluded from income
          and expenses. Current cash and active loan balances remain available.
        </p>
      } @else {
        <app-line-chart
          [title]="
            usd()
              ? 'Assets, liabilities and net worth · USD (est)'
              : 'Assets, liabilities and net worth · NC'
          "
          subtitle="Principal position over available history. Historical accrued interest is unavailable and excluded; the balance sheet above includes today’s accrued interest. USD cash is included in USD mode; NC mode shows the coin position only."
          [series]="positionChart()"
        />
      }
      <app-line-chart
        [title]="usd() ? 'Income and expenses · USD (est)' : 'Income and expenses · NC'"
        [subtitle]="'Cash flows per ' + bucket() + ' in the selected period.'"
        [series]="flowChart()"
      />
      <app-line-chart
        title="Savings rate over time"
        [subtitle]="
          'Percent of income kept per ' +
          bucket() +
          '. Periods without positive income are omitted.'
        "
        [series]="savingChart()"
      />
    }
  `,
  styles: `
    .wealth-sheets {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 1rem;
    }
    .wealth-sheets dl {
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 0.65rem;
    }
    .wealth-sheets dd {
      margin: 0;
      text-align: right;
      font-variant-numeric: tabular-nums;
    }
    .wealth-controls {
      display: flex;
      gap: 1rem;
      flex-wrap: wrap;
      align-items: end;
      margin: 1rem 0;
    }
    .wealth-controls input[type='number'] {
      max-width: 16rem;
    }
    .table-scroll {
      overflow: auto;
    }
    table {
      width: 100%;
    }
    th,
    td {
      text-align: left;
      padding: 0.6rem;
      border-bottom: 1px solid var(--line);
    }
    @media (max-width: 750px) {
      .wealth-sheets {
        grid-template-columns: 1fr;
      }
    }
  `,
})
export class WealthPage {
  readonly session = inject(Session);
  readonly money = inject(Money);
  private readonly api = inject(NanacoinService);
  private readonly base = inject(ApiBase);
  private readonly log = inject(Log);
  private readonly toasts = inject(Toasts);
  readonly usd = signal(false);
  readonly customRate = signal<number | null>(null);
  readonly period = signal('30');
  readonly bucket = signal<Bucket>('week');
  readonly data = resource({
    params: () =>
      this.session.me()?.account
        ? { account: this.session.me()!.account, source: this.base.current() }
        : undefined,
    loader: async ({ params }) => {
      const [history, loans, quotes, lottos, me] = await Promise.all([
        this.loadHistory(params.account),
        this.api.loans(),
        this.api.quotes(),
        this.api.lottos(),
        this.api.me(),
      ]);
      return { history, loans: loans.loans, quotes: quotes.quotes, lottos: lottos.lottos, me };
    },
  });
  private readonly followLedger = reloadOnLedgerChange(this.data);
  readonly position = computed(() =>
    wealthPosition(
      this.session.me()?.account ?? '',
      this.data.value()?.history.balance ?? 0,
      this.data.value()?.me.usd_cents ?? 0,
      this.data.value()?.loans ?? [],
      this.data.value()?.lottos ?? [],
      this.data.value()?.history.transactions ?? [],
    ),
  );
  readonly assetCoins = computed(() => {
    const p = this.position();
    return p.cash + p.receivable + p.interestAsset + p.savings;
  });
  readonly debtCoins = computed(() => this.position().payable + this.position().interestDebt);
  readonly netCoins = computed(() => this.assetCoins() - this.debtCoins());
  readonly marketRate = computed(() => {
    const d = this.data.value();
    const rate = actualRates(
      d?.quotes ?? [],
      d?.history.transactions ?? [],
      this.money.decimals(),
    ).at(-1)?.cents;
    return rate === undefined ? null : rate / 100;
  });
  readonly rate = computed(() => this.customRate() ?? this.marketRate());
  private readonly transactions = computed(() => {
    const since = this.period() === 'all' ? 0 : Date.now() / 1000 - Number(this.period()) * 86400;
    return (this.data.value()?.history.transactions ?? []).filter((t) => t.created_at >= since);
  });
  readonly flows = computed(() =>
    personalFlows(
      this.transactions(),
      this.session.me()?.account ?? '',
      this.data.value()?.lottos ?? [],
    ),
  );
  readonly income = computed(() => this.flows().reduce((n, r) => n + r.income, 0));
  readonly expenses = computed(() => this.flows().reduce((n, r) => n + r.expenses, 0));
  readonly limited = computed(() => {
    const h = this.data.value()?.history;
    return !!(
      h?.history_truncated ||
      h?.next_cursor ||
      h?.next_before ||
      (h?.transactions.length ?? 0) >= 365
    );
  });
  readonly missingBooks = computed(() => {
    const d = this.data.value();
    return !!d?.history.transactions.some(
      (t) =>
        (t.economic_kind === 'LOAN_PRINCIPAL' &&
          !d.loans.some((l) => t.reference === `loan-${l.id}`)) ||
        (t.reference?.startsWith('lotto-') &&
          t.economic_kind !== 'INTEREST' &&
          !d.lottos.some((l) => t.reference === `lotto-${l.id}`)),
    );
  });
  private readonly rawFlows = computed(() =>
    personalFlowSeries(
      this.transactions(),
      this.session.me()?.account ?? '',
      this.bucket(),
      this.data.value()?.lottos ?? [],
    ),
  );
  readonly flowChart = computed(() => this.convert(this.rawFlows()));
  readonly savingChart = computed<Series[]>(() => {
    const [income, expenses] = this.rawFlows();
    return [
      {
        name: 'Savings rate (%)',
        points: income.points
          .filter((p) => p.value > 0)
          .map((p) => ({
            at: p.at,
            value:
              ((p.value - (expenses.points.find((e) => e.at === p.at)?.value ?? 0)) / p.value) *
              100,
          })),
      },
    ];
  });
  readonly positionChart = computed<Series[]>(() => {
    if (this.usd() && this.rate() === null) return [];
    const d = this.data.value(),
      account = this.session.me()?.account ?? '';
    const [assets, debt, dollars] = personalPositionSeries(
      d?.history.transactions ?? [],
      account,
      this.position(),
      d?.loans ?? [],
      d?.lottos ?? [],
    );
    const convert = (n: number) => this.money.chart(n) * (this.usd() ? this.rate()! : 1);
    const a = assets.points.map((p, i) => ({
      at: p.at,
      value: convert(p.value) + (this.usd() ? dollars.points[i].value / 100 : 0),
    }));
    const l = debt.points.map((p) => ({ at: p.at, value: convert(p.value) }));
    return [
      { name: 'Assets (principal)', points: a },
      { name: 'Liabilities (principal)', points: l },
      {
        name: 'Net worth (principal)',
        points: a.map((p, i) => ({ at: p.at, value: p.value - l[i].value })),
      },
    ];
  });
  constructor() {
    effect(() => {
      const key = this.preferenceKey();
      try {
        const saved = localStorage.getItem(key);
        const rate = saved === null ? null : Number(saved);
        this.customRate.set(rate !== null && Number.isFinite(rate) && rate >= 0 ? rate : null);
      } catch (error) {
        this.log.warn('wealth', 'Could not read valuation preference', { error: String(error) });
        this.customRate.set(null);
      }
    });
  }
  private async loadHistory(account: string) {
    const first = await this.api.accountHistory(account, 100);
    const transactions = [...first.transactions];
    let page = first;
    const cursors = new Set<string>();
    while (page.next_cursor && transactions.length < 365 && !cursors.has(page.next_cursor)) {
      cursors.add(page.next_cursor);
      page = await this.api.accountHistory(
        account,
        Math.min(100, 365 - transactions.length),
        page.next_cursor,
      );
      transactions.push(...page.transactions);
    }
    return { ...first, transactions, next_cursor: page.next_cursor, next_before: page.next_before };
  }
  private preferenceKey(): string {
    return `nanacoin:wealth-rate:${this.base.current()}:${this.session.me()?.id ?? ''}:m${this.money.epoch()}`;
  }
  setRate(value: number | null): void {
    if (value === null) {
      this.resetRate();
      return;
    }
    if (!Number.isFinite(value) || value < 0) {
      this.toasts.error('Enter a finite exchange rate of zero or more USD per NC.');
      return;
    }
    this.customRate.set(value);
    try {
      localStorage.setItem(this.preferenceKey(), String(value));
    } catch (error) {
      this.log.warn('wealth', 'Valuation rate applies for this visit only', {
        error: String(error),
      });
    }
  }
  resetRate(): void {
    this.customRate.set(null);
    try {
      localStorage.removeItem(this.preferenceKey());
    } catch (error) {
      this.log.warn('wealth', 'Could not clear saved valuation rate', { error: String(error) });
    }
  }
  dollarLabel(dollars: number): string {
    return new Intl.NumberFormat(undefined, {
      style: 'currency',
      currency: 'USD',
      maximumFractionDigits: 6,
    }).format(dollars);
  }
  ncLabel(minor: number): string {
    return this.usd()
      ? this.rate() === null
        ? 'Set an exchange rate'
        : this.dollarLabel(this.money.chart(minor) * this.rate()!)
      : this.money.format(minor) + ' NC';
  }
  totalLabel(minor: number, cents: number): string {
    return this.usd()
      ? this.rate() === null
        ? `${this.dollarLabel(cents / 100)} + unpriced NC`
        : this.dollarLabel(this.money.chart(minor) * this.rate()! + cents / 100)
      : `${this.money.format(minor)} NC + ${this.dollarLabel(cents / 100)}`;
  }
  cashEstimate(): string {
    const rate = this.rate();
    return rate === null
      ? `${this.dollarLabel(this.position().dollars / 100)} + unpriced NC`
      : this.dollarLabel(
          this.money.chart(this.position().cash) * rate + this.position().dollars / 100,
        );
  }
  savingsRate(): string {
    return this.income() > 0
      ? (((this.income() - this.expenses()) / this.income()) * 100).toFixed(1) + '%'
      : '—';
  }
  private convert(series: Series[]): Series[] {
    if (this.usd() && this.rate() === null) return [];
    return series.map((s) => ({
      ...s,
      points: s.points.map((p) => ({
        ...p,
        value: this.money.chart(p.value) * (this.usd() ? this.rate()! : 1),
      })),
    }));
  }
}
