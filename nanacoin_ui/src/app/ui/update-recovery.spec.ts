import { TestBed } from '@angular/core/testing';
import { Log } from '../api/log';
import { RELOAD_PAGE, UPDATE_RECOVERY_KEY, UpdateRecovery, isChunkLoadError } from './update-recovery';

describe('deployment recovery', () => {
  const reload = vi.fn();
  let originalUrl: string;
  beforeEach(() => {
    originalUrl = location.href;
    sessionStorage.removeItem(UPDATE_RECOVERY_KEY);
    vi.useFakeTimers();
    reload.mockReset();
    TestBed.configureTestingModule({providers: [
      {provide: RELOAD_PAGE, useValue: reload},
      {provide: Log, useValue: {error: vi.fn(), warn: vi.fn()}},
    ]});
  });
  afterEach(() => {
    TestBed.resetTestingModule();
    vi.useRealTimers(); vi.restoreAllMocks();
    sessionStorage.removeItem(UPDATE_RECOVERY_KEY);
    history.replaceState(null, '', originalUrl);
  });
  it('recognizes browser and wrapped chunk failures, but not ordinary errors', () => {
    for (const error of [new TypeError('error loading dynamically imported module: chunk.js'),
      {error: {reason: new Error('Failed to fetch dynamically imported module: chunk.js')}},
      {name: 'ChunkLoadError', message: 'Loading chunk 5 failed'},
      'Loading module was blocked because of a disallowed MIME type']) expect(isChunkLoadError(error)).toBe(true);
    expect(isChunkLoadError(new Error('undefined is not a function'))).toBe(false);
    const cycle: {error?: unknown} = {}; cycle.error = cycle;
    expect(isChunkLoadError(cycle)).toBe(false);
  });
  it('reloads once and retains the requested route, API query and hosting prefix', () => {
    history.replaceState(null, '', '/nanacoin/?api=http%3A%2F%2Fbank.local%2Fapi%2Fv1#/market');
    const recovery = TestBed.inject(UpdateRecovery);
    expect(recovery.recover(new Error('ordinary error'))).toBe(false);
    recovery.recover(new TypeError('Failed to fetch dynamically imported module: missing.js'));
    recovery.recover(new TypeError('Failed to fetch dynamically imported module: missing.js'), '/specification');
    expect(recovery.updating()).toBe(true);
    vi.advanceTimersByTime(400);
    expect(reload).toHaveBeenCalledOnce();
    expect(location.pathname).toBe('/nanacoin/');
    expect(new URLSearchParams(location.search).get('api')).toBe('http://bank.local/api/v1');
    expect(location.hash).toBe('#/specification');
  });
  it('stops after a failed reload and allows a guarded manual retry', () => {
    sessionStorage.setItem(UPDATE_RECOVERY_KEY, JSON.stringify({attemptedAt: Date.now()}));
    const recovery = TestBed.inject(UpdateRecovery);
    recovery.recover(new Error('importing a module script failed'), '/recipes');
    vi.advanceTimersByTime(31_000);
    expect(recovery.failed()).toBe(true);
    expect(reload).not.toHaveBeenCalled();
    expect(sessionStorage.getItem(UPDATE_RECOVERY_KEY)).not.toBeNull();
    recovery.retry();
    expect(reload).toHaveBeenCalledOnce();
    expect(recovery.updating()).toBe(true);
    expect(location.hash).toBe('#/recipes');
    expect(JSON.parse(sessionStorage.getItem(UPDATE_RECOVERY_KEY)!).attemptedAt).toBe(Date.now());
  });
  it('offers a manual retry when offline or storage cannot save the loop guard', () => {
    const recovery = TestBed.inject(UpdateRecovery);
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {throw new Error('blocked');});
    recovery.recover('Failed to fetch dynamically imported module');
    expect(recovery.failed()).toBe(true);
    vi.advanceTimersByTime(500);
    expect(reload).not.toHaveBeenCalled();
  });
  it('does not automatically reload an offline tab', () => {
    vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(false);
    const recovery = TestBed.inject(UpdateRecovery);
    recovery.recover('Failed to fetch dynamically imported module');
    expect(recovery.failed()).toBe(true);
    vi.advanceTimersByTime(500);
    expect(reload).not.toHaveBeenCalled();
  });
  it('clears a previous guard only after the app runs without a chunk failure', () => {
    sessionStorage.setItem(UPDATE_RECOVERY_KEY, JSON.stringify({attemptedAt: Date.now()}));
    TestBed.inject(UpdateRecovery);
    vi.advanceTimersByTime(29_000);
    expect(sessionStorage.getItem(UPDATE_RECOVERY_KEY)).not.toBeNull();
    vi.advanceTimersByTime(1_000);
    expect(sessionStorage.getItem(UPDATE_RECOVERY_KEY)).toBeNull();
  });
});
