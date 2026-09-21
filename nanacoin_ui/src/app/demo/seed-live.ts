// Seeding a real server with a year of plausible household history.
//
// Distinct from demo/seed.ts, which fills an in-memory ledger for the public
// demo. This one drives the ordinary API against whatever server is
// configured - including the board - so every record it creates is a real
// journalled transaction that went through the same validation as a typed one.
//
// # Why it runs in the browser
//
// The Go service has a Seed for tests and a `-seed` flag was never wired up,
// and neither helps someone looking at a fresh household wondering what this
// thing is for. Doing it from the client needs no new endpoint, no new board
// RAM, and no firmware change - which is the whole constraint this project
// works under.
//
// # Why it is paced
//
// The board has four request workers and about 10 KB of headroom. A seeder
// that fires as fast as the socket allows is a load test, and we have one of
// those. This one waits between writes, backs off when the board says it is
// busy, and reports progress so a slow run looks like work rather than a
// hang.

import { Injectable, inject, signal } from '@angular/core';

import { CATEGORIES, ITEMS } from '../catalog/catalog';
import { Log } from '../api/log';
import { ApiError, NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session } from '../api/session';

/** How far along a seed run is, for the progress readout. */
export interface SeedProgress {
  phase: string;
  done: number;
  total: number;
}

export interface SeedOptions {
  /** Household members besides Nana. */
  members: number;
  /** Weeks of history. */
  weeks: number;
  /** Economic events per week. */
  perWeek: number;
  /** Listings posted over the period. */
  listings: number;
  /** Password given to every generated member. */
  password: string;
}

export function defaultSeed(): SeedOptions {
  // A year, at the household rate the spec estimates.
  return { members: 4, weeks: 52, perWeek: 6, listings: 24, password: 'demo-pin-1234' };
}

@Injectable({ providedIn: 'root' })
export class LiveSeeder {
  private readonly api = inject(NanacoinService);
  private readonly session = inject(Session);
  private readonly log = inject(Log);

  readonly progress = signal<SeedProgress | null>(null);
  readonly running = signal(false);

  /** Set when the run stopped early, so the UI can say why. */
  readonly lastError = signal('');

  /**
   * Fills the household with a year of history.
   *
   * Nana-only, and the server enforces that regardless: creating users and
   * issuing coin are already hers. Members are created here, so their
   * passwords are known and the seeder can act as each of them - a transfer's
   * source account comes from the authenticated user, so there is no way to
   * move money on someone's behalf.
   */
  async run(opts: SeedOptions = defaultSeed()): Promise<void> {
    if (this.running()) return;
    this.running.set(true);
    this.lastError.set('');

    const nana = this.session.me();
    if (!nana || nana.role !== 'nana') {
      this.lastError.set('Only Nana can seed a household.');
      this.running.set(false);
      return;
    }

    // Everything is restored at the end: the seeder logs in as each member to
    // act as them, and whoever started it should get their session back.
    const startedAs = nana.id;

    try {
      const members = await this.createMembers(opts);
      if (members.length < 2) {
        throw new Error('seeding needs at least two members');
      }
      await this.fund(members, opts);
      const listings = await this.postListings(members, opts);
      await this.trade(members, listings, opts);
      await this.makeOffers(members, listings);

      this.log.info('seed', 'finished', { members: members.length });
    } catch (e) {
      this.lastError.set(explain(e));
      this.log.error('seed', 'stopped early', {
        error: e instanceof Error ? e.message : String(e),
        code: e instanceof ApiError ? e.code : undefined,
        status: e instanceof ApiError ? e.status : undefined,
        phase: this.progress()?.phase,
        at: this.progress()?.done,
      });
    } finally {
      // Back to whoever pressed the button.
      try {
        await this.session.switchTo(startedAs);
      } catch {
        // Their session outlived the run; the shell will send them to login.
      }
      this.progress.set(null);
      this.running.set(false);
      await this.session.refresh().catch(() => undefined);
    }
  }

  private async createMembers(opts: SeedOptions) {
    const names = ['Dad', 'Mom', 'Sam', 'Ivy', 'Gran', 'Max'];
    const made: { id: string; username: string; account: string }[] = [];

    for (let i = 0; i < opts.members; i++) {
      const display = names[i % names.length];
      const username = `${display.toLowerCase()}${i >= names.length ? i : ''}`;
      this.step('Adding members', i, opts.members);

      // An existing household may already have these people; reuse them
      // rather than failing the whole run on a name clash.
      const existing = this.session.household().find((u) => u.username === username);
      if (existing) {
        made.push({ id: existing.id, username, account: existing.account });
        continue;
      }
      const u = await this.write(() =>
        this.api.createUser(username, display, opts.password, false),
      );
      made.push({ id: u.id, username, account: u.account });
    }
    await this.session.refresh();
    return made;
  }

  /** Nana issues a float, so the transfers below cannot run out of money. */
  private async fund(members: { account: string }[], opts: SeedOptions) {
    const float = opts.weeks * opts.perWeek * 3 * 10 ** (this.session.status()?.decimals ?? 4);
    for (let i = 0; i < members.length; i++) {
      this.step('Issuing the opening float', i, members.length);
      await this.write(() =>
        this.api.issue(members[i].account, float, 'Opening float', newIdempotencyKey()),
      );
    }
  }

  private async postListings(
    members: { username: string; account: string }[],
    opts: SeedOptions,
  ) {
    const ids: string[] = [];
    for (let i = 0; i < opts.listings; i++) {
      this.step('Posting listings', i, opts.listings);
      const seller = members[i % members.length];
      await this.actAs(seller.username, opts.password);

      const item = ITEMS[(i * 7) % ITEMS.length];
      // A quarter of them are want-ads, which is what makes the market look
      // like a household rather than a shop.
      const side = i % 4 === 3 ? 'BUY' : undefined;

      try {
        const l = await this.write(() =>
          this.api.createListing({
            title: item.name,
            description: categoryName(item.cat),
            price: (3 + ((i * 5) % 28)) * 10 ** (this.session.status()?.decimals ?? 4),
            ...(side ? { side } : {}),
          }),
        );
        ids.push(l.id);
      } catch (e) {
        // 507 is the listing table full, which is the board saying "no room"
        // rather than failing. Measured against the real board: it refuses
        // cleanly at 48 listings with the heap untouched. Grinding through
        // another twenty refusals would tell nobody anything, so stop posting
        // listings and get on with the transactions.
        if (e instanceof ApiError && e.status === 507) {
          this.log.info('seed', 'listing table is full; posting no more', { posted: ids.length });
          break;
        }
        throw e;
      }
    }
    return ids;
  }

  /**
   * The economic events: transfers for chores, and purchases.
   *
   * Deliberately shaped like a household's rather than uniform - mostly small
   * transfers, some purchases, a few corrections. Repetition matters: the
   * board interns repeated strings, so a realistic seed is a fairer test of
   * its memory than a random one.
   */
  private async trade(
    members: { id: string; username: string; account: string }[],
    listings: string[],
    opts: SeedOptions,
  ) {
    const total = opts.weeks * opts.perWeek;
    const chores = ITEMS.filter((i) => i.cat === 1).map((i) => i.name);
    let unsold = [...listings];

    for (let i = 0; i < total; i++) {
      this.step('A year of chores and purchases', i, total);

      const from = members[i % members.length];
      const to = members[(i + 1) % members.length];
      await this.actAs(from.username, opts.password);

      // Roughly one in eight is a purchase, while listings remain.
      if (i % 8 === 3 && unsold.length > 0) {
        const id = unsold[0];
        unsold = unsold.slice(1);
        try {
          await this.write(() => this.api.purchase(id, newIdempotencyKey()));
          continue;
        } catch {
          // Own listing, already sold, or not affordable - fall through to a
          // transfer rather than abandoning the run.
        }
      }

      await this.write(() =>
        this.api.transfer(
          to.account,
          (1 + (i % 9)) * 10 ** (this.session.status()?.decimals ?? 4),
          chores[i % chores.length],
          newIdempotencyKey(),
        ),
      );
    }
  }

  /** A few open offers, so the Offers tab has something in it. */
  private async makeOffers(
    members: { username: string; account: string }[],
    listings: string[],
  ) {
    const open = listings.slice(-4);
    for (let i = 0; i < open.length; i++) {
      this.step('Making a few offers', i, open.length);
      const buyer = members[(i + 1) % members.length];
      await this.actAs(buyer.username, defaultSeed().password);
      try {
        await this.write(() =>
          this.api.makeOffer(open[i], 2 + i, 'Would you take this?', newIdempotencyKey()),
        );
      } catch {
        // Own listing, or already closed. Not worth stopping for.
      }
    }
  }

  /**
   * Becomes another member, so their transfers come from their own account.
   *
   * The server derives a transfer's source from the authenticated user, which
   * is the rule that makes "a client cannot spend someone else's money" true.
   * Seeding has to respect it like anything else.
   */
  private async actAs(username: string, password: string) {
    if (this.session.me()?.username === username) return;
    const held = this.session.signedInAccounts().find((a) => a.username === username);
    if (held) {
      await this.session.switchTo(held.userId);
      return;
    }
    await this.session.login(username, password);
  }

  /**
   * One write, paced.
   *
   * The delay is the difference between seeding and load-testing. The client's
   * own 503 backoff handles the board asking for a pause; this just keeps the
   * request rate somewhere a four-worker device can live with.
   */
  private async write<T>(fn: () => Promise<T>): Promise<T> {
    const result = await fn();
    await new Promise((r) => setTimeout(r, WRITE_PACING_MS));
    return result;
  }

  private step(phase: string, done: number, total: number) {
    this.progress.set({ phase, done, total });
  }
}

/**
 * Milliseconds between writes.
 *
 * 120ms is about eight writes a second, which the board serves comfortably -
 * the measured ceiling is around ten requests a second before latency climbs.
 * Fast enough that a year of history takes a couple of minutes rather than
 * ten.
 */
const WRITE_PACING_MS = 120;

/**
 * Turns the server's error into something that names the cause.
 *
 * The seeder logs in as each member, and sessions are RAM-only, last eight
 * hours, and are never logged out. On a board with 32 slots a few runs fill
 * the table; the login is then refused, the client drops the account it could
 * not re-authenticate, and the *next* request fails with the server's own
 * wording - "invalid or expired token". Which is true, and tells whoever is
 * reading it nothing about what to do.
 *
 * The board now recycles a user's oldest session rather than refusing, so
 * this should be rare. It is still worth saying plainly when it happens.
 */
function explain(e: unknown): string {
  if (e instanceof ApiError) {
    if (e.code === 'too_many_sessions') {
      return (
        'the board has no room for another sign-in. It holds a limited number ' +
        'of sessions and frees them only as they expire. Restarting the board ' +
        'clears them.'
      );
    }
    if (e.status === 401) {
      return (
        'the sign-in used for seeding stopped being accepted. If the board was ' +
        'restarted mid-run its sessions are gone; sign in again and retry.'
      );
    }
    if (e.status === 507) {
      return 'the board is out of room for more records.';
    }
    return e.message;
  }
  return e instanceof Error ? e.message : String(e);
}

function categoryName(id: number): string {
  return CATEGORIES.find((c) => c.id === id)?.name ?? '';
}
