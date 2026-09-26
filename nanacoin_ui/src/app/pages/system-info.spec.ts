import { TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { HttpTestingController, provideHttpClientTesting } from '@angular/common/http/testing';
import { afterEach, describe, expect, it } from 'vitest';
import { ConfigurationPage } from './configuration';
import { DatabasePage } from './database';
import { ApiBase } from '../api/api-base';

afterEach(() => { TestBed.inject(HttpTestingController).verify(); TestBed.resetTestingModule(); });
function setup() {
  TestBed.configureTestingModule({ providers: [provideHttpClient(), provideHttpClientTesting(),
    { provide: ApiBase, useValue: { current: () => '/api/v1' } }] });
  return TestBed.inject(HttpTestingController);
}
describe('public system information', () => {
  it('shows exact decimal scale and grants without a session or write controls', () => {
    const http = setup();
    const fixture = TestBed.createComponent(ConfigurationPage);
    const request = http.expectOne('/api/v1/configuration');
    expect(request.request.method).toBe('GET');
    expect(request.request.headers.has('Authorization')).toBe(false);
    request.flush({ household_name: 'Our house', currency: 'NC', decimals: 4, minor_units_per_coin: 10000,
      money_epoch: 2, initial_grant: 12500, offer_settles_after: 172800, usd_decimals: 2,
      maximum_amount_minor: 1000000000000000, lending_enabled: true, rate_basis_points_per_percent: 100,
      savings_lotto_holding_seconds: 2592000, smallest_unit: '', lending_policy: '', terms_policy: '' });
    fixture.detectChanges();
    expect(fixture.nativeElement.textContent).toContain('Our house');
    expect(fixture.nativeElement.textContent).toContain('0.0001 NC');
    expect(fixture.nativeElement.textContent).toContain('1.25 NC');
    expect(fixture.nativeElement.querySelectorAll('input, select, textarea')).toHaveLength(0);
    fixture.destroy();
  });
  it('benchmarks only on demand, never overlaps, and sends only GET requests', () => {
    const http = setup();
    const fixture = TestBed.createComponent(DatabasePage);
    http.expectOne('/api/v1/diag').flush({}, { status: 404, statusText: 'no hardware' });
    http.expectOne('/api/v1/diag/database').flush({ collections: [], invariants_ok: true,
      checkpoint_supported: false, model_reserved_bytes: 0, journal_records: 0, journal_free_records: 4096,
      journal_record_capacity: 4096, journal_logical_bytes: 0, journal_frame_bytes: 1024,
      checkpoint_after: 2048, checkpoint_rows: 0, checkpoint_row_capacity: 5000, checkpoint_row_max_bytes: 4096,
      lifetime_transactions: 0, generation: 0, sequence: 0 });
    http.expectNone('/api/v1/diag/database/benchmark');
    fixture.detectChanges();
    const button = [...fixture.nativeElement.querySelectorAll('button')].find((b: HTMLButtonElement) => b.textContent?.includes('Run query')) as HTMLButtonElement;
    button.click(); button.click();
    const request = http.expectOne('/api/v1/diag/database/benchmark');
    expect(request.request.method).toBe('GET');
    expect(request.request.body).toBeNull();
    request.flush({ generation: 0, sequence: 0, read_only: true, elapsed_us: 40, queries: [], note: 'No writes.' });
    fixture.detectChanges();
    expect(fixture.nativeElement.textContent).toContain('No writes.');
    fixture.destroy();
  });
});
