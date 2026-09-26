import { Component, DestroyRef, inject, signal } from '@angular/core';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { finalize } from 'rxjs';
import { SystemInfo } from '../api/system-info';
import { IncidentHistory, IncidentHistoryPanel } from './incident-history';
import { IS_DEMO } from '../demo/demo';

@Component({ selector: 'app-error-log', imports: [IncidentHistoryPanel], template: `
  <h1>Error Log</h1>
  @if (demo) { <p>The browser demo has no board incident recorder. Browser Log contains this tab's client diagnostics.</p> }
  @else {
    <p>Board-side errors and recovery evidence. No sign-in required; no request contents or credentials are logged.</p>
    <button class="btn btn--quiet" [disabled]="busy()" (click)="load()">{{ busy() ? 'Reading…' : 'Refresh error log' }}</button>
    @if (error()) { <p class="warning">{{ error() }} Previously loaded history may be stale.</p> }
    @if (history(); as history) { <app-incident-history [history]="history" /> }
  }
` })
export class ErrorLogPage {
  private readonly api = inject(SystemInfo);
  private readonly destroy = inject(DestroyRef);
  protected readonly demo = IS_DEMO;
  protected readonly history = signal<IncidentHistory | null>(null);
  protected readonly busy = signal(false);
  protected readonly error = signal('');
  constructor() { if (!this.demo) this.load(); }
  protected load(): void {
    if (this.busy()) return;
    this.busy.set(true);
    this.api.read<IncidentHistory>('/diag/events').pipe(takeUntilDestroyed(this.destroy), finalize(() => this.busy.set(false))).subscribe({
      next: value => { this.history.set(value); this.error.set(''); },
      error: () => this.error.set('Could not read the board incident history.'),
    });
  }
}
