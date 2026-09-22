// The demo's ledger enforces the same rules the Go service does, so these
// check the rules rather than the plumbing: the ones a visitor could break by
// clicking, and the invariant that would hide an arithmetic mistake.

import { DemoError, DemoLedger, SYSTEM_ISSUANCE } from './ledger';
import { seed } from './seed';
import { employmentSnapshot, gdpSeries } from '../economy/series';

function household() {
  const l = new DemoLedger();
  l.provision('nana', 'Nana', 'demo', 'Test House');
  const nana = l.userByName('nana')!;
  const alice = l.addUser('alice', 'Alice', 'demo', 'user');
  const bob = l.addUser('bob', 'Bob', 'demo', 'user');
  l.issue(nana, alice.account, 100, 'Starting');
  l.issue(nana, bob.account, 50, 'Starting');
  return { l, nana, alice, bob };
}

describe('the demo ledger', () => {
  it('starts unprovisioned and refuses a second household', () => {
    const l = new DemoLedger();
    expect(l.status().provisioned).toBe(false);
    l.provision('nana', 'Nana', 'demo', 'House');
    expect(l.status().provisioned).toBe(true);
    expect(() => l.provision('other', 'Other', 'x', 'Another')).toThrow(DemoError);
  });

  it('lets a member change their own password but not another member\'s', () => {
    const { l, alice, bob } = household();
    expect(l.setUserPassword(alice, alice.id, 'new-pin').id).toBe(alice.id);
    expect(() => l.setUserPassword(alice, bob.id, 'stolen-pin')).toThrowError(DemoError);
    expect(() => l.setUserPassword(alice, alice.id, '123')).toThrowError(DemoError);
  });

  it('balances after every operation', () => {
    // The property that would break first if any arithmetic here were wrong.
    const { l, nana, alice, bob } = household();
    expect(l.balanced()).toBe(true);
    l.transfer(alice, bob.account, 10, 'thanks');
    expect(l.balanced()).toBe(true);
    l.retire(nana, bob.account, 5, 'oops');
    expect(l.balanced()).toBe(true);
  });

  it('counts circulation as the negation of the issuance account', () => {
    const { l } = household();
    expect(l.balanceOf(SYSTEM_ISSUANCE)).toBe(-150);
    expect(l.status().circulation).toBe(150);
  });

  it('moves money on a transfer', () => {
    const { l, alice, bob } = household();
    l.transfer(alice, bob.account, 30, 'for the LEGO');
    expect(l.balanceOf(alice.account)).toBe(70);
    expect(l.balanceOf(bob.account)).toBe(80);
  });

  it('refuses a transfer that would overdraw, leaving no trace', () => {
    const { l, alice, bob } = household();
    const before = l.status().transactions;
    expect(() => l.transfer(alice, bob.account, 500, 'too much')).toThrow(DemoError);
    // A refused transfer must not appear in the ledger at all.
    expect(l.status().transactions).toBe(before);
    expect(l.balanceOf(alice.account)).toBe(100);
  });

  it('refuses paying yourself', () => {
    const { l, alice } = household();
    expect(() => l.transfer(alice, alice.account, 5, '')).toThrow(DemoError);
  });

  it('refuses fractional and negative amounts', () => {
    const { l, alice, bob } = household();
    expect(() => l.transfer(alice, bob.account, 0.5, '')).toThrow(DemoError);
    expect(() => l.transfer(alice, bob.account, -5, '')).toThrow(DemoError);
    expect(() => l.transfer(alice, bob.account, 0, '')).toThrow(DemoError);
  });

  it('will not let anyone spend from the issuance account', () => {
    const { l, alice } = household();
    expect(() => l.transfer(alice, SYSTEM_ISSUANCE, 5, '')).toThrow(DemoError);
  });

  it('keeps issuing and reversing to Nana', () => {
    const { l, alice, bob } = household();
    expect(() => l.issue(alice, bob.account, 10, 'nope')).toThrow(DemoError);
    const txn = l.transfer(alice, bob.account, 5, '');
    expect(() => l.reverse(alice, txn.id, 'nope')).toThrow(DemoError);
  });

  it('reverses by appending a mirror rather than editing', () => {
    const { l, nana, alice, bob } = household();
    const txn = l.transfer(alice, bob.account, 40, 'wrong amount');
    const before = l.status().transactions;

    l.reverse(nana, txn.id, 'meant 4');

    // The original survives; a new transaction undoes it.
    expect(l.status().transactions).toBe(before + 1);
    expect(l.balanceOf(alice.account)).toBe(100);
    expect(l.balanceOf(bob.account)).toBe(50);
    expect(l.balanced()).toBe(true);
  });

  it('lets the recipient refund a classified payment and removes its economic effect', () => {
    const { l, alice, bob } = household();
    const txn = l.transfer(alice, bob.account, 20, 'Mow lawn', {
      economic_kind: 'LABOR', thing: 'mow', quantity_milli: 1000, unit: 'TASK',
    });
    const refund = l.reverse(bob, txn.id, 'Refund: Mow lawn');
    expect(refund.kind).toBe('REVERSAL');
    expect(refund.economic_kind).toBe('LABOR');
    expect(l.balanceOf(alice.account)).toBe(100);
    expect(l.balanceOf(bob.account)).toBe(50);
    const transactions = l.ledger(100).transactions;
    expect(gdpSeries(transactions, 'year').points.reduce((sum, point) => sum + point.value, 0)).toBe(0);
    expect(employmentSnapshot(transactions, [bob.account], refund.created_at).employed).toBe(0);
  });

  it('reverses even when the money has been spent', () => {
    // Blocking this would leave the wrong version of events as the record.
    const { l, nana, alice, bob } = household();
    const txn = l.transfer(alice, bob.account, 50, 'mistake');
    l.transfer(bob, alice.account, 100, 'spent it');

    l.reverse(nana, txn.id, 'correcting');
    expect(l.balanceOf(bob.account)).toBeLessThan(0);
    expect(l.balanced()).toBe(true);
  });

  it('refuses to reverse the same thing twice', () => {
    const { l, nana, alice, bob } = household();
    const txn = l.transfer(alice, bob.account, 10, '');
    l.reverse(nana, txn.id, 'once');
    expect(() => l.reverse(nana, txn.id, 'twice')).toThrow(DemoError);
  });

  it('hides other people\'s balances from an ordinary member', () => {
    const { l, nana, alice, bob } = household();
    const asAlice = l.allUsers(alice);
    expect(asAlice.find((u) => u.id === alice.id)?.balance).toBe(100);
    expect(asAlice.find((u) => u.id === bob.id)?.balance).toBeUndefined();

    // Nana sees everything.
    expect(l.allUsers(nana).every((u) => u.balance !== undefined)).toBe(true);
  });
});

describe('the demo marketplace', () => {
  it('buying pays the seller and closes the listing in one step', () => {
    const { l, alice, bob } = household();
    const listing = l.createListing(alice, { title: 'Cookies', description: '', price: 20 });

    const { listing: sold } = l.purchase(bob, listing.id);
    expect(sold.status).toBe('SOLD');
    expect(l.balanceOf(alice.account)).toBe(120);
    expect(l.balanceOf(bob.account)).toBe(30);
  });

  it('refuses buying your own listing, or one already sold', () => {
    const { l, alice, bob } = household();
    const listing = l.createListing(alice, { title: 'Cookies', description: '', price: 20 });
    expect(() => l.purchase(alice, listing.id)).toThrow(DemoError);
    l.purchase(bob, listing.id);
    expect(() => l.purchase(bob, listing.id)).toThrow(DemoError);
  });

  it('an offer moves no money until it is accepted', () => {
    const { l, alice, bob } = household();
    const listing = l.createListing(alice, { title: 'Switch time', description: '', price: 20 });

    l.makeOffer(bob, listing.id, 15, 'would you take 15?');
    expect(l.balanceOf(bob.account)).toBe(50);
    expect(l.listingById(listing.id).status).toBe('ACTIVE');
  });

  it('accepting an offer pays the seller at the offered price', () => {
    const { l, alice, bob } = household();
    const listing = l.createListing(alice, { title: 'Switch time', description: '', price: 20 });
    const offer = l.makeOffer(bob, listing.id, 15, '');

    l.acceptOffer(alice, offer.id);
    // 15, not the 20 asking price.
    expect(l.balanceOf(alice.account)).toBe(115);
    expect(l.balanceOf(bob.account)).toBe(35);
  });

  it('a want-ad pays the other way round', () => {
    // The poster has the money and wants the thing done, so accepting sends
    // coins from the poster to whoever offered.
    const { l, alice, bob } = household();
    const wanted = l.createListing(alice, {
      title: 'Peanut butter cookies',
      description: '',
      price: 25,
      side: 'BUY',
    });
    const offer = l.makeOffer(bob, wanted.id, 20, 'Saturday');

    l.acceptOffer(alice, offer.id);
    expect(l.balanceOf(alice.account)).toBe(80);
    expect(l.balanceOf(bob.account)).toBe(70);
  });

  it('only the listing owner may accept or decline', () => {
    const { l, alice, bob } = household();
    const listing = l.createListing(alice, { title: 'Thing', description: '', price: 10 });
    const offer = l.makeOffer(bob, listing.id, 8, '');
    expect(() => l.acceptOffer(bob, offer.id)).toThrow(DemoError);
    expect(() => l.declineOffer(bob, offer.id)).toThrow(DemoError);
  });

  it('keeps competing offers visible as not selected when one is accepted', () => {
    const { l, nana, alice, bob } = household();
    const carol = l.addUser('carol', 'Carol', 'demo', 'user');
    l.issue(nana, carol.account, 100, 'Starting');

    const listing = l.createListing(alice, { title: 'Thing', description: '', price: 10 });
    const first = l.makeOffer(bob, listing.id, 8, '');
    const second = l.makeOffer(carol, listing.id, 9, '');

    l.acceptOffer(alice, second.id);
    expect(l.offerById(first.id).status).toBe('NOT_SELECTED');
  });

  it('refuses duplicate open offers from one person', () => {
    const { l, alice, bob } = household();
    const listing = l.createListing(alice, { title: 'Thing', description: '', price: 10 });
    l.makeOffer(bob, listing.id, 8, 'First');
    expect(() => l.makeOffer(bob, listing.id, 9, 'Spam')).toThrow(DemoError);
  });
});

describe('the demo exchange', () => {
  it('settles the coin and dollar legs together', () => {
    const { l, nana, alice, bob } = household();
    l.issueUSD(nana, bob.account, 1_000, 'Cash float');
    const quote = l.postQuote(alice, 'ASK', 250_000, 10);

    const result = l.takeQuote(bob, quote.id);

    expect(result.quote.status).toBe('FILLED');
    expect(l.balanceOf(alice.account)).toBe(90);
    expect(l.balanceOf(bob.account)).toBe(60);
    expect(l.view(alice, alice).usd_cents).toBe(250);
    expect(l.view(bob, bob).usd_cents).toBe(750);
    expect(l.balanced()).toBe(true);
  });

  it('refuses a trade when the dollar buyer cannot pay', () => {
    const { l, alice, bob } = household();
    const quote = l.postQuote(alice, 'ASK', 250_000, 10);
    expect(() => l.takeQuote(bob, quote.id)).toThrow(DemoError);
    expect(quote.status).toBe('OPEN');
    expect(l.balanceOf(alice.account)).toBe(100);
    expect(l.balanceOf(bob.account)).toBe(50);
  });
});

describe('the seeded household', () => {
  it('produces a history worth showing', () => {
    const l = new DemoLedger();
    seed(l);

    const s = l.status();
    expect(s.provisioned).toBe(true);
    expect(s.users).toBe(5);
    expect(s.transactions).toBeGreaterThan(40);
    expect(s.active_listings).toBeGreaterThan(2);
    expect(s.ledger_balanced).toBe(true);
  });

  it('spreads transactions over time rather than one instant', () => {
    // A seed that lands everything in the same second collapses the economy
    // charts onto a single point - which is what happened the first time.
    const l = new DemoLedger();
    seed(l);

    const { transactions } = l.ledger(500);
    const days = new Set(transactions.map((t) => Math.floor(t.created_at / 86_400)));
    expect(days.size).toBeGreaterThan(20);
  });

  it('leaves nobody overdrawn', () => {
    const l = new DemoLedger();
    seed(l);
    for (const u of l.allUsers(l.userByName('nana')!)) {
      expect(u.balance ?? 0).toBeGreaterThanOrEqual(0);
    }
  });

  it('funds Nana with 1,000 coins and pays the sample lotto interest', () => {
    const l = new DemoLedger();
    seed(l);
    const nana = l.userByName('nana')!;
    expect(l.balanceOf(nana.account)).toBe(10_000_000 - 4_000);
  });

  it('leaves an offer waiting for a decision', () => {
    const l = new DemoLedger();
    seed(l);
    const mom = l.userByName('mom')!;
    expect(l.offersFor(mom).some((o) => o.status === 'OPEN')).toBe(true);
  });

  it('includes a reversal, so the audit trail has something in it', () => {
    const l = new DemoLedger();
    seed(l);
    expect(l.ledger(500).transactions.some((t) => t.kind === 'REVERSAL')).toBe(true);
  });
});
