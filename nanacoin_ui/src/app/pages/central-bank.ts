// Nana as central bank: the household's reserve, money supply, inflation and
// Nana's flows, in one report everyone can read. Arithmetic lives in
// economy/central-bank.ts.

import { Component, computed, inject, resource, signal } from '@angular/core';
import { Money, MoneyPipe } from '../api/money';
import { NanacoinService } from '../api/nanacoin.service';
import { Session, reloadOnLedgerChange } from '../api/session';
import { Loan, Lotto, Quote } from '../api/models';
import { actualRates } from '../economy/forex-series';
import { repeatPriceChanges } from '../economy/series';
import { centralBankFlows, centralBankPosition, supplyGrowth } from '../economy/central-bank';
import { EconomyStat } from '../ui/economy-stat';

type Window = '30' | '365' | 'all';
const DAY = 86_400;

@Component({
  selector: 'app-central-bank',
  imports: [MoneyPipe, EconomyStat],
  template: `
    <h1>Nana as central bank</h1>
    <p class="lede">Nana makes the money, keeps the dollar reserve, and stands ready to trade and lend. This report shows what backs NanaCoin, where new coins came from, and what Nana has promised.</p>
    @if (data.isLoading() && !data.hasValue()) { <p class="muted">Loading…</p> }
    @else if (data.error()) { <p role="alert">Could not load the report.</p> }
    @else if (!nana()) { <p class="muted">This household has no Nana yet.</p> }
    @else {
      @if (position(); as p) {
        <div class="stats economy-stats" aria-label="Central bank summary">
          <app-economy-stat id="cb-supply" label="money supply" [value]="money.format(p.circulation) + ' NC'"
            help="Every NanaCoin that exists, in anyone's account, including Nana's and lotto pools. Only issuing and retiring change it." />
          <app-economy-stat id="cb-public" label="held by the household" [value]="money.format(p.publicCoins) + ' NC'"
            help="Coins outside Nana's own account. These are the coins that could ask to be turned into dollars." />
          <app-economy-stat id="cb-reserve" label="dollar reserve" [value]="usdLabel(p.usdReserve)"
            [help]="'Real dollars Nana has recorded as holding. They pay for her buy offers. ' + (canSeeReserve() ? '' : 'Hidden from this account.')" />
          <app-economy-stat id="cb-ratio" label="reserve ratio" [value]="ratioLabel(p.reserveRatio)"
            help="Dollar reserve divided by what Nana owes if every one of her open buy offers is taken. 100% or more means every promise is covered." />
          <app-economy-stat id="cb-backing" label="backing per coin" [value]="p.backingCentsPerCoin === null ? '—' : usdLabel(p.backingCentsPerCoin, 4)"
            help="The dollar reserve shared equally over every coin the household holds. If everyone cashed out at once, this is roughly what each coin could get. Nana's ladder pays the first sellers more and the last ones much less." />
          <app-economy-stat id="cb-inflation" label="inflation" [value]="inflationLabel()"
            help="Average price change for goods that sold more than once in the chosen period. A dash means there were no repeat sales to compare." />
          <app-economy-stat id="cb-growth" label="money supply growth" [value]="growthLabel()"
            help="How much the money supply grew in the chosen period: new coins minus retired coins, compared with the supply at the start." />
          <app-economy-stat id="cb-rate" label="exchange rate" [value]="rateLabel()"
            help="Price of one coin in the most recent completed dollar trade." />
        </div>

        <div class="chart-controls">
          <label>Period
            <select [value]="window()" (change)="window.set($any($event.target).value)">
              <option value="30">Last 30 days</option><option value="365">Last 365 days</option><option value="all">Everything the board still holds</option>
            </select>
          </label>
        </div>
        @if (truncated()) { <p class="muted small">The board keeps the most recent {{ txns().length }} transactions. Older history has left its memory, so flows start partway through.</p> }

        @if (flows(); as f) {
          <section class="panel">
            <h2>Coins in and out of existence</h2>
            <table class="report"><tbody>
              <tr><th scope="row">New coins issued to Nana</th><td>{{ f.issuedToNana | nc }} NC</td></tr>
              <tr><th scope="row">New coins issued to members</th><td>{{ f.issuedToMembers | nc }} NC</td></tr>
              <tr><th scope="row">New coins for lotto interest</th><td>{{ f.issuedForLottoInterest | nc }} NC</td></tr>
              <tr><th scope="row">Coins retired</th><td>−{{ f.retired | nc }} NC</td></tr>
              @if (f.corrections) { <tr><th scope="row">Corrections (reversed issuing or retiring)</th><td>{{ f.corrections | nc }} NC</td></tr> }
              <tr class="total"><th scope="row">Net change in money supply</th><td>{{ f.netIssuance | nc }} NC</td></tr>
              <tr><th scope="row">Seigniorage at today's rate <small>(what the new coins are worth in dollars)</small></th><td>{{ seigniorage(f.netIssuance) }}</td></tr>
              <tr><th scope="row">Demurrage <small>(a fee for holding money)</small></th><td>None charged</td></tr>
            </tbody></table>
          </section>

          <section class="panel">
            <h2>Nana on the dollar exchange</h2>
            <p class="muted small">Buying coins back for dollars takes coins out of the household; selling coins for dollars puts them in.</p>
            <table class="report"><tbody>
              <tr><th scope="row">Coins Nana bought back</th><td>{{ f.coinsBoughtBack | nc }} NC for {{ usdLabel(f.usdPaidOut) }}</td></tr>
              <tr><th scope="row">Coins Nana sold</th><td>{{ f.coinsSold | nc }} NC for {{ usdLabel(f.usdTakenIn) }}</td></tr>
              <tr class="total"><th scope="row">Nana's dollar result from trading</th><td>{{ usdLabel(f.usdTakenIn - f.usdPaidOut) }}</td></tr>
              <tr><th scope="row">Real dollars recorded into the household</th><td>{{ usdLabel(f.usdRecorded) }}</td></tr>
            </tbody></table>
          </section>

          <section class="panel">
            <h2>Nana's coin income and spending</h2>
            <table class="report"><tbody>
              <tr><th scope="row">Interest Nana received</th><td>{{ f.interestReceived | nc }} NC</td></tr>
              <tr><th scope="row">Interest Nana paid (lottos, loans)</th><td>−{{ f.interestPaid | nc }} NC</td></tr>
              <tr><th scope="row">Nana bought work and goods</th><td>−{{ f.boughtFromMembers | nc }} NC</td></tr>
              <tr><th scope="row">Nana sold work and goods</th><td>{{ f.soldToMembers | nc }} NC</td></tr>
              <tr><th scope="row">Nana paid out (allowances, gifts, other)</th><td>−{{ f.paidOut | nc }} NC</td></tr>
              <tr><th scope="row">Nana received (other)</th><td>{{ f.receivedOther | nc }} NC</td></tr>
              <tr><th scope="row">Loans Nana made / repaid to her</th><td>{{ f.lent | nc }} / {{ f.repaid | nc }} NC</td></tr>
            </tbody></table>
            <p class="muted small">Loans move coins but are not income: a repaid loan just comes back. Interest is the income.</p>
          </section>
        }

        <section class="panel">
          <h2>What Nana holds and what she has promised</h2>
          <div class="sheet">
            <table class="report"><caption>Holds</caption><tbody>
              <tr><th scope="row">Dollar reserve</th><td>{{ usdLabel(p.usdReserve) }}</td></tr>
              <tr><th scope="row">Her own coins</th><td>{{ p.nanaCoins | nc }} NC</td></tr>
              <tr><th scope="row">Loans owed to her</th><td>{{ isNana() ? (p.loansOwedToNana | nc) + ' NC' : 'Only Nana sees every loan' }}</td></tr>
              @if (isNana() && p.loansOverdueToNana) { <tr><th scope="row">…of which overdue</th><td>{{ p.loansOverdueToNana | nc }} NC</td></tr> }
            </tbody></table>
            <table class="report"><caption>Promised</caption><tbody>
              <tr><th scope="row">Buy offers: dollars if all are taken</th><td>{{ usdLabel(p.buyBackPromise) }}</td></tr>
              <tr><th scope="row">Sell offers: coins if all are taken</th><td>{{ p.sellPromiseCoins | nc }} NC</td></tr>
              <tr><th scope="row">Lotto interest still to pay</th><td>{{ p.lottoInterestPromised | nc }} NC</td></tr>
              <tr><th scope="row">Ticket money held in her lottos</th><td>{{ p.lottoPools | nc }} NC</td></tr>
              @if (isNana()) { <tr><th scope="row">Loan offers not yet taken</th><td>{{ p.loanOffersOpen | nc }} NC</td></tr> }
            </tbody></table>
          </div>
          <p class="muted small">Best price Nana pays for a coin: {{ p.bestBid === null ? 'no buy offer' : usdLabel(p.bestBid) }} · best price she sells at: {{ p.bestAsk === null ? 'no sell offer' : usdLabel(p.bestAsk) }}{{ p.bestBid !== null && p.bestAsk !== null ? ' · spread ' + usdLabel(p.bestAsk - p.bestBid) : '' }}</p>
        </section>

        <section class="panel">
          <h2>Words on this page</h2>
          <dl class="glossary">
            <dt>Money supply</dt><dd>All the NanaCoin that exists. Nana is the only one who can make more (issue) or destroy some (retire).</dd>
            <dt>Retiring coins</dt><dd>Nana moves coins out of an account and back into the issuance account, so they stop existing. It is a normal ledger line anyone can see, not a secret edit.</dd>
            <dt>Reserve</dt><dd>Real dollars Nana keeps so children can cash out.</dd>
            <dt>Reserve ratio</dt><dd>How much of Nana's cash-out promise she can pay right now.</dd>
            <dt>Inflation</dt><dd>Prices going up. It usually happens when coins are made faster than people make things to buy.</dd>
            <dt>Seigniorage</dt><dd>The value a money-maker gets from making new money. Here: the new coins, priced at the latest dollar rate.</dd>
            <dt>Spread</dt><dd>The gap between Nana's selling and buying prices. It is how the bank earns dollars and why buying and selling straight back loses money.</dd>
            <dt>Open-market operations</dt><dd>Nana buying or selling coins to steer how many are out in the household.</dd>
            <dt>Demurrage</dt><dd>A fee for holding money without spending it. NanaCoin does not charge one.</dd>
          </dl>
        </section>
      }
    }
  `,
  styles: [`.report {border-collapse:collapse;width:100%;max-width:40rem;} .report th {text-align:left;font-weight:normal;padding:.35rem .75rem .35rem 0;} .report td {text-align:right;white-space:nowrap;padding:.35rem 0;font-variant-numeric:tabular-nums;}
    .report tr + tr {border-top:1px solid color-mix(in srgb, currentColor 12%, transparent);} .report .total th,.report .total td {font-weight:700;} .report caption {text-align:left;font-weight:700;padding-bottom:.25rem;}
    .report small {display:block;opacity:.7;} .sheet {display:grid;gap:1.5rem;grid-template-columns:repeat(auto-fit,minmax(16rem,1fr));}
    .glossary dt {font-weight:700;margin-top:.5rem;} .glossary dd {margin:0;} .panel {margin-block:1rem;}`],
})
export class CentralBankPage {
  protected readonly money = inject(Money);
  protected readonly session = inject(Session);
  private readonly api = inject(NanacoinService);
  protected readonly window = signal<Window>('365');
  protected readonly isNana = this.session.isNana;

  protected readonly data = resource({
    params: () => ({ account: this.session.me()?.account }),
    loader: async ({ params }) => {
      if (!params.account) return null;
      const [ledger, quotes, loans, lottos] = await Promise.all([
        this.api.ledger(365),
        this.api.quotes().then((r) => r.quotes).catch(() => [] as Quote[]),
        this.api.loans().then((r) => r.loans).catch(() => [] as Loan[]),
        this.api.lottos().then((r) => r.lottos).catch(() => [] as Lotto[]),
      ]);
      return { ...ledger, quotes, loans, lottos };
    },
  });
  private readonly followLedger = reloadOnLedgerChange(this.data);

  protected readonly txns = computed(() => this.data.value()?.transactions ?? []);
  protected readonly nana = computed(() => this.session.household().find((u) => u.role === 'nana'));
  protected readonly canSeeReserve = computed(() => this.nana()?.usd_cents !== undefined);
  protected readonly truncated = computed(() => (this.session.status()?.transactions ?? 0) > this.txns().length);
  private readonly since = computed(() => {
    const w = this.window();
    if (w === 'all') return 0;
    const latest = Math.max(0, ...this.txns().map((t) => t.created_at), Math.floor(Date.now() / 1000));
    return latest - Number(w) * DAY;
  });

  protected readonly flows = computed(() => {
    const nana = this.nana();
    return nana ? centralBankFlows(this.txns(), nana.account, this.since()) : null;
  });
  protected readonly position = computed(() => {
    const nana = this.nana(), data = this.data.value();
    if (!nana || !data) return null;
    return centralBankPosition({
      circulation: data.circulation, scale: 10 ** this.money.decimals(), nana: nana.account,
      nanaCoins: nana.balance ?? 0, usdReserve: nana.usd_cents ?? 0,
      quotes: data.quotes, loans: data.loans, lottos: data.lottos,
    });
  });
  private readonly lastRate = computed(() => actualRates(this.data.value()?.quotes ?? [], this.txns(), this.money.decimals()).at(-1)?.cents ?? null);

  protected usdLabel(cents: number, digits = 2): string {
    const sign = cents < 0 ? '−' : '';
    return `${sign}$${(Math.abs(cents) / 100).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: digits })}`;
  }
  protected ratioLabel(ratio: number | null): string {
    return ratio === null ? 'No buy offers' : `${Math.round(ratio * 100)}%`;
  }
  protected rateLabel(): string {
    const cents = this.lastRate();
    return cents === null ? 'No trades yet' : `${this.usdLabel(cents, 4)} per coin`;
  }
  protected seigniorage(coins: number): string {
    const cents = this.lastRate();
    return cents === null ? 'No trades yet to price it' : this.usdLabel((coins / 10 ** this.money.decimals()) * cents);
  }
  protected inflationLabel(): string {
    const changes = repeatPriceChanges(this.txns().filter((t) => t.created_at >= this.since()));
    if (!changes.length) return '—';
    const avg = changes.reduce((n, c) => n + c.percent, 0) / changes.length;
    return `${avg >= 0 ? '+' : ''}${(avg * 100).toFixed(1)}%`;
  }
  protected growthLabel(): string {
    const f = this.flows(), p = this.position();
    if (!f || !p) return '—';
    const g = supplyGrowth(p.circulation, f.netIssuance);
    return g === null ? '—' : `${g >= 0 ? '+' : ''}${(g * 100).toFixed(1)}%`;
  }
}
