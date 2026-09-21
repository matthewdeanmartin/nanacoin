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
  ledger.issue(nanaUser, dad.account, 500, 'Monthly household float');
  ledger.issue(nanaUser, mom.account, 500, 'Monthly household float');
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

  // Almost a year of sparse monthly observations makes Month and Year useful
  // in the economy charts without stuffing the tiny-board model with assets.
  for (let month = 0; month < 11; month++) {
    ledger.transfer(dad, sam.account, 12 + (month % 3), 'Monthly yard work', {
      economic_kind: 'LABOR', thing: 'thing-demo-monthly-yard-work',
      quantity_milli: 1000, unit: 'TASK',
    });
    ledger.transfer(sam, mom.account, 5 + Math.floor(month / 3), 'Peanut butter cookies', {
      economic_kind: 'GOOD', thing: 'thing-demo-peanut-butter-cookies',
      quantity_milli: 1000, unit: 'BATCH',
    });
    ledger.advance(30 * DAY);
  }

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
        const description = chores[chore++ % chores.length];
        ledger.transfer(
          parent,
          child.account,
          5 + ((week + chore) % 3) * 2,
          description,
          {
            economic_kind: 'LABOR',
            thing: `thing-demo-${description.toLocaleLowerCase().replaceAll(' ', '-')}`,
            quantity_milli: 1000,
            unit: 'TASK',
          },
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
      const description = treats[treat++ % treats.length];
      const good = description.includes('cookies') || description.includes('ice cream');
      ledger.transfer(child, to.account, 3 + (treat % 4) * 3, description, {
        economic_kind: good ? 'GOOD' : 'OTHER',
        thing: `thing-demo-${description.toLocaleLowerCase().replaceAll(' ', '-')}`,
        quantity_milli: 1000,
        unit: good && description.includes('cookies') ? 'BATCH' : 'EACH',
      });
      ledger.advance(DAY);
    }

    if (week === 3) {
      ledger.advance(DAY);
      ledger.issue(nanaUser, sam.account, 30, 'Birthday');
      ledger.transfer(dad, sam.account, 12, 'Birthday gift', {
        economic_kind: 'GIFT', quantity_milli: 1000, unit: 'EACH',
      });
    }
    ledger.advance(DAY);
  }

  // A mistake and its correction, because the audit trail is one of the
  // things worth showing: the original stays, and a mirror transaction undoes
  // it, rather than the record being edited.
  const wrong = ledger.transfer(dad, ivy.account, 40, 'Meant to send 4', {
    economic_kind: 'OTHER', quantity_milli: 1000, unit: 'EACH',
  });
  ledger.advance(600);
  ledger.reverse(nanaUser, wrong.id, 'Wrong amount - meant 4, not 40');

  // A market with something in it, in both directions.
  ledger.advance(DAY);
  ledger.createListing(sam, {
    title: 'One hour of Switch time',
    description: 'Uninterrupted, and I will not ask for it back',
    price: 12,
    economic_kind: 'OTHER', quantity_milli: 1000, unit: 'HOUR', standard: true,
  });
  ledger.createListing(ivy, {
    title: 'Old LEGO set',
    description: 'All the pieces, mostly',
    price: 30,
    economic_kind: 'GOOD', quantity_milli: 1000, unit: 'EACH', standard: true,
  });
  // A want-ad: the thing the marketplace could not do before offers existed.
  ledger.advance(DAY);
  ledger.createListing(mom, {
    title: 'Peanut butter cookies',
    description: 'A whole batch, by Saturday',
    price: 25,
    side: 'BUY',
    economic_kind: 'GOOD', thing: 'thing-demo-peanut-butter-cookies',
    quantity_milli: 1000, unit: 'BATCH', standard: true,
  });
  ledger.createListing(nanaUser, {
    title: 'Wash the car',
    description: 'Inside and out, before Sunday',
    price: 40,
    side: 'BUY',
    economic_kind: 'LABOR', quantity_milli: 1000, unit: 'TASK', standard: true,
  });

  // Repeat sales at different prices make the inflation display demonstrate
  // real observations instead of an empty-state lecture.
  ledger.advance(DAY);
  const firstCookies = ledger.createListing(mom, {
    title: 'Peanut butter cookies', description: 'One batch', price: 6,
    economic_kind: 'GOOD', thing: 'thing-demo-peanut-butter-cookies',
    quantity_milli: 1000, unit: 'BATCH', standard: true,
  });
  ledger.purchase(sam, firstCookies.id);
  ledger.advance(DAY);
  const secondCookies = ledger.createListing(mom, {
    title: 'Peanut butter cookies', description: 'One batch', price: 9,
    economic_kind: 'GOOD', thing: 'thing-demo-peanut-butter-cookies',
    quantity_milli: 1000, unit: 'BATCH', standard: true,
  });
  ledger.purchase(ivy, secondCookies.id);

  // Something else already sold, so the market has varied history.
  ledger.advance(DAY);
  const sold = ledger.createListing(ivy, {
    title: 'A friendship bracelet',
    description: 'Made it myself',
    price: 8,
    economic_kind: 'GOOD', quantity_milli: 1000, unit: 'EACH', standard: false,
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

  // Keep current-week employment visible after the historical seed and the
  // market examples have advanced the demo clock.
  ledger.advance(DAY);
  ledger.transfer(dad, sam.account, 11, 'Tidy the pantry', {
    economic_kind: 'LABOR', thing: 'thing-demo-tidy-the-pantry',
    quantity_milli: 1000, unit: 'TASK',
  });
  ledger.transfer(mom, ivy.account, 13, 'Organize the bookshelf', {
    economic_kind: 'LABOR', thing: 'thing-demo-organize-the-bookshelf',
    quantity_milli: 1000, unit: 'TASK',
  });
}
