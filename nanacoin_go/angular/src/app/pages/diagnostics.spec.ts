import { TestBed } from '@angular/core/testing';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MachineDiagnostics, MachineSnapshot } from '../api/diagnostics';
import { appendSample, DiagnosticsPage, HISTORY_LIMIT } from './diagnostics';

const sample: MachineSnapshot = { schema: 1, samples: 1, uptime_seconds: 2,
  free_heap: 40000, largest_free_block: 20000, minimum_free_heap: 30000,
  psram_free: 6000000, sampler_core: 0, http_core: 1 };

afterEach(() => { TestBed.resetTestingModule(); vi.useRealTimers(); vi.restoreAllMocks(); });

describe('machine diagnostics', () => {
  it('renders TinyGo unsupported metrics without inventing zero graph points', async () => {
    vi.useFakeTimers();
    const tinygo = { ...sample, sampling: 'on_request', machine_samples: 5,
      largest_free_block: null, minimum_free_heap: null, psram: null,
      psram_free: 0, psram_enabled: false, http_core: 0 };
    const read = vi.fn((path: string) => path ? Promise.resolve({
      platform: 'ESP32-S3 / TinyGo', firmware: 'NanaCoin / TinyGo', idf: 'Not used',
      cores: 2, active_cores: 1, cpu_mhz: null, chip_model: null, chip_revision: null,
      reset_reason: null, flash_bytes: null, snapshot_bytes: 12, response_bytes: 192,
      partitions: [], partitions_available: false,
    }) : Promise.resolve(tinygo));
    TestBed.configureTestingModule({ providers: [{ provide: MachineDiagnostics, useValue: { read } }] });
    const fixture = TestBed.createComponent(DiagnosticsPage);
    await vi.advanceTimersByTimeAsync(0);
    fixture.detectChanges();
    expect(fixture.nativeElement.textContent).toContain('single active core');
    expect(fixture.nativeElement.textContent).toContain('Not enabled by this firmware');
    expect(fixture.nativeElement.textContent).toContain('Partition enumeration is unavailable');
    expect(fixture.nativeElement.textContent).not.toContain('NaN');
    const history = appendSample([], tinygo, 1);
    expect(appendSample(history, { ...tinygo, machine_samples: 6 }, 2)).toHaveLength(2);
    fixture.destroy();
  });
  it('caps browser history, ignores duplicate samples, and clears on reboot', () => {
    let history: ReturnType<typeof appendSample> = [];
    for (let i = 1; i <= 150; i++) history = appendSample(history, { ...sample, samples: i, uptime_seconds: i * 2 }, i);
    expect(history.length).toBe(HISTORY_LIMIT);
    expect(history[0].snapshot.samples).toBe(31);
    expect(appendSample(history, history.at(-1)!.snapshot, 151)).toBe(history);
    expect(appendSample(history, sample, 152).length).toBe(1);
  });

  it('does not overlap polls and aborts requests when leaving the page', async () => {
    vi.useFakeTimers();
    let signal: AbortSignal | undefined;
    const read = vi.fn((_path, s) => { signal = s; return new Promise(() => {}); });
    TestBed.configureTestingModule({ providers: [{ provide: MachineDiagnostics, useValue: { read } }] });
    const fixture = TestBed.createComponent(DiagnosticsPage);
    expect(read).toHaveBeenCalledTimes(2); // snapshot + static, each once
    await vi.advanceTimersByTimeAsync(10000);
    expect(read).toHaveBeenCalledTimes(2);
    fixture.destroy();
    expect(signal?.aborted).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
  });

  it('pauses in hidden tabs and resumes when visible', async () => {
    vi.useFakeTimers();
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(false);
    const read = vi.fn((path: string) => path ? Promise.reject(new Error('older firmware')) : Promise.resolve(sample));
    TestBed.configureTestingModule({ providers: [{ provide: MachineDiagnostics, useValue: { read } }] });
    const fixture = TestBed.createComponent(DiagnosticsPage);
    await vi.advanceTimersByTimeAsync(0);
    hidden.mockReturnValue(true);
    await vi.advanceTimersByTimeAsync(10000);
    expect(read).toHaveBeenCalledTimes(2);
    hidden.mockReturnValue(false);
    document.dispatchEvent(new Event('visibilitychange'));
    await vi.advanceTimersByTimeAsync(0);
    expect(read).toHaveBeenCalledTimes(3);
    fixture.destroy();
    expect(vi.getTimerCount()).toBe(0);
  });
});
