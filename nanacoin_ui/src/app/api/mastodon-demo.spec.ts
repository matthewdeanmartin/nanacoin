import { signal } from '@angular/core';
import { TestBed } from '@angular/core/testing';
import { Mastodon } from './mastodon';
import { NanacoinService } from './nanacoin.service';
import { Session } from './session';
import { Toasts } from '../ui/toasts';
import { IS_DEMO } from '../demo/demo';

describe.skipIf(!IS_DEMO)('Mastodon connections in the demo', () => {
  afterEach(() => { vi.restoreAllMocks(); TestBed.resetTestingModule(); sessionStorage.clear(); });
  it('only shows the popup, even without a server or a signed-in account', async () => {
    const ok = vi.fn();
    const fetch = vi.spyOn(globalThis, 'fetch');
    TestBed.configureTestingModule({providers: [
      {provide: Session, useValue: {me: signal(null)}},
      {provide: NanacoinService, useValue: {}},
      {provide: Toasts, useValue: {ok}},
    ]});
    const mastodon = TestBed.inject(Mastodon);
    await mastodon.connect('');
    await mastodon.connect('mastodon.social');
    expect(ok).toHaveBeenCalledTimes(2);
    expect(ok).toHaveBeenCalledWith('mastodon connection disabled in demo mode');
    expect(fetch).not.toHaveBeenCalled();
    expect(sessionStorage.getItem('nanacoin:mastodon:oauth')).toBeNull();
    expect(mastodon.connected()).toBe(false);
    // An old, partially completed demo flow must not exchange a token either.
    sessionStorage.setItem('nanacoin:mastodon:oauth', '{}');
    await expect(mastodon.finish('code', 'state')).rejects.toThrow('mastodon connection disabled in demo mode');
    expect(fetch).not.toHaveBeenCalled();
    expect(sessionStorage.getItem('nanacoin:mastodon:oauth')).toBeNull();
  });
});
