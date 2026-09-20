import { Component } from '@angular/core';
import { RouterLink } from '@angular/router';
import { IS_DEMO } from '../demo/demo';

export const ENERGY_CASES = [
  { name: 'Best planning case: light household use, always on', watts: 0.5 },
  { name: 'Expected planning case: connected board with occasional requests', watts: 1 },
  { name: 'Worst planning case: nonstop API/TLS calls, all year', watts: 2.5 },
].map(c => ({ ...c, kwh: c.watts * 8760 / 1000 }));

@Component({
  selector: 'app-about', imports: [RouterLink],
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
      <p>For context, <a href="https://www.jbs.cam.ac.uk/2025/cambridge-study-sustainable-energy-rising-in-bitcoin-mining/">Cambridge’s April 2025 report</a> estimated the entire Bitcoin mining network at <strong>138 TWh/year</strong> = 138 billion kWh/year. This is a dated research estimate, not a live counter.</p>
      <p>That is roughly 6.3–31.5 billion times these one-board scenarios. This compares one household appliance with a global network—not equal scale, security, decentralization, capacity, or energy per transaction. It does not imply NanaCoin could replace Bitcoin at that power.</p>
      <p>Neither the USB scenarios nor the ratio include your phone, router, static hosting, manufacturing, or upstream USB power-supply losses. The public browser demo uses your device and hosting infrastructure; it does not consume “ESP32 watts.”</p></section>
    <section class="panel"><h2>Nana-nickles: pocket-sized promises</h2>
      <p>Anyone can package coins they own into a voucher, on screen or on paper. Only Nana can issue new money. A bearer voucher belongs, operationally, to whoever can present its secret first. A photograph or copy is another spendable copy. A signature prevents forgery, not copying or double spending.</p>
      <p>The showcase prototype is play money, valid only in this tab. Live Rust vouchers need HTTPS-only issuance/redemption, random secrets stored only as hashes, atomic single-use redemption, bounded liability tracking, and clear lost-token rules. They are not account login tokens.</p></section>`,
  styles: `table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:.65rem;border-bottom:1px solid var(--line)}.table-scroll{overflow:auto}`,
})
export class AboutPage { readonly demo = IS_DEMO; readonly cases = ENERGY_CASES; }
