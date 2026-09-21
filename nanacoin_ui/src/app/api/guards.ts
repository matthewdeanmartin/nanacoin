// Route guards.
//
// The server is the authority on every one of these rules and enforces them
// regardless - an ordinary member who POSTs to /users gets a 403 whatever this
// file says. These exist so the UI does not *offer* what the server will
// refuse: showing someone a "disable Nana" button that 403s is worse than not
// showing it, because it reads as a broken app rather than as a rule.

import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';

import { Session } from './session';

/** Nana-only routes. Everyone else is sent back to the market. */
export const nanaOnly: CanActivateFn = () => {
  const session = inject(Session);
  const router = inject(Router);

  if (session.isNana()) return true;

  // Redirect rather than blocking, so a bookmarked or hand-typed /nana lands
  // somewhere usable instead of on a blank screen.
  return router.createUrlTree(['/market']);
};

/**
 * Routes that need a session at all.
 *
 * The shell already refuses to render the app phase without one, so this is a
 * second line for a direct navigation that arrives before the shell has
 * settled.
 */
export const signedIn: CanActivateFn = () => {
  const session = inject(Session);
  const router = inject(Router);

  if (session.signedIn()) return true;
  return router.createUrlTree(['/market']);
};
