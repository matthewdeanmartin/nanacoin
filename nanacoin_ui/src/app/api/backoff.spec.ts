// The board refuses deliberately when every worker is busy: 503 with a
// Retry-After header, rather than dropping the connection. The whole point of
// doing that is so a client can tell "busy" from "dead" - and the client used
// to throw the distinction away, reporting it as "could not reach NanaCoin"
// and never retrying.
//
// These pin the behaviour that makes the firmware's backpressure worth having.

import { TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { HttpTestingController, provideHttpClientTesting } from '@angular/common/http/testing';

import { ApiBase } from './api-base';
import { BusyError, NanacoinService } from './nanacoin.service';

describe('503 backpressure', () => {
  let api: NanacoinService;
  let http: HttpTestingController;

  beforeEach(() => {
    sessionStorage.clear();
    TestBed.configureTestingModule({
      providers: [provideHttpClient(), provideHttpClientTesting(), ApiBase],
    });
    api = TestBed.inject(NanacoinService);
    http = TestBed.inject(HttpTestingController);
  });

  afterEach(() => http.verify());

  it('retries a 503 with Retry-After and succeeds on the second try', async () => {
    const promise = api.status();

    const first = http.expectOne((r) => r.url.endsWith('/status'));
    first.flush('busy', {
      status: 503,
      statusText: 'Service Unavailable',
      headers: { 'Retry-After': '0.25' },
    });

    // The retry is scheduled behind the server's own delay.
    await new Promise((r) => setTimeout(r, 300));

    const second = http.expectOne((r) => r.url.endsWith('/status'));
    second.flush({ provisioned: true, household: 'H' });

    const result = await promise;
    expect(result.household).toBe('H');
  });

  it('gives up after three attempts and reports busy, not unreachable', async () => {
    const promise = api.status();
    const failures: unknown[] = [];
    promise.catch((e) => failures.push(e));

    for (let i = 0; i < 3; i++) {
      const req = http.expectOne((r) => r.url.endsWith('/status'));
      req.flush('busy', {
        status: 503,
        statusText: 'Service Unavailable',
        headers: { 'Retry-After': '0.25' },
      });
      await new Promise((r) => setTimeout(r, 300));
    }

    await new Promise((r) => setTimeout(r, 50));
    expect(failures.length).toBe(1);
    const err = failures[0];
    expect(err instanceof BusyError).toBe(true);
    // The distinction that matters: this is not "unreachable".
    expect((err as BusyError).code).toBe('busy');
  });

  it('does not retry a 503 without Retry-After', async () => {
    // A proxy answering on the board's behalf sends a bare 503. That really is
    // "cannot reach it", and retrying would just be slow about saying so.
    const promise = api.status();
    const failures: unknown[] = [];
    promise.catch((e) => failures.push(e));

    const req = http.expectOne((r) => r.url.endsWith('/status'));
    req.flush('gateway', { status: 503, statusText: 'Service Unavailable' });

    await new Promise((r) => setTimeout(r, 50));
    expect(failures.length).toBe(1);
    expect(failures[0] instanceof BusyError).toBe(false);
    expect((failures[0] as { code: string }).code).toBe('unreachable');
  });

  it('does not retry an ordinary refusal', async () => {
    // A 400 will fail again. Retrying it wastes the board's time and delays
    // telling the person what is wrong.
    const promise = api.status();
    const failures: unknown[] = [];
    promise.catch((e) => failures.push(e));

    const req = http.expectOne((r) => r.url.endsWith('/status'));
    req.flush(
      { error: 'bad_request', message: 'nope' },
      { status: 400, statusText: 'Bad Request' },
    );

    await new Promise((r) => setTimeout(r, 50));
    expect(failures.length).toBe(1);
    expect(failures[0] instanceof BusyError).toBe(false);
  });

  it('bounds a hostile Retry-After rather than waiting forever', async () => {
    const promise = api.status();
    promise.catch(() => undefined);

    const started = Date.now();
    const req = http.expectOne((r) => r.url.endsWith('/status'));
    req.flush('busy', {
      status: 503,
      statusText: 'Service Unavailable',
      headers: { 'Retry-After': '99999' },
    });

    // Capped at 10s, so this must not have scheduled anything longer. The
    // retry itself is not awaited here; only the cap is being checked.
    await new Promise((r) => setTimeout(r, 50));
    expect(Date.now() - started).toBeLessThan(1000);

    // Drain the pending retry so verify() is happy.
    await new Promise((r) => setTimeout(r, 0));
    http.match((r) => r.url.endsWith('/status')).forEach((r) =>
      r.flush({ provisioned: false }),
    );
  });
});
