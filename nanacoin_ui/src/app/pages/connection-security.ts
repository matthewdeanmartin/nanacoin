import { Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ApiBase } from '../api/api-base';
import { NanacoinService } from '../api/nanacoin.service';

@Component({
  selector: 'app-connection-security',
  imports: [FormsModule],
  template: `
    <section class="card">
      <h2>Household connection security</h2>
      <p>Easy mode lets the family use http://nanacoin.local without installing anything.
        HTTP exposes passwords and transactions to someone able to intercept your network traffic.
        HTTPS protects a connection, but an HTTP account can still be stolen while Easy mode is on.</p>
      <p><a href="http://nanacoin.local/trust" target="_blank" rel="noopener noreferrer">Set up trusted HTTPS on each device</a>
        · <a href="https://nanacoin.local/?api=">Open the secure site</a></p>
      <button type="button" title="Read whether this household currently permits unencrypted HTTP" (click)="check()" [disabled]="busy()">Check household mode</button>
      @if (mode(); as current) {
        <p>{{ current.https_only ? 'Secure mode: HTTP app and API are disabled.' : 'Easy mode: HTTP and HTTPS are available.' }}</p>
        @if (current.supported && !current.https_only) {
          <p>Prepare and test HTTPS on every family device first. Nana can then require it for everyone.
            All sessions will end. Economy reset does not turn HTTP back on; USB recovery is required.</p>
          @if (!secure()) { <p>Open this page over trusted HTTPS to enable Secure mode.</p> }
          <label><input type="checkbox" [(ngModel)]="ready"> Every device is ready; sign everyone out and disable HTTP.</label>
          <button type="button" title="Disable HTTP access for every household member after confirmation" (click)="enable()" [disabled]="!ready || !secure() || busy()">Require HTTPS for the household</button>
        }
      }
      <p role="status">{{ message() }}</p>
    </section>`,
})
export class ConnectionSecurity {
  private readonly api = inject(NanacoinService);
  private readonly base = inject(ApiBase);
  readonly secure = computed(() => location.protocol === 'https:' &&
    new URL(this.base.current(), location.href).protocol === 'https:');
  readonly mode = signal<{ https_only: boolean; supported: boolean } | null>(null);
  readonly busy = signal(false);
  readonly message = signal('');
  ready = false;

  async check(): Promise<void> {
    this.busy.set(true);
    try { this.mode.set(await this.api.transport()); this.message.set(''); }
    catch { this.message.set('This server could not report its mode. Older firmware may not support this feature.'); }
    finally { this.busy.set(false); }
  }

  async enable(): Promise<void> {
    if (!this.ready || !this.secure() || this.busy()) return;
    this.busy.set(true);
    try {
      await this.api.requireHttps();
      location.reload();
    } catch {
      this.message.set('Could not confirm the switch. Reopen HTTPS and sign in to check the mode before retrying.');
    } finally { this.busy.set(false); }
  }
}
