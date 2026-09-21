import { Injectable, inject } from '@angular/core';
import { ApiBase } from './api-base';

export interface HeapInfo {
  total: number; free: number; largest: number | null; minimum: number | null;
  allocated_blocks: number | null; free_blocks: number | null;
}
export interface MachineSnapshot {
  schema?: number;
  uptime_seconds: number; free_heap: number; largest_free_block: number | null;
  minimum_free_heap: number | null; psram_free: number; samples: number;
  sampling?: string; memory_scope?: string; psram_enabled?: boolean; machine_samples?: number;
  sampler_core: number; http_core: number;
  sampled_at_ms?: number; internal?: HeapInfo; psram?: HeapInfo | null;
  temperature_c?: number | null; rssi_dbm?: number | null; wifi_channel?: number | null;
  ip?: number[] | null; gateway?: number[] | null; netmask?: number[] | null;
  unix_seconds?: number | null; tasks?: number | null; sampler_stack_free_min_bytes?: number | null;
  boot_ready_ms?: number | null; requests?: number | null; errors?: number | null;
  ledger_storage?: { used_entries: number; free_entries: number; available_entries: number; total_entries: number } | null;
  storage_sampled_at_ms?: number;
}
export interface MachineInfo {
  platform: string; firmware: string; idf: string; chip_model: number | null;
  chip_revision: number | null; cores: number; cpu_mhz: number | null; reset_reason: number | null;
  active_cores?: number; response_mode?: string; partitions_available?: boolean;
  flash_bytes: number | null; snapshot_bytes: number; response_bytes: number;
  partitions: { name: string; kind: number; subtype: number; offset: number; size: number; encrypted: boolean }[];
  partitions_truncated: boolean;
}

@Injectable({ providedIn: 'root' })
export class MachineDiagnostics {
  private readonly base = inject(ApiBase);

  async read<T>(path: string, signal: AbortSignal): Promise<T> {
    const controller = new AbortController();
    const abort = () => controller.abort();
    if (signal.aborted) abort();
    signal.addEventListener('abort', abort, { once: true });
    const timeout = setTimeout(abort, 6000);
    try {
      const response = await fetch(`${this.base.current()}/diag${path}`, {
        signal: controller.signal, cache: 'no-store', credentials: 'omit',
      });
      if (response.status === 404) throw new Error('This firmware does not provide these diagnostics.');
      if (!response.ok) throw new Error(`Diagnostics returned HTTP ${response.status}.`);
      return await response.json() as T;
    } finally {
      clearTimeout(timeout);
      signal.removeEventListener('abort', abort);
    }
  }
}
