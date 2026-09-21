// Server logs: what the server decided, read back from the server.
//
// This page exists because diagnosing the board from a browser is otherwise
// guesswork. A browser reports a missing CORS header for a route that does not
// exist, a network error for a request that was answered, and nothing at all
// for a request that never left. The server knows which of those happened, so
// this asks it.
//
// Deliberately reachable without logging in, and without a working session:
// the failure most worth diagnosing is the one that stops you logging in.

import { Component, inject, signal } from '@angular/core';

import { ApiBase } from '../api/api-base';
import { LogEvent } from '../api/models';
import { NanacoinService } from '../api/nanacoin.service';
import { Toasts } from '../ui/toasts';

@Component({
  selector: 'app-logs',
  templateUrl: './logs.html',
  styles: `
    .logs {
      display: grid;
      gap: 0.25rem;
      /* Monospace and tabular figures so statuses and paths line up
         vertically, which is how you spot the odd one out. */
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 0.8125rem;
      font-variant-numeric: tabular-nums;
    }
    .logrow {
      display: flex;
      gap: 0.6rem;
      align-items: baseline;
      padding: 0.35rem 0.6rem;
      background: var(--surface);
      border: 1px solid var(--line);
      border-left-width: 3px;
      border-radius: 6px;
    }
    .logrow--warn {
      border-left-color: var(--warn-ink);
    }
    .logrow--error {
      border-left-color: var(--debit);
      background: var(--warn-bg);
    }
    .logrow__seq {
      color: var(--ink-soft);
      min-width: 3ch;
      text-align: right;
    }
    .logrow__kind {
      font-weight: 700;
      min-width: 11ch;
    }
    .logrow--warn .logrow__kind,
    .logrow--error .logrow__kind {
      color: var(--warn-ink);
    }
    .logrow__detail {
      flex: 1;
      min-width: 0;
      overflow-wrap: anywhere;
    }
    .logrow__when {
      color: var(--ink-soft);
      white-space: nowrap;
    }
    .logs__bar {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      flex-wrap: wrap;
      margin-bottom: 0.75rem;
    }
    .logs__count {
      color: var(--ink-soft);
      font-size: 0.875rem;
      margin-left: auto;
    }
  `,
})
export class LogsPage {
  private readonly api = inject(NanacoinService);
  private readonly toasts = inject(Toasts);
  protected readonly apiBase = inject(ApiBase);

  protected readonly events = signal<LogEvent[]>([]);
  protected readonly total = signal(0);

  /** The board's heap, where the server reports one. Absent on a desktop. */
  protected readonly health = signal('');
  protected readonly loading = signal(false);
  protected readonly failed = signal('');

  /** Hides the routine 200s, which are most of them. */
  protected readonly problemsOnly = signal(false);

  private timer: number | null = null;
  protected readonly following = signal(false);

  constructor() {
    void this.load();
  }

  protected visible(): LogEvent[] {
    const all = this.events();
    return this.problemsOnly() ? all.filter((e) => e.level !== 'info') : all;
  }

  protected async load(): Promise<void> {
    this.loading.set(true);
    try {
      const page = await this.api.logs(60);
      this.events.set(page.events);
      this.total.set(page.total);
      this.health.set(page.health ?? '');
      this.failed.set('');
    } catch (e) {
      // If even this fails, the server is genuinely unreachable - which is
      // itself the diagnosis, so it is reported here rather than as a toast
      // that disappears.
      this.failed.set(e instanceof Error ? e.message : 'Could not read the logs.');
    } finally {
      this.loading.set(false);
    }
  }

  /** Polls while the page is open, for watching a failure happen live. */
  protected toggleFollow(): void {
    if (this.timer !== null) {
      window.clearInterval(this.timer);
      this.timer = null;
      this.following.set(false);
      return;
    }
    this.timer = window.setInterval(() => void this.load(), 2000);
    this.following.set(true);
  }

  ngOnDestroy(): void {
    if (this.timer !== null) window.clearInterval(this.timer);
  }

  /** Copies the visible log, for pasting into a bug report. */
  protected async copy(): Promise<void> {
    const text = this.visible()
      .map((e) => `${e.seq}\t${e.level}\t${e.kind}\t${e.detail}`)
      .join('\n');
    try {
      await navigator.clipboard.writeText(text);
      this.toasts.ok('Log copied.');
    } catch {
      this.toasts.error('Could not copy - the browser refused clipboard access.');
    }
  }

  protected when(unixSeconds: number): string {
    // A board with no RTC reports 0 until something sets the clock; the
    // sequence number is the ordering that always works, so an absent
    // timestamp is shown as absent rather than as 1970.
    if (!unixSeconds) return '—';
    return new Date(unixSeconds * 1000).toLocaleTimeString();
  }

  /** True once the ring has wrapped and older events have been dropped. */
  protected wrapped(): boolean {
    return this.total() > this.events().length;
  }
}
