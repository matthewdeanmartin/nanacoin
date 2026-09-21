// Session holds the derived state the UI reads constantly - who you can pay,
// what is for sale, whether you are Nana. Those derivations are small enough
// to get wrong quietly, so they are tested directly.

import { TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { HttpTestingController, provideHttpClientTesting } from '@angular/common/http/testing';

import { ApiBase } from './api-base';
import { Listing, User } from './models';
import { Session } from './session';

function user(partial: Partial<User>): User {
  return {
    id: 'user-1',
    username: 'someone',
    display_name: 'Someone',
    role: 'user',
    status: 'ACTIVE',
    account: 'account-1',
    created_at: 0,
    ...partial,
  };
}

function listing(partial: Partial<Listing>): Listing {
  return {
    id: 'listing-1',
    seller: 'account-2',
    seller_name: 'Bob',
    title: 'Thing',
    description: '',
    price: 5,
    status: 'ACTIVE',
    created_at: 0,
    updated_at: 0,
    ...partial,
  };
}

describe('Session', () => {
  let session: Session;

  beforeEach(() => {
    TestBed.configureTestingModule({
      providers: [
        provideHttpClient(),
        provideHttpClientTesting(),
        ApiBase,
      ],
    });
    session = TestBed.inject(Session);
  });

  it('starts signed out', () => {
    expect(session.signedIn()).toBe(false);
    expect(session.isNana()).toBe(false);
    expect(session.balance()).toBe(0);
  });

  it('reports the Nana role', () => {
    session.me.set(user({ role: 'nana' }));
    expect(session.isNana()).toBe(true);
  });

  it('reads the balance, defaulting to zero when the server withheld it', () => {
    session.me.set(user({ balance: 42 }));
    expect(session.balance()).toBe(42);

    session.me.set(user({ balance: undefined }));
    expect(session.balance()).toBe(0);
  });

  it('excludes the signed-in user from the list of people they can pay', () => {
    const me = user({ id: 'user-me', account: 'account-me' });
    session.me.set(me);
    session.household.set([me, user({ id: 'user-bob', account: 'account-bob' })]);

    expect(session.recipients().map((u) => u.id)).toEqual(['user-bob']);
  });

  it('excludes disabled members from the list of people they can pay', () => {
    const me = user({ id: 'user-me' });
    session.me.set(me);
    session.household.set([
      me,
      user({ id: 'user-bob' }),
      user({ id: 'user-old', status: 'DISABLED' }),
    ]);

    expect(session.recipients().map((u) => u.id)).toEqual(['user-bob']);
  });

  it('splits listings into what is for sale and what is closed', () => {
    session.listings.set([
      listing({ id: 'a', status: 'ACTIVE' }),
      listing({ id: 'b', status: 'SOLD' }),
      listing({ id: 'c', status: 'CANCELLED' }),
      listing({ id: 'd', status: 'ACTIVE' }),
    ]);

    expect(session.forSale().map((l) => l.id)).toEqual(['a', 'd']);
    expect(session.closed().map((l) => l.id)).toEqual(['b', 'c']);
  });

  it('resolves an account id to a display name, falling back to the id', () => {
    session.household.set([user({ account: 'account-bob', display_name: 'Bob' })]);

    expect(session.nameFor('account-bob')).toBe('Bob');
    // The issuance account is never a household member, so the id showing
    // through is the correct outcome rather than a blank.
    expect(session.nameFor('account:system-issuance')).toBe('account:system-issuance');
  });

  it('clears household state on logout', async () => {
    session.me.set(user({}));
    session.household.set([user({})]);
    session.listings.set([listing({})]);

    // logout() posts to the server first and resets state afterwards, so the
    // reset is only observable once that request has settled. Failing the
    // request is the interesting case: the user is logged out of this browser
    // regardless of what the server said.
    const http = TestBed.inject(HttpTestingController);
    const logout = session.logout();
    http.expectOne('/api/v1/auth/logout').flush('', { status: 500, statusText: 'nope' });
    await logout;

    expect(session.me()).toBeNull();
    expect(session.household()).toEqual([]);
    expect(session.listings()).toEqual([]);
  });
});
