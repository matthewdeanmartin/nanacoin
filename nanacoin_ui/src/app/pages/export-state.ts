// Nana's "Export server state" page: gathers everything that would have to be
// re-entered after a wipe and saves it as one ordinary HTML file.

import { Component, inject, signal } from '@angular/core';
import { ApiBase } from '../api/api-base';
import { Log } from '../api/log';
import { Money } from '../api/money';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { loadAllowances } from './allowances';
import { ServerState, exportFileName, exportHtml } from '../system/export-state';
import { Toasts } from '../ui/toasts';

@Component({
  selector: 'app-export-state',
  template: `
    <h1>Export server state</h1>
    <p class="lede">Save everything you would have to type in again if the NanaCoin board were wiped or replaced: household settings, members and their balances, listings, loans, lottos, exchange offers, open jobs, and the allowances saved in this browser.</p>
    @if (!session.isNana()) { <p class="muted">Only Nana can export the server state.</p> }
    @else {
      <p>The file is an ordinary web page. Open it in any browser, print it, or keep it with your records. It has no passwords and no transaction history, but it does list everyone's balances, so keep it private.</p>
      <button class="btn" type="button" [disabled]="busy()" (click)="download()">{{ busy() ? 'Gathering…' : 'Download export' }}</button>
      @if (summary(); as s) { <p role="status">{{ s }}</p> }
    }
  `,
})
export class ExportStatePage {
  protected readonly session = inject(Session);
  private readonly api = inject(NanacoinService);
  private readonly base = inject(ApiBase);
  private readonly money = inject(Money);
  private readonly toasts = inject(Toasts);
  private readonly log = inject(Log);
  protected readonly busy = signal(false);
  protected readonly summary = signal('');

  protected async download(): Promise<void> {
    if (this.busy()) return;
    this.busy.set(true);
    try {
      const state = await this.gather();
      const html = exportHtml(state, {
        coins: (minor) => this.money.format(minor),
        date: (s) => new Date(s * 1000).toLocaleString(),
      });
      const name = exportFileName(state.config?.household_name || state.status?.household || 'nanacoin', state.exportedAt);
      const url = URL.createObjectURL(new Blob([html], { type: 'text/html' }));
      const link = document.createElement('a');
      link.href = url; link.download = name;
      document.body.appendChild(link); link.click(); link.remove();
      setTimeout(() => URL.revokeObjectURL(url), 10_000);
      this.log.info('export', 'server state exported', { file: name, missing: state.missing });
      this.summary.set(`Saved ${name}: ${state.users.length} members, ${state.listings.filter((l) => l.status === 'ACTIVE').length} listings, ${state.loans.length} loans, ${state.lottos.length} lottos.${state.missing.length ? ` Missing: ${state.missing.join(', ')}.` : ''}`);
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.busy.set(false);
    }
  }

  /** Reads every section; a failed section is noted, not fatal. */
  private async gather(): Promise<ServerState> {
    const missing: string[] = [];
    const safe = async <T>(label: string, read: () => Promise<T>, fallback: T): Promise<T> => {
      try { return await read(); } catch (e) { missing.push(label); this.log.warn('export', `could not read ${label}`, { error: String(e) }); return fallback; }
    };
    const [status, config, transport, users, listings, things, loans, lottos, quotes, offers, fulfillments] = await Promise.all([
      safe('status', () => this.api.status(), null),
      safe('settings', () => this.api.config() as Promise<ServerState['config']>, null),
      safe('HTTPS policy', () => this.api.transport(), null),
      safe('members', () => this.api.users().then((r) => r.users), []),
      safe('listings', () => this.api.listings().then((r) => r.listings), []),
      safe('catalog', () => this.api.things().then((r) => r.things), []),
      safe('loans', () => this.api.loans().then((r) => r.loans), []),
      safe('lottos', () => this.api.lottos().then((r) => r.lottos), []),
      safe('exchange offers', () => this.api.quotes().then((r) => r.quotes), []),
      safe('listing offers', () => this.api.offers().then((r) => r.offers), []),
      safe('open jobs', () => this.api.fulfillments().then((r) => r.fulfillments), []),
    ]);
    return {
      exportedAt: Math.floor(Date.now() / 1000),
      source: this.base.description(),
      status, config, transport, users, listings, things, loans, lottos, quotes, offers, fulfillments,
      // Every allowance saved in this browser, whichever account set it up.
      allowances: loadAllowances(),
      missing,
    };
  }
}
