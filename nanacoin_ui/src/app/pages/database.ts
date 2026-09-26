import { Component, DestroyRef, inject, signal } from '@angular/core';
import { DecimalPipe, DatePipe } from '@angular/common';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { finalize } from 'rxjs';
import { DatabaseInfo, QueryBenchmark, SystemInfo } from '../api/system-info';
import { IS_DEMO } from '../demo/demo';
import { MachineSnapshot } from '../api/diagnostics';

@Component({
  selector: 'app-database', imports: [DecimalPipe, DatePipe],
  template: `<h1>Database diagnostics</h1>
    <p>Read-only inventory of persistent and temporary board data. Counts and limits, without private record contents.</p>
    @if (demo) { <p>This browser demo has no board journal or NVS database. Connect to the Rust board to inspect its database and benchmark its queries.</p> }
    @else {
      <button class="btn btn--quiet" (click)="load()" [disabled]="busy() || benchmarking()">{{ busy() ? 'Reading…' : 'Refresh inventory' }}</button>
      @if (error()) { <p class="warning">{{ error() }} Previously loaded results may be stale.</p> }
      @if (data(); as d) {
        <section class="panel"><h2>Storage and integrity</h2>
          <p>{{ d.engine }}</p><p>{{ d.indexes }}</p>
          @if (machine()?.ledger_storage; as nvs) {
            <p>Physical ledger NVS entries: {{ nvs.used_entries | number }} used / {{ nvs.free_entries | number }} free / {{ nvs.total_entries | number }} total; {{ nvs.available_entries | number }} available.
              Sampled at board uptime {{ (machine()?.storage_sampled_at_ms ?? 0) / 1000 | number:'1.0-0' }} seconds. Entries are storage accounting units, not records or bytes.</p>
          } @else { <p class="muted">Physical NVS accounting is not available from this backend. Logical record counts below remain available.</p> }
          <dl><dt>Generation / sequence</dt><dd>{{ d.generation }} / {{ d.sequence }}</dd>
            <dt>Writes</dt><dd>{{ d.storage_failed ? 'Latched read-only after a storage failure' : 'Storage has not latched a failure' }}</dd>
            <dt>Accounting / state invariants</dt><dd>{{ d.invariants_ok ? 'Pass' : 'FAIL' }}</dd>
            <dt>Journal records</dt><dd>{{ d.journal_records | number }} used / {{ d.journal_free_records | number }} free / {{ d.journal_record_capacity | number }} maximum</dd>
            <dt>Logical journal bytes</dt><dd>{{ d.journal_logical_bytes | number }} ({{ d.journal_frame_bytes }} per frame; excludes physical storage overhead)</dd>
            <dt>Checkpoint support</dt><dd>{{ d.checkpoint_supported ? 'Enabled' : 'Unavailable on this storage backend' }}</dd>
            <dt>Checkpoint rotation trigger</dt><dd>{{ d.checkpoint_after | number }} journal records, when supported</dd>
            <dt>Active checkpoint rows</dt><dd>{{ d.checkpoint_rows | number }} / {{ d.checkpoint_row_capacity | number }} maximum; up to {{ d.checkpoint_row_max_bytes | number }} bytes per encoded row</dd>
            <dt>Lifetime / retained transactions</dt><dd>{{ d.lifetime_transactions | number }} lifetime; retained range {{ d.oldest_retained_sequence ?? 'none' }}–{{ d.newest_retained_sequence ?? 'none' }}</dd>
            <dt>Oldest retained timestamp</dt><dd>{{ d.oldest_retained_at ? (d.oldest_retained_at * 1000 | date:'medium') : 'None / unset clock' }}</dd>
            <dt>Newest retained timestamp</dt><dd>{{ d.newest_retained_at ? (d.newest_retained_at * 1000 | date:'medium') : 'None / unset clock' }}</dd>
            <dt>Model reserved RAM estimate</dt><dd>{{ d.model_reserved_bytes / 1024 | number:'1.1-1' }} KiB</dd>
          </dl><p class="muted">{{ d.memory_note }}</p>
        </section>
        <section><h2>Data types</h2><p>Active means enabled/open/outstanding, or unexpired for authentication caches. Expired cache slots still count as occupied until reused.</p>
          <div class="table-scroll"><table><thead><tr><th>Data type</th><th>Persistence</th><th>Used / free / limit</th><th>Active</th><th>Reserved payload KiB</th><th>Retention</th></tr></thead>
            <tbody>@for (c of d.collections; track c.name) { <tr><th>{{ c.name }}</th><td>{{ c.persistence }}</td>
              <td>{{ c.used }} / {{ c.free }} / {{ c.capacity }}</td><td>{{ c.active ?? '—' }}</td>
              <td>{{ c.payload_reserved_bytes ? (c.payload_reserved_bytes / 1024 | number:'1.1-1') : 'Included in model total' }}</td><td>{{ c.retention }}</td></tr> }</tbody>
          </table></div>
        </section>
      }
      <section class="panel"><h2>Read-only query benchmarks</h2>
        <p>Runs existing status, ledger, listings and transaction queries against current data. No new records, writes, checkpointing or cleanup. Server timings exclude network/TLS and lock wait; this is not a concurrency test.</p>
        <button class="btn" (click)="run()" [disabled]="benchmarking() || busy() || !data()">{{ benchmarking() ? 'Running reads…' : 'Run query benchmarks (read-only)' }}</button>
        @if (benchError()) { <p class="warning">{{ benchError() }}</p> }
        @if (benchmark(); as b) {
          <p>Generation {{ b.generation }}, sequence {{ b.sequence }} · {{ b.elapsed_us / 1000 | number:'1.2-2' }} ms server total · {{ roundTrip() | number:'1.0-0' }} ms client round trip</p>
          <div class="table-scroll"><table><thead><tr><th>Query</th><th>Runs</th><th>Min / mean / max ms</th><th>Response bytes</th><th>Result</th></tr></thead><tbody>
            @for (q of b.queries; track q.name) { <tr><td>{{ q.name }}</td><td>{{ q.runs }}</td><td>{{ q.min_us / 1000 | number:'1.3-3' }} / {{ q.mean_us / 1000 | number:'1.3-3' }} / {{ q.max_us / 1000 | number:'1.3-3' }}</td><td>{{ q.response_bytes | number }}</td><td>{{ q.error === 'not_found' ? 'No retained transaction to look up' : (q.error ?? (q.runs ? 'OK' : 'Time budget reached')) }}</td></tr> }
          </tbody></table></div><p class="muted">{{ b.note }}</p>
        }
      </section>
    }`,
  styles: `dl { display:grid; grid-template-columns:minmax(10rem,1fr) 2fr; gap:.6rem } dd { margin:0 } .panel { margin-block:1rem }
    .table-scroll { overflow-x:auto } table { width:100%; border-collapse:collapse } td,th { text-align:left; vertical-align:top; padding:.6rem; border-bottom:1px solid var(--line) }`,
})
export class DatabasePage {
  private readonly api = inject(SystemInfo);
  private readonly destroy = inject(DestroyRef);
  protected readonly demo = IS_DEMO;
  protected readonly data = signal<DatabaseInfo | null>(null);
  protected readonly machine = signal<MachineSnapshot | null>(null);
  protected readonly benchmark = signal<QueryBenchmark | null>(null);
  protected readonly busy = signal(false);
  protected readonly benchmarking = signal(false);
  protected readonly error = signal('');
  protected readonly benchError = signal('');
  protected readonly roundTrip = signal(0);
  constructor() { if (!this.demo) this.load(); }
  protected load(): void {
    if (this.busy() || this.benchmarking()) return;
    this.busy.set(true);
    this.api.read<MachineSnapshot>('/diag').pipe(takeUntilDestroyed(this.destroy)).subscribe({
      next: snapshot => this.machine.set(snapshot), error: () => this.machine.set(null),
    });
    this.api.read<DatabaseInfo>('/diag/database').pipe(takeUntilDestroyed(this.destroy), finalize(() => this.busy.set(false))).subscribe({
      next: data => { this.data.set(data); this.error.set(''); }, error: () => this.error.set('Database diagnostics unavailable. Check the connection and Rust firmware.'),
    });
  }
  protected run(): void {
    if (this.benchmarking() || this.busy() || !this.data()) return;
    this.benchmarking.set(true); this.benchError.set(''); this.benchmark.set(null);
    const start = performance.now();
    this.api.read<QueryBenchmark>('/diag/database/benchmark').pipe(takeUntilDestroyed(this.destroy), finalize(() => this.benchmarking.set(false))).subscribe({
      next: data => { this.benchmark.set(data); this.roundTrip.set(performance.now() - start); },
      error: () => this.benchError.set('Benchmark did not complete. No write operation was requested.'),
    });
  }
}
