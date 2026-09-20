// The log is meant to be copied and pasted into a chat window. That is the
// whole point of it, and it is exactly why what goes in has to be checked:
// a bearer token in a pasted log is a bearer token handed to whoever reads it.

import { TestBed } from '@angular/core/testing';

import { Log, redact } from './log';

describe('redact', () => {
  it('hides passwords wherever they appear', () => {
    const out = redact({ username: 'nana', password: 'hunter2' }) as Record<string, unknown>;
    expect(out['username']).toBe('nana');
    expect(out['password']).toBe('[redacted]');
  });

  it('hides tokens, verifiers and authorization headers', () => {
    const out = redact({
      access_token: 'abc',
      refresh_token: 'def',
      code_verifier: 'ghi',
      Authorization: 'Bearer xyz',
    }) as Record<string, unknown>;

    expect(Object.values(out).every((v) => v === '[redacted]')).toBe(true);
  });

  it('matches on the key however it is spelled', () => {
    const out = redact({
      PASSWORD: 'a',
      userPassword: 'b',
      'x-auth-token': 'c',
    }) as Record<string, unknown>;

    expect(out['PASSWORD']).toBe('[redacted]');
    expect(out['userPassword']).toBe('[redacted]');
    expect(out['x-auth-token']).toBe('[redacted]');
  });

  it('reaches secrets nested inside a request body', () => {
    // The shape that actually occurs: the HTTP tracer logs { body }, and the
    // body of a login is what carries the password.
    const out = redact({
      body: { username: 'nana', password: 'hunter2' },
    }) as Record<string, Record<string, unknown>>;

    expect(out['body']['username']).toBe('nana');
    expect(out['body']['password']).toBe('[redacted]');
  });

  it('records that a secret was present rather than dropping the key', () => {
    // "Did the request carry a token at all" is often the question being
    // asked, so the key survives even though the value does not.
    const out = redact({ token: 'abc' }) as Record<string, unknown>;
    expect('token' in out).toBe(true);
  });

  it('leaves null and undefined alone', () => {
    const out = redact({ token: null, password: undefined }) as Record<string, unknown>;
    expect(out['token']).toBeNull();
    expect(out['password']).toBeUndefined();
  });

  it('passes ordinary values through untouched', () => {
    const out = redact({ status: 404, path: '/api/v1/offers', ok: false }) as Record<
      string,
      unknown
    >;
    expect(out).toEqual({ status: 404, path: '/api/v1/offers', ok: false });
  });

  it('stops rather than recursing forever on a cyclic object', () => {
    // A logger that hangs turns a diagnostic into an outage.
    const cyclic: Record<string, unknown> = { name: 'loop' };
    cyclic['self'] = cyclic;
    expect(() => redact(cyclic)).not.toThrow();
  });

  it('bounds a long array', () => {
    const out = redact({ items: Array.from({ length: 500 }, (_, i) => i) }) as Record<
      string,
      unknown[]
    >;
    expect(out['items'].length).toBeLessThanOrEqual(20);
  });
});

describe('Log', () => {
  let log: Log;

  beforeEach(() => {
    TestBed.configureTestingModule({ providers: [Log] });
    log = TestBed.inject(Log);
    log.toConsole.set(false);
    log.clear();
  });

  it('records what it was told, newest first', () => {
    log.info('http', 'first');
    log.warn('auth', 'second');

    const recent = log.recent();
    expect(recent[0].message).toBe('second');
    expect(recent[0].level).toBe('warn');
    expect(recent[1].message).toBe('first');
  });

  it('redacts on the way in, so nothing secret is ever held', () => {
    log.info('auth', 'logging in', { password: 'hunter2' });
    // Not merely hidden at render time: the entry itself must be clean, or a
    // copy button is a leak.
    expect(JSON.stringify(log.all())).not.toContain('hunter2');
  });

  it('keeps a bounded number of entries', () => {
    for (let i = 0; i < 500; i++) log.info('http', `request ${i}`);
    expect(log.all().length).toBeLessThanOrEqual(200);
    // The newest survive; the oldest are what gets dropped.
    expect(log.recent()[0].message).toBe('request 499');
  });

  it('formats itself as pasteable text', () => {
    log.error('http', 'GET /api/v1/status failed', { status: 404 });
    const text = log.asText();
    expect(text).toContain('error');
    expect(text).toContain('GET /api/v1/status failed');
    expect(text).toContain('404');
  });

  it('bumps a revision so views can react', () => {
    const before = log.revision();
    log.info('boot', 'started');
    expect(log.revision()).toBeGreaterThan(before);
  });
});
