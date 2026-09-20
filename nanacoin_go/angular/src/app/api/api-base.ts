import { Injectable, signal } from '@angular/core';

/** Where to remember the chosen API base between visits. */
const STORAGE_KEY = 'nanacoin.apiBase';

/**
 * Which NanaCoin this page talks to.
 *
 * A service with a signal rather than a constant, because the address is
 * something a household member has to be able to change from the UI: the board
 * gets its address by DHCP, so it moves, and this site may be served from
 * somewhere entirely different (a static host, or another machine on the LAN).
 *
 * Changing it does not require a reload - the HTTP client reads current()
 * per request.
 */
@Injectable({ providedIn: 'root' })
export class ApiBase {
  /** The base URL every request is built on, e.g. '/api/v1'. */
  readonly current = signal<string>(initial());

  /** True when pointed at something other than this page's own origin. */
  readonly isRemote = signal<boolean>(initial() !== sameOrigin());

  /**
   * Points the app at a different NanaCoin and remembers the choice.
   *
   * Accepts what someone would actually type: `192.168.1.158`,
   * `http://192.168.1.158`, or a full `.../api/v1`.
   */
  set(raw: string): string {
    // The same-origin value is a relative path, so it must not go through
    // normalise() - that would prepend a scheme and produce 'http:///api/v1'.
    const trimmed = raw.trim();
    const base = trimmed === sameOrigin() ? sameOrigin() : normalise(trimmed) || sameOrigin();
    this.current.set(base);
    this.isRemote.set(base !== sameOrigin());
    try {
      if (base === sameOrigin()) localStorage.removeItem(STORAGE_KEY);
      else localStorage.setItem(STORAGE_KEY, base);
    } catch {
      // Not remembering it only costs a re-entry next visit.
    }
    return base;
  }

  /** Goes back to this page's own origin - the dev proxy, or a self-hosted copy. */
  reset(): void {
    this.set(sameOrigin());
  }

  /**
   * The host, for showing in the UI: the bare host rather than the scheme and
   * the /api/v1 suffix, neither of which the user typed.
   *
   * Same-origin reads as this page's own host, because that is literally where
   * the request went - saying "this site" would read oddly in a sentence like
   * "could not reach NanaCoin at ...".
   */
  label(): string {
    const base = this.current();
    if (base === sameOrigin()) return location.host;
    try {
      return new URL(base).host;
    } catch {
      return base;
    }
  }

  /** A short description of where requests go, for the footer. */
  description(): string {
    return this.isRemote() ? this.label() : `${location.host} (this site)`;
  }
}

/**
 * The starting value, in order of precedence:
 *
 *  1. `?api=` in the URL, which also overwrites the remembered value. Still
 *     supported because it is the quickest thing to paste into a chat or a
 *     terminal, but it is no longer the only way in - see the connect screen.
 *  2. A previously remembered choice.
 *  3. `<meta name="nanacoin-api">`, for a static deployment baked with a known
 *     address.
 *  4. This page's own origin, which is the `ng serve` proxy in development.
 */
function initial(): string {
  const fromQuery = new URLSearchParams(location.search).get('api');
  if (fromQuery !== null) {
    const base = normalise(fromQuery);
    try {
      if (base) localStorage.setItem(STORAGE_KEY, base);
      else localStorage.removeItem(STORAGE_KEY);
    } catch {
      // The query string still applies to this page load regardless.
    }
    return base || sameOrigin();
  }

  try {
    const remembered = localStorage.getItem(STORAGE_KEY);
    if (remembered) return remembered;
  } catch {
    // Fall through to the meta tag.
  }

  const meta = document.querySelector('meta[name="nanacoin-api"]')?.getAttribute('content');
  if (meta) return normalise(meta);

  return sameOrigin();
}

function sameOrigin(): string {
  return '/api/v1';
}

export function normalise(raw: string): string {
  let v = raw.trim().replace(/\/+$/, '');
  if (!v) return '';
  if (!/^https?:\/\//i.test(v)) v = `http://${v}`;
  if (!/\/api\/v\d+$/.test(v)) v = `${v}/api/v1`;
  return v;
}
