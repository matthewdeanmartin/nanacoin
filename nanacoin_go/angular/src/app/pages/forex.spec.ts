// The exchange page, rendered against a fake book.
//
// What is worth pinning here is direction. A quote has two sides and a trade
// has two legs, and every one of those is a place where "sell" can quietly
// become "buy": the marketplace already shipped a bug where a want-ad paid
// backwards because a Side was dropped on the way to storage. These tests
// read the rendered text, not the internals, because the wrong direction is
// only wrong once a person reads it.

import { TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { provideHttpClientTesting } from '@angular/common/http/testing';

import { ApiBase } from '../api/api-base';
import { Quote, QuoteSide } from '../api/models';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { ForexPage } from './forex';

function quote(over: Partial<Quote> & { side: QuoteSide; cents_per_coin: number }): Quote {
  const coins = over.coins ?? 10;
  return {
    id: over.id ?? `q-${over.side}-${over.cents_per_coin}`,
    maker: 'account:alice',
    maker_name: 'Alice',
    coins,
    cents: coins * over.cents_per_coin,
    status: 'OPEN',
    created_at: 1_700_000_000,
    updated_at: 1_700_000_000,
    live: true,
    ...over,
  } as Quote;
}

/** Renders the page with a fixed book and returns its text. */
async function render(quotes: Quote[], me: Partial<{ account: string; balance: number; usd_cents: number }> = {}) {
  TestBed.configureTestingModule({
    providers: [provideHttpClient(), provideHttpClientTesting(), ApiBase],
  });
  const api = TestBed.inject(NanacoinService);
  api.quotes = () => Promise.resolve({ quotes });

  const session = TestBed.inject(Session);
  session.me.set({
    id: 'user:bob',
    account: me.account ?? 'account:bob',
    username: 'bob',
    display_name: 'Bob',
    role: 'user',
    status: 'ACTIVE',
    balance: me.balance ?? 100,
    usd_cents: me.usd_cents ?? 5000,
  } as never);

  const fixture = TestBed.createComponent(ForexPage);
  fixture.detectChanges();
  await fixture.whenStable();
  fixture.detectChanges();
  return { fixture, text: (fixture.nativeElement as HTMLElement).textContent ?? '' };
}

afterEach(() => TestBed.resetTestingModule());

describe('the exchange book', () => {
  it('describes an ask as paying dollars to get coins', async () => {
    const { text } = await render([quote({ side: 'ASK', cents_per_coin: 25, coins: 10 })]);

    // The card's own sentence, and the button that acts on it, must agree
    // that taking an ask means buying coins.
    expect(text).toContain('Alice will sell 10 coins for $2.50');
    expect(text).toContain('Buy coins');
  });

  it('describes a bid as giving coins to get dollars', async () => {
    const { text } = await render([quote({ side: 'BID', cents_per_coin: 30, coins: 4 })]);

    expect(text).toContain('Alice will pay $1.20 for 4 coins');
    expect(text).toContain('Sell coins');
  });

  it('orders asks cheapest first and bids best-price first', async () => {
    const { fixture } = await render([
      quote({ side: 'ASK', id: 'a30', cents_per_coin: 30 }),
      quote({ side: 'ASK', id: 'a20', cents_per_coin: 20 }),
      quote({ side: 'BID', id: 'b12', cents_per_coin: 12 }),
      quote({ side: 'BID', id: 'b18', cents_per_coin: 18 }),
    ]);
    const page = fixture.componentInstance as unknown as {
      asks: () => Quote[];
      bids: () => Quote[];
      spread: () => { bid: number; ask: number } | null;
    };

    expect(page.asks().map((q) => q.cents_per_coin)).toEqual([20, 30]);
    expect(page.bids().map((q) => q.cents_per_coin)).toEqual([18, 12]);
    // A buyer pays at least 20; a seller receives at most 18. That gap is the
    // spread, and it is the number that says this is a market.
    expect(page.spread()).toEqual({ bid: 18, ask: 20 });
  });

  it('shows no spread when only one side of the book exists', async () => {
    const { fixture } = await render([quote({ side: 'ASK', cents_per_coin: 25 })]);
    const page = fixture.componentInstance as unknown as { spread: () => unknown };
    expect(page.spread()).toBeNull();
  });

  it('offers to withdraw your own quote rather than to take it', async () => {
    const { text } = await render(
      [quote({ side: 'ASK', cents_per_coin: 25, maker: 'account:bob', maker_name: 'Bob' })],
      { account: 'account:bob' },
    );

    // Taking your own quote is refused by the server with a 400; not offering
    // the button is how that refusal stops being a surprise.
    expect(text).toContain('Withdraw');
    expect(text).not.toContain('Buy coins');
  });

  it('keeps filled and cancelled quotes out of the live book', async () => {
    const { fixture, text } = await render([
      quote({ side: 'ASK', id: 'open', cents_per_coin: 25 }),
      quote({ side: 'ASK', id: 'gone', cents_per_coin: 5, live: false, status: 'FILLED' }),
    ]);
    const page = fixture.componentInstance as unknown as {
      asks: () => Quote[];
      settled: () => Quote[];
    };

    // A filled quote at a spectacular rate must not sit at the top of the
    // book looking takeable.
    expect(page.asks().map((q) => q.id)).toEqual(['open']);
    expect(page.settled().map((q) => q.id)).toEqual(['gone']);
    expect(text).toContain('no longer live');
  });

  it('formats cents under a dollar as cents and above as dollars', async () => {
    const { fixture } = await render([]);
    const page = fixture.componentInstance as unknown as {
      cents: (n: number) => string;
      dollars: (n: number) => string;
    };

    expect(page.cents(25)).toBe('25c');
    expect(page.cents(150)).toBe('$1.50');
    expect(page.dollars(0)).toBe('$0.00');
    expect(page.dollars(5)).toBe('$0.05');
    expect(page.dollars(1000)).toBe('$10.00');
  });

  it('says what a posted rate would do, in the right direction', async () => {
    const { fixture } = await render([]);
    const page = fixture.componentInstance as unknown as {
      side: QuoteSide;
      coins: number | null;
      rate: number | null;
      preview: () => string | null;
    };

    page.coins = 4;
    page.rate = 25;

    page.side = 'ASK';
    expect(page.preview()).toBe('You give 4 coins, you get $1.00.');

    page.side = 'BID';
    expect(page.preview()).toBe('You pay $1.00, you get 4 coins.');
  });

  it('reports an old board as unsupported rather than as an error', async () => {
    TestBed.configureTestingModule({
      providers: [provideHttpClient(), provideHttpClientTesting(), ApiBase],
    });
    const api = TestBed.inject(NanacoinService);
    const { ApiError } = await import('../api/nanacoin.service');
    api.quotes = () => Promise.reject(new ApiError(404, 'not_found', 'no'));

    const fixture = TestBed.createComponent(ForexPage);
    fixture.detectChanges();
    await fixture.whenStable();
    fixture.detectChanges();

    expect((fixture.nativeElement as HTMLElement).textContent).toContain(
      'connected server does not support currency exchange',
    );
  });
});
