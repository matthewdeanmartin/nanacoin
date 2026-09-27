import { Injectable, signal } from '@angular/core';

const KEY = 'nanacoin:trusted-art-hosts';

/**
 * Sites this viewer has chosen to load art from without asking. Kept in this
 * browser only: it is a per-viewer privacy choice, not household data.
 */
@Injectable({ providedIn: 'root' })
export class TrustedArtHosts {
  readonly hosts = signal<readonly string[]>(read());
  has(host: string): boolean { return this.hosts().includes(host); }
  trust(host: string): void { this.save([...new Set([...this.hosts(), host])].sort()); }
  forget(host: string): void { this.save(this.hosts().filter((h) => h !== host)); }
  private save(hosts: string[]): void {
    this.hosts.set(hosts);
    try { localStorage.setItem(KEY, JSON.stringify(hosts)); } catch { /* still remembered for this visit */ }
  }
}

function read(): string[] {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? '[]') as unknown;
    return Array.isArray(value) ? value.filter((h): h is string => typeof h === 'string' && /^[a-z0-9.-]+(:\d+)?$/i.test(h)) : [];
  } catch { return []; }
}
