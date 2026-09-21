import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { Money, moneyText } from '../api/money';
import { ReformInput, ReformResult } from '../api/models';
import { NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session } from '../api/session';

@Component({
  selector: 'app-currency-reform', imports: [FormsModule],
  template: `<section class="panel">
    <h2>Currency precision & decimal reform</h2>
    <p>Current precision: {{money.decimals()}} decimal places. A reform changes the unit for balances, debts, prices, and retained history together. It neither creates nor destroys value.</p>
    <label>New decimal places (0–8) <input type="number" min="0" max="8" [(ngModel)]="decimals" (ngModelChange)="clear()" /></label>
    <label>Conversion power (−12 to 12) <input type="number" min="-12" max="12" [(ngModel)]="power" (ngModelChange)="clear()" /></label>
    <p>1 new NC = {{10 ** power}} old NC. Use 0 to change precision without redenominating.</p>
    <p class="muted small">Reforms that would round any balance, price, debt, or retained amount are rejected. More decimal places can preserve small holdings during a redenomination.</p>
    <button class="btn btn--quiet" [disabled]="busy()" (click)="preview()">Preview exact conversion</button>
    @if (result(); as preview) {
      <p>Money supply after reform: <strong>{{formatted(preview)}} NC</strong>. Every amount was checked for exact conversion. The preview is invalidated if the ledger changes.</p>
      <button class="btn" [disabled]="busy()" (click)="apply()">Apply reform to the household</button>
    }
    @if (error()) { <p role="alert">{{error()}}</p> }
  </section>`,
})
export class CurrencyReform {
  protected readonly money = inject(Money);
  private readonly api = inject(NanacoinService);
  private readonly session = inject(Session);
  protected decimals = this.money.decimals(); protected power = 0;
  protected readonly result = signal<ReformResult | null>(null);
  protected readonly busy = signal(false); protected readonly error = signal('');
  private input: ReformInput | null = null; private key = '';
  protected clear(): void { this.result.set(null); this.input = null; }
  protected formatted(result: ReformResult): string { return moneyText(result.circulation, result.decimals, this.money.locale); }
  protected async preview(): Promise<void> {
    this.busy.set(true); this.error.set(''); this.clear();
    try {
      const status = await this.session.loadStatus();
      this.input = { decimals: this.decimals, power: this.power, expected_epoch: status.money_epoch!, expected_sequence: status.sequence!, preview: true };
      this.key = newIdempotencyKey(); this.result.set(await this.api.reform(this.input, this.key));
    } catch (e) { this.error.set(e instanceof Error ? `${e.message} Try more decimal places if the conversion would lose fractions.` : String(e)); }
    finally { this.busy.set(false); }
  }
  protected async apply(): Promise<void> {
    if (!this.input || !this.result() || this.busy()) return;
    this.busy.set(true); this.error.set('');
    try { await this.api.reform({ ...this.input, preview: false }, this.key); await this.session.refresh(); }
    catch (e) { this.error.set(e instanceof Error ? e.message : String(e)); }
    finally { this.busy.set(false); }
  }
}
