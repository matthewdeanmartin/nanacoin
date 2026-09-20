// "Which NanaCoin?" - the screen shown when the server cannot be reached.
//
// This exists because the board gets its address from DHCP, so the address
// changes, and a household member cannot be expected to know about query
// strings. Whoever opens the page has to be able to type in the number the
// board printed and get on with it.

import { Component, inject, input, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { ApiBase, normalise } from '../api/api-base';
import { NanacoinService } from '../api/nanacoin.service';
import { candidates, Discovery } from '../api/discovery';

@Component({
  selector: 'app-connect-form',
  imports: [FormsModule],
  template: `
    <div class="panel">
      <h1>Where is NanaCoin?</h1>

      @if (reason()) {
        <p class="connect__reason">{{ reason() }}</p>
      }

      <p class="muted">
        NanaCoin runs on your own machine or on the board. Enter its address —
        the board prints one when it starts up, like <code>192.168.1.158</code
        >.
      </p>

      <form (ngSubmit)="connect()">
        <label>
          Address
          <input
            name="address"
            [(ngModel)]="address"
            placeholder="192.168.1.158"
            autocomplete="off"
            autocapitalize="off"
            spellcheck="false"
            inputmode="url"
            [disabled]="busy()"
          />
        </label>

        <button class="btn" type="submit" [disabled]="busy()">
          {{ busy() ? 'Trying…' : 'Connect' }}
        </button>
      </form>

      <p>
        <button class="btn btn--quiet" type="button" (click)="search()" [disabled]="busy()">
          Search for NanaCoin (HTTP and HTTPS)
        </button>
        @if (searching()) {
          <button class="btn btn--quiet" type="button" (click)="cancelSearch()">Cancel search</button>
        }
      </p>
      <p class="muted small" role="status">{{ searchMessage() }}</p>
      <p class="muted small">
        Searches nanacoin-rs.local, nanacoin-api.local, saved addresses, and
        192.168.1.158 / 192.168.1.157. HTTPS needs a trusted certificate matching
        the address. An HTTPS page may block HTTP boards; open the web board's
        HTTP page to use TinyGo.
      </p>

      @if (preview()) {
        <p class="muted small">Will try <code>{{ preview() }}</code></p>
      }

      @if (diagnosis() === 'absent') {
        <p class="muted small">
          Nothing answered there. On the board, the first connection after a
          reboot can take up to a minute — waiting and trying again is often
          all it needs.
        </p>
      }

      @if (diagnosis() === 'cors') {
        <p class="connect__reason">
          NanaCoin is running at that address, but it is not letting this page
          read its answers.
        </p>
        <p class="muted small">
          It has to list this page's address, <code>{{ thisOrigin }}</code>, as
          an allowed origin — a browser blocks the response otherwise. On the
          desktop server add
          <code>-origins "{{ thisOrigin }}"</code>; on the board it is
          <code>defaultOrigins</code> in
          <code>cmd/nanacoin-esp32/main.go</code>, which needs a reflash.
        </p>
      }

      @if (diagnosis()) {
        <p class="muted small">
          @if (logsAvailable()) {
            <button class="btn btn--quiet btn--small" type="button" (click)="showLogs.emit()">
              Look at the server logs
            </button>
          }
        </p>
      }

      <details class="disclosure">
        <summary>It is running on this computer</summary>
        <p class="muted small">
          Then the site can use its own address. This is the usual setup during
          development, where <code>npm start</code> forwards to the Go server on
          port 8080.
        </p>
        <button class="btn btn--quiet" type="button" (click)="useThisSite()" [disabled]="busy()">
          Use this site's own server
        </button>
      </details>
    </div>
  `,
  styles: `
    .connect__reason {
      background: var(--warn-bg);
      color: var(--warn-ink);
      border-radius: 8px;
      padding: 0.6rem 0.75rem;
      margin: 0 0 1rem;
      font-size: 0.9rem;
      font-weight: 600;
    }
    code {
      background: var(--bg);
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 0.05rem 0.35rem;
    }
  `,
})
export class ConnectForm {
  private readonly discovery = inject(Discovery);
  private searchController: AbortController | null = null;
  protected readonly searching = signal(false);
  protected readonly searchMessage = signal('');
  private readonly apiBase = inject(ApiBase);
  private readonly api = inject(NanacoinService);

  /** Why the connect screen is showing, if it was an error that caused it. */
  readonly reason = input('');

  /** Emitted once something at the entered address actually answers. */
  readonly connected = output<void>();

  /**
   * Whether this server has server logs worth offering.
   *
   * False only when a reachable server reported that it was built without
   * them; an unreachable one leaves this true, since the link is most useful
   * exactly when nothing else works.
   */
  readonly logsAvailable = input(true);

  /** Asks the shell to show the logs page without logging in first. */
  readonly showLogs = output<void>();

  protected address = '';
  protected readonly busy = signal(false);

  /** Why the last attempt failed, once it has been worked out. */
  protected readonly diagnosis = signal<'' | 'absent' | 'cors'>('');

  /** This page's origin, which is what the server has to be told to allow. */
  protected readonly thisOrigin = location.origin;

  /** Shows what the typed text will be turned into, so it is never a surprise. */
  protected preview(): string {
    const v = this.address.trim();
    return v ? normalise(v) : '';
  }

  protected async search(): Promise<void> {
    if (this.busy()) return;
    const controller = new AbortController();
    this.searchController = controller;
    this.busy.set(true);
    this.searching.set(true);
    this.diagnosis.set('');
    try {
      const meta = document.querySelector('meta[name="nanacoin-api"]')?.getAttribute('content') ?? '';
      const base = await this.discovery.find(candidates(this.apiBase.current(), meta),
        controller.signal, (url) => this.searchMessage.set(`Trying ${url}…`));
      if (controller.signal.aborted) return;
      if (base) {
        this.apiBase.set(base);
        this.connected.emit();
      } else {
        this.searchMessage.set('No readable NanaCoin API found. Check power, Wi-Fi, certificate trust and browser network permissions, or enter its address above.');
      }
    } finally {
      this.busy.set(false);
      this.searching.set(false);
      this.searchController = null;
    }
  }

  protected cancelSearch(): void {
    this.searchController?.abort();
    this.searchMessage.set('Search cancelled.');
  }

  ngOnDestroy(): void { this.searchController?.abort(); }

  protected async connect(): Promise<void> {
    if (this.busy()) return;
    const typed = this.address.trim();
    if (!typed) return;

    await this.tryBase(() => this.apiBase.set(typed));
  }

  protected async useThisSite(): Promise<void> {
    if (this.busy()) return;
    await this.tryBase(() => this.apiBase.reset());
  }

  /**
   * Points the app somewhere and proves it before accepting it.
   *
   * /status needs no token and works before provisioning, so it is the right
   * probe: a success means there is a NanaCoin there, not merely that
   * something answered. On failure the previous address is put back, so a
   * failed attempt cannot leave the app pointed at nothing.
   */
  private async tryBase(apply: () => void): Promise<void> {
    const previous = this.apiBase.current();
    this.busy.set(true);
    this.diagnosis.set('');
    try {
      apply();
      await this.api.status();
      this.connected.emit();
    } catch {
      // Work out which kind of failure it was before restoring the old
      // address, since the probe has to run against the one just tried.
      // "Running but not letting this page read it" and "nothing there" need
      // completely different fixes, and the browser reports both the same way.
      const reachable = (await this.api.probe()) === 'reachable';
      this.diagnosis.set(reachable ? 'cors' : 'absent');
      this.apiBase.set(previous);
    } finally {
      this.busy.set(false);
    }
  }
}
