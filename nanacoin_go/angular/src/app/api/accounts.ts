// The signed-in accounts this browser is holding tokens for, and which one is
// active.
//
// # Why several at once
//
// A household has one person who is both a parent and the central bank. Nana
// issues the coin and corrects the mistakes; the same adult then takes part in
// the market like everyone else. Before this, switching between those roles
// meant logging out and typing a password back in, several times an evening.
//
// So tokens are keyed by user rather than being a single slot. Switching is
// then a pointer move: no network, no re-entered password, and the sessions
// that are not active stay valid on the server, which already issues one token
// per login and does not care how many a browser holds.
//
// # Where they are kept
//
// sessionStorage, deliberately, which is where the single token lived before.
// Closing the tab ends every session at once. On a shared family computer that
// is the property worth keeping - and it matters more here, not less, because
// there are now several sessions to walk away from rather than one. Surviving
// a browser restart is explicitly not a goal; the board forgets its sessions
// on reboot anyway, so a token that outlived the tab would usually be dead.

import { Injectable, computed, signal } from '@angular/core';

/** One signed-in account: the token, plus enough to draw a switcher. */
export interface StoredAccount {
  userId: string;
  username: string;
  displayName: string;
  role: 'nana' | 'user';
  token: string;
}

const STORE_KEY = 'nanacoin.accounts';

/** The shape written to sessionStorage. Versioned so a change can be ignored. */
interface Persisted {
  v: 1;
  accounts: StoredAccount[];
  activeUserId: string | null;
}

@Injectable({ providedIn: 'root' })
export class Accounts {
  private readonly state = signal<Persisted>(read());

  /** Every account this browser holds a token for, in the order added. */
  readonly all = computed(() => this.state().accounts);

  /** The active account, or null when nobody is signed in. */
  readonly active = computed(() => {
    const { accounts, activeUserId } = this.state();
    return accounts.find((a) => a.userId === activeUserId) ?? null;
  });

  /** The active account's bearer token, which is what the HTTP layer wants. */
  readonly token = computed(() => this.active()?.token ?? null);

  /** True once there is more than one, which is when a switcher earns its space. */
  readonly hasSeveral = computed(() => this.state().accounts.length > 1);

  /**
   * Records a freshly logged-in account and makes it active.
   *
   * Logging in again as someone already here replaces their token rather than
   * adding a second entry: the old one may well still be valid server-side,
   * but this browser has no further use for it and two rows for one person in
   * the switcher would be a bug, not a feature.
   */
  add(account: StoredAccount): void {
    this.state.update((s) => {
      const accounts = s.accounts.filter((a) => a.userId !== account.userId);
      accounts.push(account);
      return { ...s, accounts, activeUserId: account.userId };
    });
    this.persist();
  }

  /** Switches to an account already held. Unknown ids are ignored. */
  activate(userId: string): void {
    if (!this.state().accounts.some((a) => a.userId === userId)) return;
    this.state.update((s) => ({ ...s, activeUserId: userId }));
    this.persist();
  }

  /**
   * Drops one account, activating another if the one dropped was active.
   *
   * Falling back to whoever remains is what makes "log out" on a switcher feel
   * like leaving one account rather than ending the whole evening. When the
   * last one goes, activeUserId becomes null and the app returns to login.
   */
  remove(userId: string): void {
    this.state.update((s) => {
      const accounts = s.accounts.filter((a) => a.userId !== userId);
      const activeUserId =
        s.activeUserId === userId ? (accounts[accounts.length - 1]?.userId ?? null) : s.activeUserId;
      return { ...s, accounts, activeUserId };
    });
    this.persist();
  }

  /** Forgets every account. The "log out of everything" path. */
  clear(): void {
    this.state.set({ v: 1, accounts: [], activeUserId: null });
    this.persist();
  }

  /**
   * Replaces the active account's token, or drops the account when it is gone.
   *
   * The HTTP layer calls this on a 401: that session has expired or the board
   * has rebooted, and the right response is to forget that one account rather
   * than to sign everybody out. The others may still be perfectly good - they
   * were issued separately and, on a reboot, are equally dead, which the next
   * request will discover for itself.
   */
  invalidateActive(): void {
    const active = this.active();
    if (active) this.remove(active.userId);
  }

  private persist(): void {
    try {
      sessionStorage.setItem(STORE_KEY, JSON.stringify(this.state()));
    } catch {
      // Private browsing and blocked site data both throw. Everything still
      // works for this tab; the cost is re-logging in after a reload.
    }
  }
}

function read(): Persisted {
  const empty: Persisted = { v: 1, accounts: [], activeUserId: null };
  try {
    const raw = sessionStorage.getItem(STORE_KEY);
    if (!raw) return empty;
    const parsed = JSON.parse(raw) as Persisted;
    // Anything not matching the current shape is discarded rather than
    // migrated. The cost of being wrong is one re-login; the cost of trusting
    // a half-understood blob is an unreadable auth bug.
    if (parsed?.v !== 1 || !Array.isArray(parsed.accounts)) return empty;
    const accounts = parsed.accounts.filter(
      (a) => a && typeof a.userId === 'string' && typeof a.token === 'string',
    );
    const activeUserId = accounts.some((a) => a.userId === parsed.activeUserId)
      ? parsed.activeUserId
      : (accounts[accounts.length - 1]?.userId ?? null);
    return { v: 1, accounts, activeUserId };
  } catch {
    return empty;
  }
}
