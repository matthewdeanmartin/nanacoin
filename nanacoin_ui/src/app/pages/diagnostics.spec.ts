import { TestBed } from '@angular/core/testing';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MachineDiagnostics, MachineSnapshot } from '../api/diagnostics';
import { appendSample, DiagnosticsPage, HISTORY_LIMIT } from './diagnostics';

const sample: MachineSnapshot = { schema: 1, samples: 1, uptime_seconds: 2,
  free_heap: 40000, largest_free_block: 20000, minimum_free_heap: 30000,
  psram_free: 6000000, sampler_core: 0, http_core: 1 };

afterEach(() => { TestBed.resetTestingModule(); vi.useRealTimers(); vi.restoreAllMocks(); });

describe('machine diagnostics', () => {
  it('retains incident history with a stale warning after a failed retrieval', async () => {
    vi.useFakeTimers();
    let fail = false;
    const history = { boot_id: 42, retained_bytes: 3000, volatile: true,
      dropped: 0, overwritten: 0, high_water: [2, 8, 4], last_handshake_ms: 500,
      max_handshake_ms: 1000, counters: [], events: [], samples: [] };
    const read = vi.fn((path: string) => {
      if (!path) return Promise.resolve(sample);
      if (path === '/events') return fail ? Promise.reject(new Error('connection lost')) : Promise.resolve(history);
      return Promise.reject(new Error('static unavailable'));
    });
    TestBed.configureTestingModule({ providers: [{ provide: MachineDiagnostics, useValue: { read } }] });
    const fixture = TestBed.createComponent(DiagnosticsPage);
    await vi.advanceTimersByTimeAsync(0);
    fixture.detectChanges();
    expect(fixture.nativeElement.textContent).toContain('Boot 42');
    fail = true;
    const refresh = [...fixture.nativeElement.querySelectorAll('button')]
      .find((button: HTMLButtonElement) => button.textContent?.trim() === 'Refresh') as HTMLButtonElement;
    refresh.click();
    await vi.advanceTimersByTimeAsync(0);
    fixture.detectChanges();
    expect(fixture.nativeElement.textContent).toContain('Boot 42');
    expect(fixture.nativeElement.textContent).toContain('connection lost');
    expect(fixture.nativeElement.textContent).toContain('may be stale');
    fixture.destroy();
  });
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
    fixture.detectChanges();
    const live = [...fixture.nativeElement.querySelectorAll('button')]
      .find((button: HTMLButtonElement) => button.textContent?.includes('Start live updates')) as HTMLButtonElement;
    expect(live).toBeTruthy();
    live.click();
    await vi.advanceTimersByTimeAsync(0);
    expect(read.mock.calls.filter(([path]) => path === '')).toHaveLength(2);
    hidden.mockReturnValue(true);
    await vi.advanceTimersByTimeAsync(10000);
    expect(read.mock.calls.filter(([path]) => path === '')).toHaveLength(2);
    hidden.mockReturnValue(false);
    document.dispatchEvent(new Event('visibilitychange'));
    await vi.advanceTimersByTimeAsync(0);
    expect(read.mock.calls.filter(([path]) => path === '')).toHaveLength(3);
    fixture.destroy();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('starts paused and polls every five seconds only after being started', async () => {
    vi.useFakeTimers();
    const read = vi.fn((path: string) => path ? Promise.reject(new Error('older firmware')) : Promise.resolve(sample));
    TestBed.configureTestingModule({ providers: [{ provide: MachineDiagnostics, useValue: { read } }] });
    const fixture = TestBed.createComponent(DiagnosticsPage);
    await vi.advanceTimersByTimeAsync(0);
    fixture.detectChanges();
    expect(fixture.nativeElement.textContent).toContain('Start live updates');
    await vi.advanceTimersByTimeAsync(10000);
    expect(read.mock.calls.filter(([path]) => path === '')).toHaveLength(1);
    const live = [...fixture.nativeElement.querySelectorAll('button')]
      .find((button: HTMLButtonElement) => button.textContent?.includes('Start live updates')) as HTMLButtonElement;
    live.click();
    await vi.advanceTimersByTimeAsync(0);
    expect(read.mock.calls.filter(([path]) => path === '')).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(4999);
    expect(read.mock.calls.filter(([path]) => path === '')).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(1);
    expect(read.mock.calls.filter(([path]) => path === '')).toHaveLength(3);
    fixture.destroy();
  });
});
