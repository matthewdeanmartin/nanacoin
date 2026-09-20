// A household with a history, so the demo opens on something worth looking at.
//
// An empty ledger demonstrates nothing: the charts are blank, the market is
// empty, and every screen says "nothing yet". So the demo starts eight weeks
// in, with allowances paid, chores done, cookies bought, one thing reversed
// and a couple of offers still open.
//
// The transactions are spread over real days rather than created in one
// instant. That is not decoration - a seed that lands every transaction in
// the same second collapses the economy charts onto a single point, which is
// exactly what happened the first time this was tested against real data.

import { DemoLedger } from './ledger';

const DAY = 86_400;

export function seed(ledger: DemoLedger): void {
  const nana = ledger.provision('nana', 'Nana', 'demo', 'The Demo House');
  const nanaUser = ledger.userByName('nana')!;

  const dad = ledger.addUser('dad', 'Dad', 'demo', 'user');
  const mom = ledger.addUser('mom', 'Mom', 'demo', 'user');
  const sam = ledger.addUser('sam', 'Sam', 'demo', 'user');
  const ivy = ledger.addUser('ivy', 'Ivy', 'demo', 'user');
  void nana;

  // Nana is the central bank, so the demo should also demonstrate that she
  // can inject currency on demand. Keep a visible reserve in her own account
  // rather than making every issuance start from an empty balance.
  ledger.issue(nanaUser, nanaUser.account, 1000, 'Central-bank warchest');

  // Opening balances. The parents hold the float; the children start small,
  // which is what makes the first few weeks of allowance visible on a chart.
  ledger.issue(nanaUser, dad.account, 200, 'Monthly household float');
  ledger.issue(nanaUser, mom.account, 200, 'Monthly household float');
  ledger.issueUSD(nanaUser, dad.account, 10_000, 'Household cash float');
  ledger.issueUSD(nanaUser, mom.account, 10_000, 'Household cash float');
  ledger.advance(DAY);
  ledger.issue(nanaUser, sam.account, 25, 'Starting allowance');
  ledger.issue(nanaUser, ivy.account, 25, 'Starting allowance');

  const chores = [
    'Do the dishes',
    'Take out the trash',
    'Vacuum the stairs',
    'Walk the dog',
    'Fold the laundry',
    'Water the houseplants',
    'Clean the bathroom',
  ];
  const treats = [
    'Peanut butter cookies',
    'A bowl of ice cream',
    'One hour of Switch time',
    'Pick the movie',
    'Stay up half an hour late',
  ];

  // Eight weeks of an ordinary household: chores paid on Saturdays, treats
  // bought back during the week.
  let chore = 0;
  let treat = 0;
  for (let week = 0; week < 8; week++) {
    // Chores are paid as they are done, across two days rather than all at
    // once, so a week is several points on a chart instead of one spike.
    for (const parent of [dad, mom]) {
      ledger.advance(DAY);
      for (const child of [sam, ivy]) {
        ledger.transfer(
          parent,
          child.account,
          5 + ((week + chore) % 3) * 2,
          chores[chore++ % chores.length],
        );
        ledger.advance(3600 * 6);
      }
    }

    ledger.advance(2 * DAY);
    for (const child of [sam, ivy]) {
      // Not every week: a child who spends everything every week has a
      // sawtooth balance, and one who never spends has a straight line.
      if ((week + child.username.length) % 3 === 0) continue;
      const to = week % 2 === 0 ? dad : mom;
      ledger.transfer(child, to.account, 3 + (treat % 4) * 3, treats[treat++ % treats.length]);
      ledger.advance(DAY);
    }

    if (week === 3) {
      ledger.advance(DAY);
      ledger.issue(nanaUser, sam.account, 30, 'Birthday');
    }
    ledger.advance(DAY);
  }

  // A mistake and its correction, because the audit trail is one of the
  // things worth showing: the original stays, and a mirror transaction undoes
  // it, rather than the record being edited.
  const wrong = ledger.transfer(dad, ivy.account, 40, 'Meant to send 4');
  ledger.advance(600);
  ledger.reverse(nanaUser, wrong.id, 'Wrong amount - meant 4, not 40');

  // A market with something in it, in both directions.
  ledger.advance(DAY);
  ledger.createListing(sam, {
    title: 'One hour of Switch time',
    description: 'Uninterrupted, and I will not ask for it back',
    price: 12,
  });
  ledger.createListing(ivy, {
    title: 'Old LEGO set',
    description: 'All the pieces, mostly',
    price: 30,
  });
  ledger.createListing(dad, {
    title: '$5 of real cash',
    description: 'NanaCoin only records the coin side',
    price: 50,
    kind: 'currency',
    currency: 'USD',
    minor_units: 500,
  });

  // A want-ad: the thing the marketplace could not do before offers existed.
  ledger.advance(DAY);
  ledger.createListing(mom, {
    title: 'Peanut butter cookies',
    description: 'A whole batch, by Saturday',
    price: 25,
    side: 'BUY',
  });
  ledger.createListing(nanaUser, {
    title: 'Wash the car',
    description: 'Inside and out, before Sunday',
    price: 40,
    side: 'BUY',
  });

  // Something already sold, so the market has history rather than only stock.
  ledger.advance(DAY);
  const sold = ledger.createListing(ivy, {
    title: 'A friendship bracelet',
    description: 'Made it myself',
    price: 8,
  });
  ledger.purchase(sam, sold.id);

  // Offers left open, so the Offers page has a decision waiting in it.
  ledger.advance(DAY);
  const cookies = ledger.allListings('ACTIVE').find((l) => l.title.includes('cookies'))!;
  ledger.makeOffer(sam, cookies.id, 20, 'I can do it Saturday morning');

  const switchTime = ledger.allListings('ACTIVE').find((l) => l.title.includes('Switch'))!;
  ledger.makeOffer(ivy, switchTime.id, 9, 'Would you take 9?');

  // Both sides of the exchange book are visible in the showcase. Dad can pay
  // dollars for coins; Sam can sell coins for dollars.
  ledger.postQuote(dad, 'BID', 20, 10);
  ledger.postQuote(sam, 'ASK', 30, 8);

  ledger.advance(DAY);
}
