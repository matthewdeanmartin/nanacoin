import { DemoError, DemoLedger } from '../demo/ledger';
import { GOOD_DEED, GOOD_DEED_IDEAS, missingIdeas } from './good-deeds';

const bytes = (s: string) => new TextEncoder().encode(s).length;

describe('good deed catalog', () => {
  it('has 25 distinct deeds that fit the board', () => {
    expect(GOOD_DEED_IDEAS).toHaveLength(25);
    expect(new Set(GOOD_DEED_IDEAS.map((d) => d.title.toLocaleLowerCase())).size).toBe(25);
    for (const d of GOOD_DEED_IDEAS) {
      expect(bytes(d.title)).toBeLessThanOrEqual(80);
      expect(bytes(d.description)).toBeLessThanOrEqual(96);
      expect(Number.isInteger(d.reward) && d.reward > 0).toBe(true);
    }
  });
  it('skips deeds already posted and ignores closed or ordinary listings', () => {
    const [first, second] = GOOD_DEED_IDEAS;
    expect(missingIdeas([
      { title: first.title.toUpperCase(), kind: GOOD_DEED, status: 'ACTIVE' },
      { title: second.title, kind: GOOD_DEED, status: 'CANCELLED' },
      { title: GOOD_DEED_IDEAS[2].title, kind: 'item', status: 'ACTIVE' },
    ]).map((d) => d.title)).toEqual(GOOD_DEED_IDEAS.slice(1).map((d) => d.title));
  });
});

describe('demo good deeds', () => {
  function household() {
    const l = new DemoLedger();
    l.provision('nana', 'Nana', 'demo', 'House');
    const nana = l.userByName('nana')!, sam = l.addUser('sam', 'Sam', 'demo', 'user'), ivy = l.addUser('ivy', 'Ivy', 'demo', 'user');
    return { l, nana, sam, ivy };
  }
  const deed = { title: 'Plant something', description: '', price: 40, side: 'BUY' as const, kind: GOOD_DEED };
  it('lets only Nana post a good deed, as a reward she pays', () => {
    const { l, sam, nana } = household();
    expect(() => l.createListing(sam, deed)).toThrow(DemoError);
    expect(() => l.createListing(nana, { ...deed, side: 'SELL' })).toThrow(DemoError);
    expect(l.createListing(nana, deed).kind).toBe(GOOD_DEED);
  });
  it('issues new money on acceptance and stays open for the next person', () => {
    const { l, nana, sam, ivy } = household();
    const listing = l.createListing(nana, deed);
    expect(() => l.purchase(sam, listing.id)).toThrow(DemoError);
    const nanaBefore = l.balanceOf(nana.account);
    const first = l.acceptOffer(nana, l.makeOffer(sam, listing.id, 40, 'Planted basil').id);
    l.acceptOffer(nana, l.makeOffer(ivy, listing.id, 40, 'Planted a tree').id);
    expect(first.transaction.kind).toBe('ISSUE');
    expect([l.balanceOf(sam.account), l.balanceOf(ivy.account), l.balanceOf(nana.account)]).toEqual([40, 40, nanaBefore]);
    expect(l.allListings().find((x) => x.id === listing.id)?.status).toBe('ACTIVE');
    expect(l.balanced()).toBe(true);
  });
  it('lets members write only their own one-line bio', () => {
    const { l, sam, ivy } = household();
    expect(l.setUserBio(sam, sam.id, 'I plant things').bio).toBe('I plant things');
    expect(() => l.setUserBio(sam, ivy.id, 'hacked')).toThrow(DemoError);
    expect(() => l.setUserBio(sam, sam.id, 'x'.repeat(97))).toThrow(DemoError);
    expect(l.allUsers(ivy).find((u) => u.id === sam.id)?.bio).toBe('I plant things');
  });
});
