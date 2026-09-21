import { Component, computed, inject, signal } from '@angular/core';
import { DatePipe, DecimalPipe, JsonPipe } from '@angular/common';
import { MachineDiagnostics, MachineInfo, MachineSnapshot } from '../api/diagnostics';
import { ApiBase } from '../api/api-base';
import { LineChart } from '../economy/line-chart';
import { Series } from '../economy/series';

interface Sample { at: number; snapshot: MachineSnapshot }
export const HISTORY_LIMIT = 120;
export function appendSample(history: Sample[], snapshot: MachineSnapshot, at: number): Sample[] {
  const previous = history.at(-1)?.snapshot;
  if (previous && (previous.machine_samples ?? previous.samples) === (snapshot.machine_samples ?? snapshot.samples)
      && previous.uptime_seconds === snapshot.uptime_seconds) return history;
  const kept = previous && snapshot.uptime_seconds < previous.uptime_seconds ? [] : history;
  return [...kept.slice(-(HISTORY_LIMIT - 1)), { at, snapshot }];
}

@Component({
  selector: 'app-diagnostics',
  imports: [DatePipe, DecimalPipe, JsonPipe, LineChart],
  templateUrl: './diagnostics.html',
  styles: `
    .toolbar { display:flex; flex-wrap:wrap; gap:.7rem; align-items:center; margin-bottom:1rem }
    .grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(min(100%, 300px),1fr)); gap:1rem }
    .panel { min-width:0; margin-bottom:1rem }
    dl { display:grid; grid-template-columns:1fr 1fr; gap:.5rem; margin-bottom:0 }
    dt { color:var(--ink-soft) } dd { margin:0; text-align:right; overflow-wrap:anywhere; font-variant-numeric:tabular-nums }
    .warning { padding:.7rem; background:var(--warn-bg); color:var(--warn-ink); border-radius:8px }
    progress { width:100%; accent-color:var(--credit) }
    .table-scroll { overflow-x:auto } table { width:100%; border-collapse:collapse }
    td, th { text-align:left; padding:.5rem; border-bottom:1px solid var(--line); white-space:nowrap }
    pre { overflow:auto; max-height:24rem; font-size:.8rem }
  `,
})
export class DiagnosticsPage {
  private readonly api = inject(MachineDiagnostics);
  protected readonly base = inject(ApiBase);
  protected readonly snapshot = signal<MachineSnapshot | null>(null);
  protected readonly info = signal<MachineInfo | null>(null);
  protected readonly failure = signal('');
  protected readonly infoFailure = signal('');
  protected readonly busy = signal(false);
  protected readonly infoBusy = signal(false);
  protected readonly following = signal(false);
  protected readonly receivedAt = signal<number | null>(null);
  protected readonly latency = signal(0);
  protected readonly stalled = signal(false);
  protected readonly history = signal<Sample[]>([]);
  private readonly lifetime = new AbortController();
  private timer: ReturnType<typeof setTimeout> | undefined;
  private sampleChangedAt = performance.now();
  private source = this.base.current();

  protected readonly memorySeries = computed(() => this.series([
    ['Internal free (KiB)', (s) => s.free_heap / 1024],
    ['Largest block (KiB)', (s) => s.largest_free_block == null ? null : s.largest_free_block / 1024],
  ]));
  protected readonly temperatureSeries = computed(() => this.series([
    ['Die temperature (°C)', (s) => s.temperature_c],
  ]));
  protected readonly rssiSeries = computed(() => this.series([
    ['Wi-Fi RSSI (dBm)', (s) => s.rssi_dbm],
  ]));

  constructor() {
    void this.refresh();
    void this.loadInfo();
    document.addEventListener('visibilitychange', this.visibility);
  }

  private readonly visibility = () => {
    if (!document.hidden && this.following()) void this.refresh();
  };

  private series(fields: [string, (s: MachineSnapshot) => number | null | undefined][]): Series[] {
    return fields.map(([name, field]) => ({ name, points: this.history().flatMap(({ at, snapshot }) => {
      const value = field(snapshot);
      return value == null ? [] : [{ at, value: Math.round(value * 10) / 10 }];
    }) }));
  }

  protected async refresh(): Promise<void> {
    if (this.busy() || this.lifetime.signal.aborted) return;
    clearTimeout(this.timer);
    if (this.source !== this.base.current()) {
      this.source = this.base.current();
      this.history.set([]); this.snapshot.set(null); this.info.set(null);
      void this.loadInfo();
    }
    this.busy.set(true);
    const start = performance.now();
    try {
      const data = await this.api.read<MachineSnapshot>('', this.lifetime.signal);
      if (this.lifetime.signal.aborted) return;
      if (typeof data.samples !== 'number' || typeof data.free_heap !== 'number'
          || typeof data.uptime_seconds !== 'number') {
        throw new Error('This firmware uses an older diagnostics format. The machine dashboard is unavailable.');
      }
      const previous = this.snapshot();
      if (!previous || (data.machine_samples ?? data.samples) !== (previous.machine_samples ?? previous.samples)
          || data.uptime_seconds !== previous.uptime_seconds) {
        this.sampleChangedAt = performance.now();
      }
      this.stalled.set(performance.now() - this.sampleChangedAt > 6000);
      if (previous && data.uptime_seconds < previous.uptime_seconds) void this.loadInfo();
      this.snapshot.set(data);
      this.history.update((h) => appendSample(h, data, Date.now() / 1000));
      this.latency.set(Math.round(performance.now() - start));
      this.receivedAt.set(Date.now());
      this.failure.set('');
    } catch (e) {
      if (!this.lifetime.signal.aborted) this.failure.set(e instanceof Error ? e.message : String(e));
    } finally {
      this.busy.set(false);
      if (this.following() && !this.lifetime.signal.aborted) {
        this.timer = setTimeout(() => {
          if (document.hidden) this.scheduleVisible();
          else void this.refresh();
        }, 5000);
      }
    }
  }

  private scheduleVisible(): void {
    // No hidden-tab polling. The visibility listener resumes when shown.
    clearTimeout(this.timer);
  }

  protected async loadInfo(): Promise<void> {
    if (this.infoBusy() || this.lifetime.signal.aborted) return;
    this.infoBusy.set(true);
    try {
      const info = await this.api.read<MachineInfo>('/static', this.lifetime.signal);
      if (!this.lifetime.signal.aborted) { this.info.set(info); this.infoFailure.set(''); }
    } catch (e) {
      if (!this.lifetime.signal.aborted) this.infoFailure.set(e instanceof Error ? e.message : String(e));
    } finally { this.infoBusy.set(false); }
  }

  protected toggle(): void {
    this.following.update((value) => !value);
    clearTimeout(this.timer);
    if (this.following()) void this.refresh();
  }
  protected bytes(value: number | null | undefined): string {
    if (value == null) return 'Unavailable';
    return value >= 1048576 ? `${(value / 1048576).toFixed(2)} MiB` : `${(value / 1024).toFixed(1)} KiB`;
  }
  protected ip(value: number[] | null | undefined): string { return value?.join('.') ?? 'Unavailable'; }
  protected contiguous(free: number, largest: number | null): string { return largest == null ? 'Unavailable' : `${free ? Math.round(100 * largest / free) : 0}%`; }
  protected resetReason(value: number | null): string {
    if (value == null) return 'Unavailable';
    return ['Unknown', 'Power on', 'External pin', 'Software restart', 'Panic', 'Interrupt watchdog',
      'Task watchdog', 'Watchdog', 'Deep sleep', 'Brownout', 'SDIO', 'USB', 'JTAG', 'eFuse',
      'Power glitch', 'CPU lockup'][value] ?? `Code ${value}`;
  }
  ngOnDestroy(): void {
    this.lifetime.abort();
    clearTimeout(this.timer);
    document.removeEventListener('visibilitychange', this.visibility);
  }
}
