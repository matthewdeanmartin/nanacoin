import { Injectable } from '@angular/core';

// Documented DHCP addresses of these development boards; never scan a subnet.
export const KNOWN_HOSTS = ['nanacoin.local', 'nanacoin-rs.local', 'nanacoin-api.local', '192.168.1.158', '192.168.1.157'];

export function candidates(current: string, meta = ''): string[] {
  const urls: string[] = [];
  for (const raw of [current, meta, ...KNOWN_HOSTS]) {
    if (!raw || raw.startsWith('/')) continue;
    try {
      const url = new URL(/^https?:\/\//i.test(raw) ? raw : `https://${raw}`);
      if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) continue;
      // Always test TLS first, including when a saved address used plain HTTP.
      for (const protocol of ['https:', 'http:']) {
        url.protocol = protocol;
        url.pathname = '/api/v1';
        url.search = '';
        url.hash = '';
        urls.push(url.href);
      }
    } catch { /* An invalid saved address must not prevent known-host discovery. */ }
  }
  return [...new Set(urls)];
}

export function isNanacoinStatus(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false;
  const status = value as Record<string, unknown>;
  return typeof status['provisioned'] === 'boolean' && typeof status['household'] === 'string'
    && typeof status['users'] === 'number' && typeof status['ledger_balanced'] === 'boolean';
}

@Injectable({ providedIn: 'root' })
export class Discovery {
  /** Sequential probes bound the load on a board with only four TLS sockets.
   * Never send a stored bearer token to an unverified discovery candidate. */
  async find(bases: string[], signal: AbortSignal, progress: (base: string) => void): Promise<string | null> {
    for (const base of bases) {
      // A certificate/network failure on the secure site must never downgrade.
      if (location.protocol === 'https:' && new URL(base).protocol !== 'https:') continue;
      if (signal.aborted) return null;
      progress(base);
      const controller = new AbortController();
      const abort = () => controller.abort();
      signal.addEventListener('abort', abort, { once: true });
      const timeout = setTimeout(abort, 3000);
      try {
        const response = await fetch(`${base}/status`, {
          signal: controller.signal, cache: 'no-store', credentials: 'omit', redirect: 'error',
        });
        if (response.ok && isNanacoinStatus(await response.json()) && !signal.aborted) return base;
      } catch { /* Includes unreachable, TLS, mixed-content and CORS failures. */ }
      finally {
        clearTimeout(timeout);
        signal.removeEventListener('abort', abort);
      }
    }
    return null;
  }
}
