import { TestBed } from '@angular/core/testing';
import { Toasts } from './toasts';
import { Log } from '../api/log';

describe('error toast diagnostics', () => {
  afterEach(() => vi.restoreAllMocks());
  it('records direct and partial-success errors with their cause even when informational console logging is off', () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const log = TestBed.inject(Log);
    log.toConsole.set(false);
    const toasts = TestBed.inject(Toasts);
    const cause = new TypeError('randomUUID is unavailable');
    toasts.error('Board message saved, Mastodon copy failed', cause);
    expect(log.asText()).toContain('randomUUID is unavailable');
    expect(log.recent()[0].detail?.['stack']).toContain('TypeError');
    expect(consoleError).toHaveBeenCalled();
    toasts.dismiss(toasts.items()[0].id);
    expect(log.asText()).toContain('Mastodon copy failed');
  });
});
