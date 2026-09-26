import { TestBed } from '@angular/core/testing';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { IncidentHistory, IncidentHistoryPanel } from './incident-history';

const history: IncidentHistory = {
  boot_id: 42, retained_bytes: 3000, volatile: true, dropped: 2, overwritten: 3,
  high_water: [2, 8, 4], last_handshake_ms: 900, max_handshake_ms: 1200,
  counters: [{ kind: 'worker_recovered', count: 1 }],
  events: [{ first_ms: 1000, last_ms: 4000, count: 1, duration_ms: 3000, code: 1, kind: 'worker_recovered' }],
  samples: [{ at_ms: 5000, free_heap: 10240, largest_block: 4096, rssi: null,
    tls_gap_ms: 0, http_gap_ms: 3000, pending_tls: 2, tls_clients: 8, http_clients: 1 }],
};
afterEach(() => { TestBed.resetTestingModule(); vi.restoreAllMocks(); vi.useRealTimers(); });
describe('incident history', () => {
  it('shows retained recovery evidence, loss counts and reboot limitation', () => {
    const fixture = TestBed.createComponent(IncidentHistoryPanel);
    fixture.componentRef.setInput('history', history);
    fixture.detectChanges();
    const text = fixture.nativeElement.textContent;
    expect(text).toContain('worker recovered');
    expect(text).toContain('HTTP worker');
    expect(text).toContain('3000 ms');
    expect(text).toContain('3 events overwritten');
    expect(text).toContain('2 event/sample updates missed');
    expect(text).toContain('history clears on restart');
    expect(text).toContain('disconnected / unavailable');
  });
  it('exports the retained history locally', () => {
    vi.useFakeTimers();
    const create = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:test');
    const revoke = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    const fixture = TestBed.createComponent(IncidentHistoryPanel);
    fixture.componentRef.setInput('history', history);
    fixture.detectChanges();
    fixture.nativeElement.querySelector('button').click();
    expect(create).toHaveBeenCalledWith(expect.any(Blob));
    expect(click).toHaveBeenCalledOnce();
    vi.advanceTimersByTime(1000);
    expect(revoke).toHaveBeenCalledWith('blob:test');
  });
});
