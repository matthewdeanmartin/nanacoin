// The demo build's answer. Swapped in by `ng build --configuration demo`;
// see the fileReplacements in angular.json.
//
// DEMO_USERS lives here rather than beside the seed because the login form
// has to name it. Importing it from seed.ts pulled the whole demo ledger into
// the ordinary build, where it is dead weight that only the board pays for.

/** One of the people a visitor can look around as. */
export interface DemoPerson {
  username: string;
  label: string;
  hint: string;
}

export const IS_DEMO = true;

export const DEMO_USERS: readonly DemoPerson[] = [
  { username: 'nana', label: 'Nana', hint: 'issues coins, corrects mistakes, sees everything' },
  { username: 'dad', label: 'Dad', hint: 'an ordinary member, with a household float' },
  { username: 'mom', label: 'Mom', hint: 'posted a want-ad and has an offer waiting' },
  { username: 'sam', label: 'Sam', hint: 'a child, with chores to sell and an offer out' },
  { username: 'ivy', label: 'Ivy', hint: 'a child, with the smallest balance' },
];
