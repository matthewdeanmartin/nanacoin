import { ENERGY_CASES } from './about';
import { browserHealth } from './browser-health';
import { TestBed } from '@angular/core/testing';
import { Notebook } from '../ui/notebook';
import { redact } from '../api/log';

describe('public showcase', () => {
 afterEach(() => { vi.restoreAllMocks(); TestBed.resetTestingModule(); });
 it('calculates annual board energy without confusing W, kWh and TWh', () => {
   expect(ENERGY_CASES.map(c => c.kwh)).toEqual([4.38, 8.76, 21.9]);
 });
 it('reads local browser data without fetching board diagnostics', () => {
   const fetcher = vi.spyOn(globalThis, 'fetch');
   const data = browserHealth();
   expect(data.uptime).toBeGreaterThanOrEqual(0);
   expect(data.heap).not.toContain('NaN');
   expect(fetcher).not.toHaveBeenCalled();
 });
 it('has independently selectable notebook and accessible plain-text modes', () => {
   const fixture = TestBed.createComponent(Notebook); fixture.detectChanges();
   const controls = fixture.nativeElement.querySelectorAll('input');
   expect(fixture.nativeElement.querySelector('.notebook')).toBeNull();
   controls[0].click(); fixture.detectChanges();
   expect(fixture.nativeElement.querySelector('.notebook')).not.toBeNull();
   controls[1].click(); fixture.detectChanges();
   expect(fixture.nativeElement.querySelector('.cursive-ledger')).not.toBeNull();
 });
 it('redacts voucher secrets from request logs', () => {
   expect(JSON.stringify(redact({ body: { token: 'DEMO-NN-secret' } }))).not.toContain('DEMO-NN-secret');
 });
});
