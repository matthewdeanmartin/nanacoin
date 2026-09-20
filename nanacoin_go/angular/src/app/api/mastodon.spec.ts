import { signal } from '@angular/core';
import { TestBed } from '@angular/core/testing';

import { User } from './models';
import { Mastodon } from './mastodon';
import { NanacoinService } from './nanacoin.service';
import { Session } from './session';

describe('Mastodon', () => {
  const me = signal<User | null>({
    id: 'user-1', username: 'alice', display_name: 'Alice', role: 'user',
    status: 'ACTIVE', account: 'account-1', created_at: 1,
  });
  const refresh = vi.fn(() => Promise.resolve());
  const setUserMastodonId = vi.fn(() => Promise.resolve({} as User));
  let mastodon: Mastodon;

  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    vi.restoreAllMocks();
    TestBed.configureTestingModule({ providers: [
      Mastodon,
      { provide: Session, useValue: { me, refresh } },
      { provide: NanacoinService, useValue: { setUserMastodonId } },
    ] });
    mastodon = TestBed.inject(Mastodon);
  });

  it('can only send a direct status to a member with a registered ID', async () => {
    localStorage.setItem('nanacoin:mastodon:credentials:user-1', JSON.stringify({
      server: 'https://social.example', clientId: 'app', clientSecret: 'public',
      accessToken: 'browser-token', acct: '@alice@social.example',
    }));
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response('{}', { status: 200, headers: { 'Content-Type': 'application/json' } }),
    );
    const bob: User = {
      id: 'user-2', username: 'bob', display_name: 'Bob', role: 'user',
      status: 'ACTIVE', account: 'account-2', created_at: 1,
      mastodon_id: '@bob@elsewhere.example',
    };

    await mastodon.sendDirect(bob, 'Payment received');

    expect(fetchMock).toHaveBeenCalledOnce();
    const [url, request] = fetchMock.mock.calls[0];
    expect(url).toBe('https://social.example/api/v1/statuses');
    expect(JSON.parse(String(request?.body))).toEqual({
      status: '@bob@elsewhere.example Payment received', visibility: 'direct',
    });
    await expect(mastodon.sendDirect({ ...bob, mastodon_id: undefined }, 'hello'))
      .rejects.toThrow('no registered Mastodon ID');
  });
});
