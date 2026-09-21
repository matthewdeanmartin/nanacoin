// Whether this build is the public demo.
//
// A compile-time constant rather than a runtime check, so the ordinary build
// does not carry the in-memory ledger, the seeded household or the
// pick-a-person login at all. `ng build --configuration demo` swaps this file
// for demo.public.ts; every other build gets this one.
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

export const IS_DEMO = false;

export const DEMO_USERS: readonly DemoPerson[] = [];
