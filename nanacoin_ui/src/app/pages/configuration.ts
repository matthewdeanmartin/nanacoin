import { Component, DestroyRef, inject, signal } from '@angular/core';
import { DecimalPipe } from '@angular/common';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { finalize } from 'rxjs';
import { EconomyConfiguration, SystemInfo } from '../api/system-info';
import { moneyText } from '../api/money';

@Component({
  selector: 'app-configuration', imports: [DecimalPipe],
  template: `<h1>Configuration</h1>
    <p>Household economy settings, visible to everyone. This page is read-only.</p>
    <button class="btn btn--quiet" (click)="load()" [disabled]="busy()">{{ busy() ? 'Reading…' : 'Refresh' }}</button>
    @if (error()) { <p class="warning">{{ error() }} Previously loaded values may be stale.</p> }
    @if (config(); as c) {
      <section class="panel"><h2>Household and money</h2><dl>
        <dt>Household</dt><dd>{{ c.household_name || 'Not provisioned' }}</dd>
        <dt>Currency</dt><dd>{{ c.currency }}</dd>
        <dt>Decimal places</dt><dd>{{ c.decimals }}</dd>
        <dt>One whole coin</dt><dd>{{ c.minor_units_per_coin | number }} stored minor units</dd>
        <dt>Smallest amount</dt><dd>{{ amount(1, c.decimals) }} {{ c.currency }}</dd>
        <dt>Currency revision</dt><dd>{{ c.money_epoch }}</dd>
        <dt>Default new-member grant</dt><dd>{{ amount(c.initial_grant, c.decimals) }} {{ c.currency }}</dd>
        <dt>Maximum amount</dt><dd>{{ amount(c.maximum_amount_minor, c.decimals) }} {{ c.currency }}</dd>
        <dt>Dollar decimal places</dt><dd>{{ c.usd_decimals }}</dd>
      </dl><p class="muted">{{ c.smallest_unit }}</p></section>
      <section class="panel"><h2>Trading, loans and lotto</h2><dl>
        <dt>Offer settlement window</dt><dd>{{ c.offer_settles_after / 3600 | number }} hours ({{ c.offer_settles_after | number }} seconds)</dd>
        <dt>Lending</dt><dd>{{ c.lending_enabled ? 'Enabled' : 'Disabled' }}</dd>
        <dt>Interest-rate units</dt><dd>{{ c.rate_basis_points_per_percent }} basis points = 1%</dd>
        <dt>Delayed / savings lotto holding period</dt><dd>{{ c.savings_lotto_holding_seconds / 86400 | number }} days</dd>
      </dl><p>{{ c.lending_policy }}</p><p>{{ c.terms_policy }}</p></section>
    }`,
  styles: `dl { display:grid; grid-template-columns:minmax(10rem,1fr) 2fr; gap:.7rem } dd { margin:0; overflow-wrap:anywhere } .panel { margin-block:1rem }`,
})
export class ConfigurationPage {
  private readonly api = inject(SystemInfo);
  private readonly destroy = inject(DestroyRef);
  protected readonly config = signal<EconomyConfiguration | null>(null);
  protected readonly busy = signal(false);
  protected readonly error = signal('');
  protected readonly amount = moneyText;
  constructor() { this.load(); }
  protected load(): void {
    if (this.busy()) return;
    this.busy.set(true);
    this.api.read<EconomyConfiguration>('/configuration').pipe(takeUntilDestroyed(this.destroy), finalize(() => this.busy.set(false))).subscribe({
      next: data => { this.config.set(data); this.error.set(''); },
      error: () => this.error.set('Could not read household configuration. Check the connection and firmware.'),
    });
  }
}
