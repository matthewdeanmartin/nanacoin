// Accounts holds bearer tokens and decides which one every request uses, so
// its edge cases are the ones that would show someone else's money under your
// name. They are tested directly.

import { TestBed } from '@angular/core/testing';

import { Accounts, StoredAccount } from './accounts';

function account(partial: Partial<StoredAccount> = {}): StoredAccount {
  return {
    userId: 'user-1',
    username: 'alice',
    displayName: 'Alice',
    role: 'user',
    token: 'token-1',
    ...partial,
  };
}

describe('Accounts', () => {
  let accounts: Accounts;

  beforeEach(() => {
    sessionStorage.clear();
    TestBed.configureTestingModule({ providers: [Accounts] });
    accounts = TestBed.inject(Accounts);
  });

  it('starts empty', () => {
    expect(accounts.all()).toEqual([]);
    expect(accounts.active()).toBeNull();
    expect(accounts.token()).toBeNull();
  });

  it('makes a newly added account active', () => {
    accounts.add(account());
    expect(accounts.active()?.userId).toBe('user-1');
    expect(accounts.token()).toBe('token-1');
  });

  it('holds several accounts at once', () => {
    accounts.add(account());
    accounts.add(account({ userId: 'user-2', username: 'nana', role: 'nana', token: 'token-2' }));

    expect(accounts.all().length).toBe(2);
    expect(accounts.hasSeveral()).toBe(true);
    // The most recent login is the active one.
    expect(accounts.token()).toBe('token-2');
  });

  it('replaces the token when the same user logs in again', () => {
    accounts.add(account());
    accounts.add(account({ token: 'token-fresh' }));

    expect(accounts.all().length).toBe(1);
    expect(accounts.token()).toBe('token-fresh');
  });

  it('switches the active token without touching the others', () => {
    accounts.add(account());
    accounts.add(account({ userId: 'user-2', token: 'token-2' }));

    accounts.activate('user-1');
    expect(accounts.token()).toBe('token-1');
    expect(accounts.all().length).toBe(2);
  });

  it('ignores a switch to an account it does not hold', () => {
    accounts.add(account());
    accounts.activate('nobody');
    expect(accounts.token()).toBe('token-1');
  });

  it('falls back to a remaining account when the active one is removed', () => {
    accounts.add(account());
    accounts.add(account({ userId: 'user-2', token: 'token-2' }));

    accounts.remove('user-2');
    expect(accounts.active()?.userId).toBe('user-1');
    expect(accounts.token()).toBe('token-1');
  });

  it('signs out entirely when the last account is removed', () => {
    accounts.add(account());
    accounts.remove('user-1');

    expect(accounts.all()).toEqual([]);
    expect(accounts.token()).toBeNull();
  });

  it('removing a background account leaves the active one alone', () => {
    accounts.add(account());
    accounts.add(account({ userId: 'user-2', token: 'token-2' }));

    accounts.remove('user-1');
    expect(accounts.token()).toBe('token-2');
  });

  it('invalidateActive drops only the account that made the request', () => {
    accounts.add(account());
    accounts.add(account({ userId: 'user-2', token: 'token-2' }));

    // A 401 on user-2's request must not sign user-1 out too.
    accounts.invalidateActive();
    expect(accounts.all().length).toBe(1);
    expect(accounts.active()?.userId).toBe('user-1');
  });

  it('restores accounts from sessionStorage', () => {
    accounts.add(account());
    accounts.add(account({ userId: 'user-2', token: 'token-2' }));
    accounts.activate('user-1');

    // A fresh instance reads what the first one persisted, which is what a
    // reload in the same tab does.
    const reloaded = new Accounts();
    expect(reloaded.all().length).toBe(2);
    expect(reloaded.active()?.userId).toBe('user-1');
  });

  it('ignores a stored blob of the wrong shape rather than trusting it', () => {
    sessionStorage.setItem('nanacoin.accounts', JSON.stringify({ v: 99, accounts: 'nope' }));
    const fresh = new Accounts();
    expect(fresh.all()).toEqual([]);
    expect(fresh.token()).toBeNull();
  });

  it('drops stored entries that are missing a token', () => {
    sessionStorage.setItem(
      'nanacoin.accounts',
      JSON.stringify({
        v: 1,
        accounts: [account(), { userId: 'broken' }],
        activeUserId: 'user-1',
      }),
    );
    const fresh = new Accounts();
    expect(fresh.all().length).toBe(1);
    expect(fresh.token()).toBe('token-1');
  });

  it('picks a live account when the stored active id is gone', () => {
    sessionStorage.setItem(
      'nanacoin.accounts',
      JSON.stringify({ v: 1, accounts: [account()], activeUserId: 'vanished' }),
    );
    const fresh = new Accounts();
    expect(fresh.active()?.userId).toBe('user-1');
  });

  it('clear forgets everything', () => {
    accounts.add(account());
    accounts.add(account({ userId: 'user-2', token: 'token-2' }));
    accounts.clear();

    expect(accounts.all()).toEqual([]);
    expect(accounts.active()).toBeNull();
  });
});
