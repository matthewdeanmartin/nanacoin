// Who is logged in, what the household looks like, and the one place the rest
// of the app goes to refresh it.
//
// Signals rather than a store library: this is four pieces of state and a
// reload function.

import { Injectable, computed, inject, signal } from '@angular/core';

import { Accounts } from './accounts';
import { Log } from './log';
import { ApiError, NanacoinService } from './nanacoin.service';
import { Listing, Status, User } from './models';

@Injectable({ providedIn: 'root' })
export class Session {
  private readonly api = inject(NanacoinService);
  private readonly accounts = inject(Accounts);
  private readonly log = inject(Log);

  /** Every account this browser is signed into, for the switcher. */
  readonly signedInAccounts = this.accounts.all;

  /** True once a switcher is worth showing. */
  readonly hasSeveralAccounts = this.accounts.hasSeveral;

  /** The signed-in user, or null. */
  readonly me = signal<User | null>(null);

  /** Everyone in the household. Ordinary users see names but not balances. */
  readonly household = signal<User[]>([]);

  /** The server's own summary, also readable before logging in. */
  readonly status = signal<Status | null>(null);

  readonly listings = signal<Listing[]>([]);

  /** Set while a refresh is in flight, so views can show it without their own flag. */
  readonly loading = signal(false);

  readonly isNana = computed(() => this.me()?.role === 'nana');

  /**
   * Whether the server has server logs to show.
   *
   * Undefined means a server built before the capability was reported, which
   * always had logs - so the tab stays for it rather than vanishing on an
   * upgrade path nobody asked about.
   */
  readonly logsAvailable = computed(() => this.status()?.logs_enabled !== false);

  /** Whether the server serves /diag. Same defaulting as logsAvailable. */
  readonly diagAvailable = computed(() => this.status()?.diag_enabled !== false);
  readonly balance = computed(() => this.me()?.balance ?? 0);
  readonly signedIn = computed(() => this.me() !== null);

  /** The household minus the signed-in user: everyone they could pay. */
  readonly recipients = computed(() => {
    const self = this.me();
    return this.household().filter((u) => u.status === 'ACTIVE' && u.id !== self?.id);
  });

  /** Active listings, which is what the market screen shows. */
  readonly forSale = computed(() => this.listings().filter((l) => l.status === 'ACTIVE'));

  /** Everything no longer for sale, kept out of the way but not hidden. */
  readonly closed = computed(() => this.listings().filter((l) => l.status !== 'ACTIVE'));

  /** Reads the public status. Safe before provisioning and before logging in. */
  async loadStatus(): Promise<Status> {
    const s = await this.api.status();
    this.status.set(s);
    return s;
  }

  /**
   * Restores a session from a stored token, if there is one that still works.
   *
   * Returns false when there is nothing to restore - including the common case
   * of a token that outlived the board's last reboot, since sessions are
   * RAM-only by design.
   */
  async restore(): Promise<boolean> {
    if (!this.api.authenticated) {
      this.log.info('session', 'nothing to restore; no stored token');
      return false;
    }
    try {
      this.me.set(await this.api.me());
      await this.refresh();
      this.log.info('session', 'restored', { as: this.me()?.username });
      return true;
    } catch {
      // Not an error worth shouting about: a token that outlived the board's
      // last reboot is the ordinary case, since sessions are RAM-only.
      this.log.info('session', 'stored token no longer works; logging in again');
      this.me.set(null);
      return false;
    }
  }

  async login(username: string, password: string): Promise<void> {
    this.me.set(await this.api.login(username, password));
    await this.refresh();
  }

  /**
   * Switches to another account already signed in on this browser.
   *
   * No network call and no password: the token was obtained at login and is
   * still the server's. Everything the previous account could see is dropped
   * and re-read, because household visibility differs by role - Nana sees
   * balances that an ordinary member does not, and showing one person's view
   * under another's name would be the worst possible bug in a ledger app.
   *
   * A token the server has since forgotten surfaces as a 401 on the refresh,
   * which drops that account and leaves the caller to send the user back to
   * the login screen.
   */
  async switchTo(userId: string): Promise<void> {
    if (this.accounts.active()?.userId === userId) {
      this.log.debug('session', 'switch to the account already active; nothing to do');
      return;
    }
    this.log.info('session', 'switching account', {
      from: this.accounts.active()?.username,
      to: this.accounts.all().find((a) => a.userId === userId)?.username,
    });

    const previous = this.accounts.active();
    this.accounts.activate(userId);
    this.me.set(null);
    this.household.set([]);
    this.listings.set([]);

    try {
      this.me.set(await this.api.me());
      await this.refresh();
    } catch (e) {
      // The target's token was dead, and its own 401 has already dropped it.
      // Go back to the account that was working rather than leaving the app
      // signed into nothing while other live sessions are still held.
      this.log.warn('session', 'the account switched to was not accepted', {
        fallingBackTo: previous?.username,
      });
      if (previous && this.accounts.all().some((a) => a.userId === previous.userId)) {
        this.accounts.activate(previous.userId);
        try {
          this.me.set(await this.api.me());
          await this.refresh();
        } catch {
          // Both are gone - a board reboot takes every session with it.
          this.me.set(null);
        }
      }
      throw e;
    }
  }

  /** Ends every session this browser holds. */
  async logoutAll(): Promise<void> {
    try {
      await this.api.logoutAll();
    } finally {
      this.me.set(null);
      this.household.set([]);
      this.listings.set([]);
    }
  }

  /**
   * Logs out locally whatever the server says.
   *
   * Telling the server is a courtesy - it lets the session be revoked
   * immediately rather than at expiry - but a failed request must not leave
   * someone still logged in on a shared computer, which is the whole reason
   * they clicked the button.
   */
  async logout(): Promise<void> {
    try {
      await this.api.logout();
    } catch {
      // Already handled by clearing local state below.
    } finally {
      this.me.set(null);
      this.household.set([]);
      this.listings.set([]);
    }

    // Logging out of one account while others remain lands on whichever the
    // store fell back to, rather than at the login screen. That is the point
    // of holding several: leaving Nana should return the parent to their own
    // account, not to a password box.
    // remove() has already pointed the store at the fallback, so switchTo
    // would see it as "already active" and return without loading anything.
    // Load it here instead.
    if (this.accounts.active()) {
      try {
        this.me.set(await this.api.me());
        await this.refresh();
      } catch {
        // The fallback session was dead too. Its own 401 has already dropped
        // it, and the caller sees a signed-out app.
        this.me.set(null);
      }
    }
  }

  /**
   * Reloads everything the signed-in user can see, in parallel.
   *
   * Called after every mutation rather than patching state locally: the server
   * is authoritative about balances and listing status, and re-reading is both
   * simpler and correct when someone else in the house is also clicking.
   */
  async refresh(): Promise<void> {
    if (!this.me()) return;
    this.loading.set(true);
    try {
      const [me, users, listings, status] = await Promise.all([
        this.api.me(),
        this.api.users(),
        this.api.listings(),
        this.api.status(),
      ]);
      this.me.set(me);
      this.household.set(users.users);
      this.listings.set(listings.listings);
      this.status.set(status);
    } finally {
      this.loading.set(false);
    }
  }

  /** The display name behind an account id, for rendering the other side of a posting. */
  nameFor(accountId: string): string {
    const u = this.household().find((h) => h.account === accountId);
    return u?.display_name ?? accountId;
  }
}

/** True when an error means the session is gone and the user must log in again. */
export function isAuthError(e: unknown): boolean {
  return e instanceof ApiError && e.status === 401;
}
