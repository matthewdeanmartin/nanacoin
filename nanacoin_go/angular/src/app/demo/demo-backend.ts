// The demo's server, wired in as an HTTP interceptor.
//
// # Why an interceptor rather than a fake service
//
// Swapping NanacoinService for a mock would demo the mock. This way every
// line of the real client runs unchanged - the same PKCE exchange, the same
// idempotency keys, the same error mapping, the same signals - and only the
// wire is different. If the demo works, the client works.
//
// Requests never leave the page. There is no server, no network, and nothing
// to deploy behind the static site.

import { HttpErrorResponse, HttpEvent, HttpHandlerFn, HttpRequest, HttpResponse } from '@angular/common/http';
import { Observable, delay, of, throwError } from 'rxjs';

import { EconomicKind, EconomicUnit } from '../api/models';
import { DemoError, DemoLedger } from './ledger';
import { seed } from './seed';

/** The one ledger this tab is showing. */
export const demoLedger = new DemoLedger();

/** Tokens handed out by the fake auth endpoints, mapped to user ids. */
const sessions = new Map<string, string>();

/** Authorisation codes, between /auth/authorize and /auth/token. */
const codes = new Map<string, string>();

let seeded = false;

/**
 * A small delay on every response.
 *
 * Not to imitate the board's latency - the point of the demo is that it is
 * pleasant - but because an API that answers in zero milliseconds makes
 * loading states, disabled buttons and spinners untestable and unshowable.
 * 120ms is enough to see them work and short enough not to be noticed.
 */
const LATENCY_MS = 120;

function quantityMilli(value: unknown): number {
  const [whole, fraction = ''] = String(value).split('.');
  return Number(whole) * 1000 + Number(fraction.padEnd(3, '0'));
}

export function demoBackend(
  req: HttpRequest<unknown>,
  next: HttpHandlerFn,
): Observable<HttpEvent<unknown>> {
  if (!req.url.includes('/api/v1')) return next(req);

  if (!seeded) {
    seed(demoLedger);
    seeded = true;
  }

  try {
    const body = handle(req);
    return of(new HttpResponse({ status: 200, body })).pipe(delay(LATENCY_MS));
  } catch (e) {
    const err =
      e instanceof DemoError
        ? new HttpErrorResponse({
            status: e.status,
            error: { error: e.code, message: e.message },
            url: req.url,
          })
        : new HttpErrorResponse({
            status: 500,
            error: { error: 'demo_error', message: String(e) },
            url: req.url,
          });
    return throwError(() => err).pipe(delay(LATENCY_MS));
  }
}

function handle(req: HttpRequest<unknown>): unknown {
  // Everything after /api/v1, without the query string.
  const path = req.url.split('/api/v1')[1].split('?')[0];
  const query = new URLSearchParams(req.url.split('?')[1] ?? '');
  const body = (req.body ?? {}) as Record<string, string & number>;
  const method = req.method;

  // --- open endpoints ---

  if (path === '/status') return demoLedger.status();
  if (path === '/public/ledger' && method === 'GET') return demoLedger.ledger(100);
  if (path === '/transport' && method === 'GET') return { https_only: false, supported: false };

  if (path === '/provision' && method === 'POST') {
    return demoLedger.provision(
      body['username'],
      body['display_name'],
      body['password'],
      body['household_name'],
    );
  }

  /**
   * The demo signs in by name alone.
   *
   * The real exchange is Authorization Code + PKCE against a PBKDF2 verifier.
   * Here the password is ignored: the site offers a list of household members
   * to pick from, and asking a visitor to type a password printed beside the
   * box would be a puzzle rather than a demonstration. The client still runs
   * its half of PKCE unchanged - it generates a verifier, hashes it, and
   * exchanges a code - so what is being shown is still the real flow.
   */
  if (path === '/auth/authorize' && method === 'POST') {
    const user = demoLedger.userByName(body['username']);
    if (!user) {
      throw new DemoError(401, 'invalid_credentials', 'No such member in this household.');
    }
    if (user.status !== 'ACTIVE') {
      throw new DemoError(403, 'disabled', 'That account is disabled.');
    }
    const code = `code-${Math.random().toString(36).slice(2)}`;
    codes.set(code, user.id);
    return { code };
  }

  if (path === '/auth/token' && method === 'POST') {
    const userId = codes.get(body['code']);
    codes.delete(body['code']);
    if (!userId) throw new DemoError(400, 'invalid_grant', 'That login has expired.');
    const user = demoLedger.userById(userId)!;
    const token = `demo-${Math.random().toString(36).slice(2)}`;
    sessions.set(token, userId);
    return {
      access_token: token,
      token_type: 'Bearer',
      expires_in: 8 * 3600,
      user: demoLedger.view(user, user),
    };
  }

  if (path === '/auth/logout' && method === 'POST') {
    const token = bearer(req);
    if (token) sessions.delete(token);
    return null;
  }

  // --- everything below needs a session ---

  const me = current(req);
  if (path === '/nickles' && method === 'POST') return demoLedger.createNickle(me, Number(body['amount']), (req.body as { fresh_money?: boolean }).fresh_money === true);
  if (path === '/nickles/redeem' && method === 'POST') return demoLedger.redeemNickle(me, String(body['token'] ?? ''));

  if (path === '/me') return demoLedger.view(me, me);

  if (path === '/users' && method === 'GET') {
    return { users: demoLedger.allUsers(me) };
  }

  if (path === '/users' && method === 'POST') {
    if (me.role !== 'nana') throw new DemoError(403, 'forbidden', 'Only Nana can add members.');
    const created = demoLedger.addUser(
      body['username'],
      body['display_name'],
      body['password'] || 'demo',
      'user',
    );
    if (body['grant']) {
      demoLedger.issue(me, created.account, 20, 'Starting allocation');
    }
    return demoLedger.view(created, me);
  }

  if (path.startsWith('/users/') && method === 'PATCH') {
    return demoLedger.setUserStatus(me, lastSegment(path), body['status'] as 'ACTIVE' | 'DISABLED');
  }

  if (path.startsWith('/accounts/')) {
    const rest = path.slice('/accounts/'.length);
    if (rest.endsWith('/transactions')) {
      const account = rest.slice(0, -'/transactions'.length);
      return demoLedger.history(account, Number(query.get('limit') ?? 50));
    }
  }

  if (path === '/transactions' && method === 'GET') {
    return demoLedger.ledger(Number(query.get('limit') ?? 100));
  }

  if (path.startsWith('/transactions/') && path.endsWith('/reverse') && method === 'POST') {
    const id = path.slice('/transactions/'.length, -'/reverse'.length);
    return demoLedger.reverse(me, id, body['reason']);
  }

  if (path === '/transfers' && method === 'POST') {
    return demoLedger.transfer(
      me,
      body['to'],
      Number(body['amount']),
      body['memo'] ?? '',
      body['economic_kind'] ? {
        economic_kind: body['economic_kind'] as EconomicKind,
        thing: body['thing'] as string | undefined,
        quantity_milli: quantityMilli(body['quantity']),
        unit: body['unit'] as EconomicUnit,
      } : undefined,
    );
  }

  if (path === '/admin/issue' && method === 'POST') {
    return demoLedger.issue(me, body['to'], Number(body['amount']), body['reason'] ?? '');
  }

  if (path === '/admin/retire' && method === 'POST') {
    return demoLedger.retire(me, body['from'], Number(body['amount']), body['reason'] ?? '');
  }

  if (path === '/admin/issue-usd' && method === 'POST') {
    return demoLedger.issueUSD(me, body['to'], Number(body['cents']), body['reason'] ?? '');
  }

  if (path === '/quotes' && method === 'GET') return { quotes: demoLedger.allQuotes() };
  if (path === '/quotes' && method === 'POST') {
    return demoLedger.postQuote(me, body['side'] as 'ASK' | 'BID', Number(body['cents_per_coin']), Number(body['coins']));
  }
  if (path.startsWith('/quotes/') && path.endsWith('/take') && method === 'POST') {
    return demoLedger.takeQuote(me, path.slice('/quotes/'.length, -'/take'.length));
  }
  if (path.startsWith('/quotes/') && path.endsWith('/cancel') && method === 'POST') {
    return demoLedger.cancelQuote(me, path.slice('/quotes/'.length, -'/cancel'.length));
  }

  if (path === '/admin/config') {
    return { household_name: demoLedger.household, initial_grant: 20, currency: 'NanaCoin' };
  }

  // --- marketplace ---

  if (path === '/listings' && method === 'GET') {
    return { listings: demoLedger.allListings() };
  }
  if (path === '/things' && method === 'GET') {
    return { things: demoLedger.allThings() };
  }

  if (path === '/listings' && method === 'POST') {
    return demoLedger.createListing(me, {
      title: body['title'],
      description: body['description'] ?? '',
      price: Number(body['price']),
      side: body['side'] as 'SELL' | 'BUY' | undefined,
      kind: body['kind'],
      currency: body['currency'],
      minor_units: body['minor_units'] ? Number(body['minor_units']) : undefined,
      economic_kind: body['economic_kind'] as EconomicKind | undefined,
      thing: body['thing'] as string | undefined,
      quantity_milli: body['quantity'] ? quantityMilli(body['quantity']) : undefined,
      unit: body['unit'] as EconomicUnit | undefined,
      standard: Boolean(body['standard']),
    });
  }

  if (path.startsWith('/listings/')) {
    const rest = path.slice('/listings/'.length);

    if (rest.endsWith('/purchase') && method === 'POST') {
      return demoLedger.purchase(me, rest.slice(0, -'/purchase'.length));
    }
    if (rest.endsWith('/cancel') && method === 'POST') {
      return demoLedger.cancelListing(me, rest.slice(0, -'/cancel'.length));
    }
    if (rest.endsWith('/offers')) {
      const listingId = rest.slice(0, -'/offers'.length);
      if (method === 'POST') {
        return demoLedger.makeOffer(me, listingId, Number(body['amount']), body['message'] ?? '');
      }
      return { offers: demoLedger.offersFor(me).filter((o) => o.listing === listingId) };
    }
    if (method === 'GET') return demoLedger.listingById(rest);
  }

  // --- offers ---

  if (path === '/offers' && method === 'GET') {
    return { offers: demoLedger.offersFor(me) };
  }

  if (path.startsWith('/offers/')) {
    const rest = path.slice('/offers/'.length);
    if (rest.endsWith('/accept') && method === 'POST') {
      return demoLedger.acceptOffer(me, rest.slice(0, -'/accept'.length));
    }
    if (rest.endsWith('/decline') && method === 'POST') {
      return demoLedger.declineOffer(me, rest.slice(0, -'/decline'.length));
    }
    if (rest.endsWith('/withdraw') && method === 'POST') {
      return demoLedger.withdrawOffer(me, rest.slice(0, -'/withdraw'.length));
    }
  }

  // The real board has no event log in this build either, so the client
  // hiding those tabs is the honest behaviour rather than a demo shortcut.
  if (path === '/logs' || path === '/diag') {
    throw new DemoError(404, 'not_found', 'This NanaCoin serves no logs.');
  }

  throw new DemoError(404, 'not_found', `No route for ${method} ${path}.`);
}

function bearer(req: HttpRequest<unknown>): string | null {
  const header = req.headers.get('Authorization');
  return header?.startsWith('Bearer ') ? header.slice(7) : null;
}

function current(req: HttpRequest<unknown>) {
  const token = bearer(req);
  const userId = token ? sessions.get(token) : undefined;
  const user = userId ? demoLedger.userById(userId) : undefined;
  if (!user) throw new DemoError(401, 'no_session', 'Please log in again.');
  return user;
}

function lastSegment(path: string): string {
  return path.slice(path.lastIndexOf('/') + 1);
}
