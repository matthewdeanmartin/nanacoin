// What this browser did, readable without opening devtools.
//
// The server has had a log page for a while, because diagnosing the board
// from outside meant guessing. This is the same idea pointed the other way:
// the half of a failure that happens in the browser was invisible unless
// someone thought to open the console before it happened - and by then the
// interesting thing has usually already gone past.
//
// Deliberately reachable without a session, like the server's, because the
// failure most worth reading about is the one that stops you logging in.

import { Component, computed, inject, signal } from '@angular/core';

import { Log, LogEntry, LogLevel } from '../api/log';
import { Toasts } from '../ui/toasts';

@Component({
  selector: 'app-clientlog',
  template: `
    <h2>What this browser did</h2>

    <p class="muted small">
      The last {{ entries().length }} things the app did, newest first. Kept in
      memory only — it goes away when the tab does, and nothing here leaves
      your machine unless you copy it.
    </p>

    <div class="logbar">
      <label class="checkbox">
        <input type="checkbox" [checked]="problemsOnly()"
               (change)="problemsOnly.set($any($event.target).checked)" />
        Problems only
      </label>

      <label class="checkbox">
        <input type="checkbox" [checked]="log.toConsole()"
               (change)="log.toConsole.set($any($event.target).checked)" />
        Also print to the console
      </label>

      <button class="btn btn--quiet btn--small" (click)="copy()">Copy</button>
      <button class="btn btn--quiet btn--small" (click)="log.clear()">Clear</button>
    </div>

    @if (entries().length === 0) {
      <p class="muted">Nothing recorded yet.</p>
    } @else {
      <div class="clientlog">
        @for (e of entries(); track e.seq) {
          <div class="clientlog__row" [class]="'clientlog__row--' + e.level">
            <span class="clientlog__when">{{ time(e) }}</span>
            <span class="clientlog__level">{{ e.level }}</span>
            <span class="clientlog__scope">{{ e.scope }}</span>
            <span class="clientlog__msg">
              {{ e.message }}
              @if (e.detail) {
                <span class="clientlog__detail">{{ detail(e) }}</span>
              }
            </span>
          </div>
        }
      </div>
    }
  `,
})
export class ClientLogPage {
  protected readonly log = inject(Log);
  private readonly toasts = inject(Toasts);

  protected readonly problemsOnly = signal(false);

  protected readonly entries = computed<LogEntry[]>(() => {
    // Reading revision is what makes this recompute as entries arrive; the
    // array itself is deliberately not a signal, so that an HTTP-heavy page
    // does not churn change detection for a log nobody is looking at.
    this.log.revision();

    const all = this.log.recent();
    if (!this.problemsOnly()) return all;
    return all.filter((e) => e.level === 'warn' || e.level === 'error');
  });

  protected time(e: LogEntry): string {
    return new Date(e.at).toISOString().slice(11, 23);
  }

  protected detail(e: LogEntry): string {
    return e.detail ? JSON.stringify(e.detail) : '';
  }

  protected async copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(this.log.asText());
      this.toasts.ok('Copied.');
    } catch {
      // Clipboard access is refused in plenty of ordinary situations - an
      // insecure origin, a browser that wants a user gesture it did not see.
      // Saying so is better than a button that silently does nothing.
      this.toasts.error('Could not copy. Select the text and copy it by hand.');
    }
  }
}

/** Levels in the order a reader cares about them. */
export const LEVELS: LogLevel[] = ['error', 'warn', 'info', 'debug'];
