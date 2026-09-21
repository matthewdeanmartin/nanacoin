import { TestBed } from '@angular/core/testing';
import { provideHttpClient, withInterceptors } from '@angular/common/http';
import { HttpTestingController, provideHttpClientTesting } from '@angular/common/http/testing';
import { ApiBase } from './api-base';
import { Accounts } from './accounts';
import { NanacoinService, newIdempotencyKey, journalGenerationInterceptor } from './nanacoin.service';

describe('journal maintenance client', () => {
  let api: NanacoinService;
  let http: HttpTestingController;
  beforeEach(() => {
    TestBed.configureTestingModule({providers: [provideHttpClient(withInterceptors([journalGenerationInterceptor])), provideHttpClientTesting()]});
    TestBed.inject(ApiBase).set('http://storage-test.local');
    api = TestBed.inject(NanacoinService); http = TestBed.inject(HttpTestingController);
  });
  afterEach(() => { http.verify(); TestBed.resetTestingModule(); });
  it('enables HTTPS with explicit confirmation and clears local sessions only on success', async () => {
    const clear = vi.spyOn(TestBed.inject(Accounts), 'clear');
    const pending = api.requireHttps();
    const request = http.expectOne((r) => r.url.endsWith('/admin/transport'));
    expect(request.request.body).toEqual({ confirmation: 'REQUIRE HTTPS' });
    expect(clear).not.toHaveBeenCalled();
    request.flush(true); await pending;
    expect(clear).toHaveBeenCalledOnce();
  });

  it('captures generation in new keys and keeps already-created keys unchanged', async () => {
    const status = api.status();
    http.expectOne((r) => r.url.endsWith('/status')).flush({journal_generation: 10});
    await status;
    const original = newIdempotencyKey(); expect(original.startsWith('g10:')).toBe(true);
    const write = api.issue('account-1', 1, '', original);
    const request = http.expectOne((r) => r.url.endsWith('/admin/issue'));
    expect(request.request.headers.get('Idempotency-Key')).toBe(original);
    request.flush({}, { headers: { 'X-Nanacoin-Generation': '11' } }); await write;
    expect(newIdempotencyKey().startsWith('g11:')).toBe(true);
    expect(original.startsWith('g10:')).toBe(true);
  });

  it('sends reset confirmation and concurrency checks and only clears accounts after success', async () => {
    const clear = vi.spyOn(TestBed.inject(Accounts), 'clear');
    const pending = api.resetEconomy({ generation: 5, sequence: 42, journal_records: 7, checkpoint_after: 2048, checkpoint_supported: true }, 'RESET ECONOMY');
    const request = http.expectOne((r) => r.url.endsWith('/admin/reset'));
    expect(request.request.body).toEqual({ expected_generation: 5, expected_sequence: 42, confirmation: 'RESET ECONOMY' });
    expect(clear).not.toHaveBeenCalled(); request.flush({ generation: 6 }); await pending;
    expect(clear).toHaveBeenCalledOnce();
  });

  it('does not clear accounts or retry a failed reset', async () => {
    const clear = vi.spyOn(TestBed.inject(Accounts), 'clear');
    const pending = api.resetEconomy({ generation: 5, sequence: 42, journal_records: 7, checkpoint_after: 2048, checkpoint_supported: true }, 'RESET ECONOMY');
    const outcome = expect(pending).rejects.toMatchObject({status: 409});
    http.expectOne((r) => r.url.endsWith('/admin/reset')).flush({error:'conflict'}, {status:409, statusText:'Conflict'});
    await outcome; expect(clear).not.toHaveBeenCalled();
  });
});
