import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { timeout } from 'rxjs';
import { ApiBase } from './api-base';

export interface CollectionInfo {
  name: string; persistence: string; used: number; capacity: number; free: number;
  active: number | null; payload_reserved_bytes: number; retention: string;
}
export interface DatabaseInfo {
  engine: string; generation: number; sequence: number; storage_failed: boolean; invariants_ok: boolean;
  model_reserved_bytes: number; memory_note: string; checkpoint_supported: boolean;
  checkpoint_rows: number; checkpoint_row_capacity: number; checkpoint_row_max_bytes: number;
  journal_records: number; journal_record_capacity: number; journal_free_records: number;
  checkpoint_after: number; journal_frame_bytes: number; journal_logical_bytes: number;
  lifetime_transactions: number; oldest_retained_sequence: number | null; newest_retained_sequence: number | null;
  oldest_retained_at: number | null; newest_retained_at: number | null;
  collections: CollectionInfo[]; indexes: string;
}
export interface QueryBenchmark {
  generation: number; sequence: number; read_only: boolean; budget_ms: number; elapsed_us: number; note: string;
  queries: { name: string; runs: number; min_us: number; mean_us: number; max_us: number; response_bytes: number; error: string | null }[];
}
export interface EconomyConfiguration {
  household_name: string; currency: string; decimals: number; minor_units_per_coin: number;
  smallest_unit: string; money_epoch: number; initial_grant: number; offer_settles_after: number;
  usd_decimals: number; maximum_amount_minor: number; lending_enabled: boolean; lending_policy: string;
  rate_basis_points_per_percent: number; savings_lotto_holding_seconds: number; terms_policy: string;
}

@Injectable({ providedIn: 'root' })
export class SystemInfo {
  private readonly http = inject(HttpClient);
  private readonly base = inject(ApiBase);
  /** Public allowlisted GETs only; no token, mutation or automatic retry. */
  read<T>(path: '/configuration' | '/diag' | '/diag/database' | '/diag/database/benchmark' | '/diag/events') {
    return this.http.get<T>(this.base.current() + path).pipe(timeout(10000));
  }
}
