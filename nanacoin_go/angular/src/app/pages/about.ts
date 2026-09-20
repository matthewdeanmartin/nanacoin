import { DecimalPipe } from '@angular/common';
import { Component } from '@angular/core';
import { RouterLink } from '@angular/router';
import { IS_DEMO } from '../demo/demo';

const BITCOIN_KWH = 138_000_000_000;
const LOW_OWNER_ESTIMATE = 106_000_000;
const HIGH_OWNER_ESTIMATE = 365_000_000;
const BITCOIN_BASE_LAYER_TRANSACTIONS = 175_056_440;
const ETHEREUM_KWH = 2_600_000;
const ETHEREUM_BASE_LAYER_TRANSACTIONS = 446_857_103;
const NANA_MIN_KWH = 4.38;
const NANA_MAX_KWH = 21.9;

function chartRange(label: string, lowKwh: number, highKwh: number, minPower: number, maxPower: number, value: string) {
  const position = (kwh: number) => (Math.log10(kwh) - minPower) / (maxPower - minPower) * 100;
  return { label, left: position(lowKwh), width: position(highKwh) - position(lowKwh), value, range: lowKwh !== highKwh };
}

export const ENERGY_CASES = [
  { name: 'Best planning case: light household use, always on', watts: 0.5 },
  { name: 'Expected planning case: connected board with occasional requests', watts: 1 },
  { name: 'Worst planning case: nonstop API/TLS calls, all year', watts: 2.5 },
].map(c => ({ ...c, kwh: c.watts * 8760 / 1000 }));

@Component({
  selector: 'app-about', imports: [RouterLink, DecimalPipe],
  template: `<h1>A small bank. A very large notebook.</h1>
    <p>NanaCoin is a household ledger for chores, gifts and good deeds. Nana issues the money; double-entry bookkeeping keeps the books balanced. No mining race, blockchain or promise of investment returns.</p>
    <p>{{ demo ? 'This public showcase is entirely static. Its people and balances are fictional and live only in this tab. Reloading resets the economy; never enter real passwords or financial information.' : 'This client can talk to a real household board. Treat its accounts and transactions as real household data.' }}</p>
    <section class="panel"><h2>Inspired by SMBC</h2>
      <p>The beloved-Nana central bank, spiral notebook, cursive ledger, lemon squares and nana-nickel joke come from Zach Weinersmith’s
      <a href="https://www.smbc-comics.com/comic/nanacoin" target="_blank" rel="noopener noreferrer">SMBC: Nanacoin</a>.
      This is an independent homage, not an official SMBC product or endorsement. The comic is linked, not republished.</p>
      <p>The self-hosted Dancing Script typeface is by the Dancing Script Project Authors, under the SIL Open Font License. Its license is included with the static assets; no third-party font service is contacted.</p>
      <p><a routerLink="/recipes">Have a lemon bar</a> · <a routerLink="/ledger">Visit the notebook</a></p></section>
    <section class="panel"><h2>Electricity: tiny board, explicit assumptions</h2>
      <p>Unmeasured engineering scenarios for one ESP32-S3 N16R8 board at its USB input, including onboard memory and regulator losses. These are not measured minima, averages or hard upper bounds. All three assume 24 × 365 hours powered on; the worst planning case assumes nonstop API/TLS activity.</p>
      <div class="table-scroll"><table><thead><tr><th>Scenario</th><th>Average power</th><th>Annual electricity</th></tr></thead><tbody>
      @for (c of cases; track c.name) { <tr><td>{{ c.name }}</td><td>{{ c.watts }} W</td><td>{{ c.kwh }} kWh</td></tr> }
      </tbody></table></div>
      <p>Calculation: watts × 8,760 ÷ 1,000 = kWh/year. At 5 V these budgets are 100, 200 and 500 mA. An inline USB meter and a sustained load test are needed to replace assumptions with measurements. A faulty or heavily accessorized board can exceed this planning range.</p>
      <p><a href="https://documentation.espressif.com/esp32_s3_datasheet_en.pdf">Espressif’s ESP32-S3 datasheet</a> describes chip power modes and radio current, not the power of this complete board. Deep-sleep figures would be misleading for an always-available web server.</p>
      <p>For context, <a href="https://www.jbs.cam.ac.uk/2025/cambridge-study-sustainable-energy-rising-in-bitcoin-mining/">Cambridge’s April 2025 report</a> estimated the entire Bitcoin mining network at <strong>138 TWh/year</strong> = 138 billion kWh/year at its 30 June 2024 measurement point. This is a dated annualized estimate, not a current meter reading.</p>
      <p>There is no trustworthy count of Bitcoin users. The blockchain records addresses, while one person may control many addresses and one exchange address may represent millions of customers. The often-repeated <a href="https://buybitcoinworldwide.com/users-bitbo/">106 million estimate from Bitbo/Buy Bitcoin Worldwide</a> is an inference from addresses and exchange accounts, not a representative global survey. At the other end, <a href="https://crypto.com/us/research/crypto-market-sizing-report-2025">Crypto.com estimates 365 million owners</a>. A <a href="https://www.flossbachvonstorch-researchinstitute.com/fileadmin/user_upload/files/RI/Kommentare/Files/englisch/2025/250520-bitcoin-mass-market-or-elite-circle.pdf">2025 research-institute review</a> highlights the enormous spread in these claims and concludes that the number of private owners is unknown.</p>
      <p>Using both industry estimates as sensitivity bounds, 138 billion kWh works out to <strong>{{ bitcoinPerOwnerLow | number:'1.0-0' }}–{{ bitcoinPerOwnerHigh | number:'1.0-0' }} kWh per nominal owner per year</strong>. Four nominal owners correspond to roughly <strong>{{ bitcoinPerHouseholdLow | number:'1.0-0' }}–{{ bitcoinPerHouseholdHigh | number:'1.0-0' }} kWh/year</strong>, or about 69–1,189 times one NanaCoin board’s 4.38–21.9 kWh/year planning range.</p>
      <div class="energy-chart owner-chart" role="img" aria-label="Logarithmic chart. One NanaCoin board uses 4.38 to 21.9 kilowatt-hours per year. Bitcoin electricity allocated per nominal owner ranges from 378 to 1,302 kilowatt-hours per year, and per four-person household from 1,512 to 5,208 kilowatt-hours per year.">
        <p class="chart-title">Annual electricity comparison · logarithmic scale</p>
        <div class="chart-axis" aria-hidden="true"><span></span><span class="chart-ticks"><span>1 kWh</span><span>10</span><span>100</span><span>1,000</span><span>10,000</span></span><span></span></div>
        @for (row of ownerChartRows; track row.label) {
          <div class="chart-row" aria-hidden="true"><span>{{ row.label }}</span><span class="chart-plot">
            @if (row.range) { <span class="chart-range" [style.left.%]="row.left" [style.width.%]="row.width"></span> }
            @else { <span class="chart-marker" [style.left.%]="row.left"></span> }
          </span><span>{{ row.value }}</span></div>
        }
      </div>
      <p>This is an illustrative allocation of network electricity across estimated owners, not a claim that adding or removing one owner changes mining consumption by that amount. The systems also differ in scale, security, decentralization and capacity.</p>
      <h3>Allocated electricity per base-layer transaction</h3>
      <p>For another view, the daily charts report 175,056,440 confirmed Bitcoin transactions and 446,857,103 Ethereum mainnet transactions from 1 July 2024 through 30 June 2025. Dividing each annual network estimate by its base-layer transaction count gives:</p>
      <div class="table-scroll"><table>
        <caption>Annual network electricity allocated across one year of recorded transactions</caption>
        <thead><tr><th>Network and utilization</th><th>Annual electricity estimate</th><th>Recorded transactions</th><th>Allocated electricity per transaction</th></tr></thead>
        <tbody>
          <tr><td>Bitcoin, proof of work</td><td>138 TWh</td><td><a href="https://www.blockchain.com/explorer/charts/n-transactions">{{ bitcoinTransactions | number }}</a></td><td><strong>{{ bitcoinKwhPerTransaction | number:'1.0-0' }} kWh</strong></td></tr>
          <tr><td>Ethereum, proof of stake</td><td><a href="https://ethereum.org/energy-consumption/">0.0026 TWh</a></td><td><a href="https://etherscan.io/chart/tx">{{ ethereumTransactions | number }}</a></td><td><strong>{{ ethereumWhPerTransaction | number:'1.1-1' }} Wh</strong> ({{ ethereumKwhPerTransaction | number:'1.4-4' }} kWh)</td></tr>
          @for (n of nanaTransactionCases; track n.label) {
            <tr><td>NanaCoin board, {{ n.label }}</td><td>4.38–21.9 kWh</td><td>{{ n.transactions | number }} hypothetical local entries</td><td><strong>{{ n.lowWh | number:'1.2-2' }}–{{ n.highWh | number:'1.1-1' }} Wh</strong></td></tr>
          }
        </tbody>
      </table></div>
      <div class="energy-chart transaction-chart" role="img" aria-label="Logarithmic chart of allocated electricity per transaction. Bitcoin is 788 kilowatt-hours. A NanaCoin board ranges from 12 to 60 watt-hours at one transaction per day, 1.2 to 6 watt-hours at ten per day, and 0.12 to 0.6 watt-hours at one hundred per day. Ethereum is 5.8 watt-hours.">
        <p class="chart-title">Electricity allocated per transaction · logarithmic kWh scale</p>
        <div class="chart-axis" aria-hidden="true"><span></span><span class="chart-ticks"><span>0.001</span><span>0.01</span><span>0.1</span><span>1</span><span>10</span><span>100</span><span>1,000 kWh</span></span><span></span></div>
        @for (row of transactionChartRows; track row.label) {
          <div class="chart-row" aria-hidden="true"><span>{{ row.label }}</span><span class="chart-plot">
            @if (row.range) { <span class="chart-range" [style.left.%]="row.left" [style.width.%]="row.width"></span> }
            @else { <span class="chart-marker" [style.left.%]="row.left"></span> }
          </span><span>{{ row.value }}</span></div>
        }
      </div>
      <p>NanaCoin’s row is necessarily a utilization model rather than a measured transaction cost. The board’s estimated 4.38–21.9 kWh annual electricity stays roughly constant whether the household writes one or hundreds of entries per day. At ten entries per day, that budget allocates to 1.2–6 Wh per entry; at one hundred, it falls to 0.12–0.6 Wh.</p>
      <p>These are accounting quotients, not the electricity caused by one more transaction. Mining and validators secure blocks and the network as a whole. A single base-layer transaction can batch many payments, while Bitcoin’s Lightning payments and Ethereum rollup transactions are absent from these base-layer counts. Ethereum itself cautions that per-transaction comparisons are sensitive to what gets counted.</p>
      <p>Fees are related to scarce ledger capacity, but they are not electricity bills. <a href="https://developer.bitcoin.org/devguide/transactions.html#transaction-fees-and-change">Bitcoin fees rise with demand for limited block space</a>, and miners generally favor higher fee rates. Proof-of-work makes Bitcoin’s network electricity-intensive, but the marginal transaction does not consume the quotient above. <a href="https://ethereum.org/developers/docs/gas/">Ethereum gas prices rise with demand and computational complexity</a>. Since Ethereum moved to proof of stake, its cited network estimate is about 0.0026 TWh/year, so a high Ethereum fee primarily reflects demand for computation and block space rather than electricity burned to record that transaction.</p>
      <p>Neither the USB scenarios nor the ratio include your phone, router, static hosting, manufacturing, or upstream USB power-supply losses. The public browser demo uses your device and hosting infrastructure; it does not consume “ESP32 watts.”</p></section>
    <section class="panel"><h2>Nana-nickles: pocket-sized promises</h2>
      <p>Anyone can package coins they own into a voucher, on screen or on paper. Only Nana can issue new money. A bearer voucher belongs, operationally, to whoever can present its secret first. A photograph or copy is another spendable copy. A signature prevents forgery, not copying or double spending.</p>
      <p>The showcase prototype is play money, valid only in this tab. Live Rust vouchers need HTTPS-only issuance/redemption, random secrets stored only as hashes, atomic single-use redemption, bounded liability tracking, and clear lost-token rules. They are not account login tokens.</p></section>`,
  styles: `table{width:100%;border-collapse:collapse}caption{text-align:left;font-weight:700;padding:.5rem 0}th,td{text-align:left;padding:.65rem;border-bottom:1px solid var(--line)}.table-scroll{overflow:auto}.energy-chart{margin:1.25rem 0;padding:1rem;border:1px solid var(--line);border-radius:.6rem;overflow-x:auto}.chart-title{font-weight:700;margin:0 0 .75rem}.chart-axis,.chart-row{min-width:43rem;display:grid;grid-template-columns:10rem minmax(24rem,1fr) 8.5rem;gap:.75rem;align-items:center}.chart-axis{color:var(--muted);font-size:.75rem}.chart-ticks{display:flex;justify-content:space-between;align-items:center}.chart-row{margin:.65rem 0}.chart-plot{position:relative;height:1.25rem;border-inline:1px solid var(--line);background:repeating-linear-gradient(to right,transparent 0,transparent calc(16.666% - 1px),var(--line) calc(16.666% - 1px),var(--line) 16.666%)}.owner-chart .chart-plot{background:repeating-linear-gradient(to right,transparent 0,transparent calc(25% - 1px),var(--line) calc(25% - 1px),var(--line) 25%)}.chart-range{position:absolute;top:.25rem;height:.75rem;min-width:.35rem;border-radius:1rem;background:var(--accent)}.chart-marker{position:absolute;top:.125rem;width:1rem;height:1rem;border-radius:50%;background:var(--accent);transform:translateX(-50%);border:2px solid var(--surface)}@media(max-width:700px){.energy-chart{padding:.75rem}}`,
})
export class AboutPage {
  readonly demo = IS_DEMO;
  readonly cases = ENERGY_CASES;
  readonly bitcoinPerOwnerLow = BITCOIN_KWH / HIGH_OWNER_ESTIMATE;
  readonly bitcoinPerOwnerHigh = BITCOIN_KWH / LOW_OWNER_ESTIMATE;
  readonly bitcoinPerHouseholdLow = this.bitcoinPerOwnerLow * 4;
  readonly bitcoinPerHouseholdHigh = this.bitcoinPerOwnerHigh * 4;
  readonly bitcoinTransactions = BITCOIN_BASE_LAYER_TRANSACTIONS;
  readonly ethereumTransactions = ETHEREUM_BASE_LAYER_TRANSACTIONS;
  readonly bitcoinKwhPerTransaction = BITCOIN_KWH / BITCOIN_BASE_LAYER_TRANSACTIONS;
  readonly ethereumKwhPerTransaction = ETHEREUM_KWH / ETHEREUM_BASE_LAYER_TRANSACTIONS;
  readonly ethereumWhPerTransaction = this.ethereumKwhPerTransaction * 1000;
  readonly nanaTransactionCases = [1, 10, 100].map(perDay => ({
    label: `${perDay} ${perDay === 1 ? 'entry' : 'entries'}/day`,
    transactions: perDay * 365,
    lowWh: NANA_MIN_KWH * 1000 / (perDay * 365),
    highWh: NANA_MAX_KWH * 1000 / (perDay * 365),
  }));
  readonly ownerChartRows = [
    chartRange('NanaCoin board', NANA_MIN_KWH, NANA_MAX_KWH, 0, 4, '4.38–21.9 kWh'),
    chartRange('Bitcoin / nominal owner', this.bitcoinPerOwnerLow, this.bitcoinPerOwnerHigh, 0, 4, '378–1,302 kWh'),
    chartRange('Bitcoin / household of four', this.bitcoinPerHouseholdLow, this.bitcoinPerHouseholdHigh, 0, 4, '1,512–5,208 kWh'),
  ];
  readonly transactionChartRows = [
    chartRange('Bitcoin', this.bitcoinKwhPerTransaction, this.bitcoinKwhPerTransaction, -3, 3, '788 kWh'),
    chartRange('NanaCoin · 1/day', NANA_MIN_KWH / 365, NANA_MAX_KWH / 365, -3, 3, '12–60 Wh'),
    chartRange('NanaCoin · 10/day', NANA_MIN_KWH / 3650, NANA_MAX_KWH / 3650, -3, 3, '1.2–6 Wh'),
    chartRange('NanaCoin · 100/day', NANA_MIN_KWH / 36500, NANA_MAX_KWH / 36500, -3, 3, '0.12–0.6 Wh'),
    chartRange('Ethereum', this.ethereumKwhPerTransaction, this.ethereumKwhPerTransaction, -3, 3, '5.8 Wh'),
  ];
}
