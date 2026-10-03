// My Settings: how this browser shows NanaCoin (Appearance), and the two ways
// other programs act for you - an API key, and a connected Mastodon account.

import { Component, computed, inject, resource, signal } from '@angular/core';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute, Router } from '@angular/router';

import { KeyScope, KeyState, NewApiKey } from '../api/models';
import { ApiBase } from '../api/api-base';
import { Mastodon, SUGGESTED_SERVERS } from '../api/mastodon';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { IS_DEMO } from '../demo/demo';
import { Dialogs } from '../ui/dialog';
import { SectionTabs } from '../ui/section-tabs';
import { THEMES, Theme, ThemeId } from '../ui/theme';
import { Toasts } from '../ui/toasts';

const APPEARANCE = { id: 'settings-appearance', label: 'Appearance' };
const API_KEYS = { id: 'settings-api-keys', label: 'API Keys' };
const MASTODON = { id: 'settings-mastodon', label: 'Mastodon' };

@Component({
  selector: 'app-settings',
  imports: [FormsModule, SectionTabs],
  template: `
    <h1>My Settings</h1>
    <p class="lede">How NanaCoin looks on this device, and how other programs can act for you.</p>

    <app-section-tabs [tabs]="tabs()" [selected]="tab()" (selectedChange)="select($event)" label="Settings sections" prefix="settings" />

    <section id="settings-appearance" role="tabpanel" aria-labelledby="settings-tab-settings-appearance" [hidden]="tab() !== 'settings-appearance'">
      <fieldset class="theme-picker">
        <legend>Colour theme</legend>
        @for (t of themes; track t.id) {
          <label class="theme-picker__choice">
            <input type="radio" name="theme" [value]="t.id" [checked]="theme.current() === t.id" (change)="setTheme(t.id)" />
            <span class="theme-picker__swatch" aria-hidden="true">
              @for (c of t.swatch; track $index) { <i [style.background]="c"></i> }
            </span>
            <span class="theme-picker__text"><strong>{{ t.label }}</strong><span>{{ t.hint }}</span></span>
          </label>
        }
      </fieldset>
      <p class="muted small">Saved in this browser only. Other devices, and other people signed in here, keep their own choice.</p>
    </section>

    @if (session.signedIn()) {
    <section id="settings-api-keys" role="tabpanel" aria-labelledby="settings-tab-settings-api-keys" [hidden]="tab() !== 'settings-api-keys'">
      <p>An API key lets a script or another program use NanaCoin as you. It cannot change your password or make new keys.</p>
      <p class="muted small">You may have one key of each kind. Making a new one cancels the old one of that kind; changing your password cancels both.</p>

      @if (newKey(); as key) {
        <div class="secret-reveal" role="status">
          <p><strong>Copy your new {{ key.scope === 'read' ? 'read-only ' : '' }}key now.</strong> NanaCoin keeps only a fingerprint of it and cannot show it again.</p>
          <div class="secret-reveal__value">
            <input readonly [value]="key.api_key" aria-label="Your new API key" (focus)="selectAll($event)" />
            <button class="btn" type="button" (click)="copy(key.api_key)">Copy</button>
          </div>
          <p class="muted small">Send it as a bearer token, for example:</p>
          <pre><code>curl -H "Authorization: Bearer {{ key.api_key }}" {{ apiUrl() }}/me</code></pre>
          <button class="btn btn--quiet" type="button" (click)="newKey.set(null)">I have saved it</button>
        </div>
      }

      @if (keyStatus.isLoading()) {
        <p class="muted">Loading…</p>
      } @else if (keyStatus.error()) {
        <p class="muted">API keys are not supported by this server.</p>
      } @else {
        @for (k of kinds; track k.scope) {
          @let state = keyState(k.scope);
          @if (state) {
            <h3>{{ k.label }}</h3>
            <p class="muted small">{{ k.what }}</p>
            @if (state.active) {
              <p>You have one{{ state.created_at ? ', made ' + when(state.created_at) : '' }}.</p>
              <div class="voucher-actions">
                <button class="btn" type="button" [disabled]="busy()" (click)="makeKey(true, k.scope)">Replace with a new key…</button>
                <button class="btn btn--danger" type="button" [disabled]="busy()" (click)="revokeKey(k.scope)">Revoke key</button>
              </div>
            } @else {
              <p class="empty">You have none.</p>
              <button class="btn" type="button" [disabled]="busy()" (click)="makeKey(false, k.scope)">Make a {{ k.noun }}…</button>
            }
          }
        }
      }
      @if (isDemo) {
        <p class="muted small">In this demo a key is only pretend: nothing outside this tab accepts it.</p>
      }
    </section>

    <section id="settings-mastodon" role="tabpanel" aria-labelledby="settings-tab-settings-mastodon" [hidden]="tab() !== 'settings-mastodon'">
      <p>Connect your Mastodon account to send a direct-message copy when you send money or a message. NanaCoin records the payment either way; Mastodon is only the extra copy.</p>
      @if (mastodon.connected()) {
        <p>Connected as <strong>{{ mastodon.account() }}</strong> on {{ host(mastodon.server()) }}.</p>
        <p class="muted small">The access token stays in this browser. Connect again on each device you use.</p>
        <button class="btn btn--quiet" type="button" (click)="disconnect()">Disconnect</button>
      } @else if (isDemo) {
        <button class="btn" type="button" (click)="connect()">Connect</button>
      } @else {
        <form (ngSubmit)="connect()">
          <label>
            Your Mastodon server
            <input name="server" [(ngModel)]="server" list="mastodon-servers" required autocomplete="url"
                   autocapitalize="off" spellcheck="false" placeholder="mastomini.local or mastodon.social" />
            <datalist id="mastodon-servers">
              @for (s of suggestions; track s) { <option [value]="s"></option> }
            </datalist>
          </label>
          <p class="muted small">
            You will sign in on that server and approve NanaCoin, then come back here. Uses PKCE: no password is shared with NanaCoin.
          </p>
          <button class="btn" type="submit" [disabled]="connecting()">{{ connecting() ? 'Connecting…' : 'Connect' }}</button>
        </form>
      }
      <p class="muted small">
        @if (session.me()?.mastodon_id) {
          Household members send you copies at {{ session.me()?.mastodon_id }}.
        } @else {
          Nobody can send you Mastodon copies yet. Connecting records your handle so they can.
        }
      </p>
    </section>
    }
  `,
})
export class SettingsPage {
  protected readonly session = inject(Session);
  protected readonly theme = inject(Theme);
  protected readonly mastodon = inject(Mastodon);
  private readonly api = inject(NanacoinService);
  private readonly apiBase = inject(ApiBase);
  private readonly dialogs = inject(Dialogs);
  private readonly toasts = inject(Toasts);
  private readonly router = inject(Router);

  protected readonly isDemo = IS_DEMO;
  protected readonly themes = THEMES;
  protected readonly suggestions = SUGGESTED_SERVERS;

  /** Signed out, only Appearance applies: the others act on an account. */
  protected readonly tabs = computed(() => (this.session.signedIn() ? [APPEARANCE, API_KEYS, MASTODON] : [APPEARANCE]));
  protected readonly tab = signal(APPEARANCE.id);

  protected readonly keyStatus = resource({
    params: () => this.session.me()?.id,
    loader: () => this.api.apiKeyStatus(),
  });
  /** The key just made. Held only until the member dismisses it or leaves. */
  protected readonly newKey = signal<NewApiKey | null>(null);
  protected readonly kinds: { scope: KeyScope; label: string; noun: string; what: string }[] = [
    { scope: 'full', label: 'Full key', noun: 'full key',
      what: 'Does everything you are allowed to do: pay, list, trade.' },
    { scope: 'read', label: 'Read-only key', noun: 'read-only key',
      what: 'Only reads: for a news bot or a dashboard. It can never move money.' },
  ];

  /** One scope's key, or null when the server has no keys of that scope. */
  protected keyState(scope: KeyScope): KeyState | null {
    const status = this.keyStatus.value();
    if (!status) return null;
    if (scope === 'full') return status.full ?? { active: status.active, created_at: status.created_at };
    return status.read ?? null;
  }
  protected readonly busy = signal(false);

  protected server: string;
  protected readonly connecting = signal(false);

  constructor() {
    this.server = this.mastodon.lastServer();
    inject(ActivatedRoute).queryParamMap.pipe(takeUntilDestroyed()).subscribe((q) => {
      const tab = q.get('tab');
      this.tab.set(this.tabs().some((t) => t.id === tab) ? tab! : APPEARANCE.id);
      const code = q.get('mastodon_code');
      const state = q.get('mastodon_state');
      if (code && state) {
        void this.router.navigate([], { queryParams: { tab: MASTODON.id }, replaceUrl: true });
        void this.finishMastodon(code, state);
      }
    });
  }

  protected select(tab: string): void {
    this.tab.set(tab);
    void this.router.navigate([], { queryParams: { tab }, replaceUrl: true });
  }

  protected setTheme(id: ThemeId): void {
    this.theme.set(id);
  }

  protected apiUrl(): string {
    return new URL(this.apiBase.current(), location.href).href.replace(/\/$/, '');
  }

  protected async makeKey(replacing: boolean, scope: KeyScope = 'full'): Promise<void> {
    if (this.busy()) return;
    const noun = scope === 'read' ? 'read-only key' : 'API key';
    const password = await this.dialogs.password({
      title: replacing ? `Replace your ${noun}` : `Make a ${noun}`,
      message: 'Enter your password to confirm it is you.',
      detail: replacing ? ['Your current key stops working immediately.'] : undefined,
      placeholder: 'Your PIN or password',
      confirmLabel: replacing ? 'Replace key' : 'Make key',
      required: true,
    });
    if (password === null) return;
    this.busy.set(true);
    try {
      const made = await this.api.createApiKey(password, scope);
      this.newKey.set({ ...made, scope });
      this.keyStatus.set(await this.api.apiKeyStatus());
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.busy.set(false);
    }
  }

  protected async revokeKey(scope: KeyScope = 'full'): Promise<void> {
    if (this.busy()) return;
    const ok = await this.dialogs.confirm({
      title: scope === 'read' ? 'Revoke your read-only key?' : 'Revoke your API key?',
      message: 'Anything using it stops working immediately.',
      confirmLabel: 'Revoke key',
      danger: true,
    });
    if (ok === null) return;
    this.busy.set(true);
    try {
      this.keyStatus.set(await this.api.revokeApiKey(scope));
      this.newKey.set(null);
      this.toasts.ok('API key revoked.');
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.busy.set(false);
    }
  }

  protected selectAll(event: Event): void {
    (event.target as HTMLInputElement).select();
  }

  protected async copy(text: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
      this.toasts.ok('Copied.');
    } catch {
      this.toasts.error('Could not copy. Select the key and copy it yourself.');
    }
  }

  protected when(unixSeconds: number): string {
    return new Date(unixSeconds * 1000).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
  }

  protected host(server: string | null): string {
    return server ? new URL(server).host : '';
  }

  protected async connect(): Promise<void> {
    if (this.connecting()) return;
    this.connecting.set(true);
    try {
      await this.mastodon.connect(this.server);
    } catch (e) {
      this.toasts.error(e instanceof Error ? e.message : 'Could not connect Mastodon.');
    } finally {
      this.connecting.set(false);
    }
  }

  protected disconnect(): void {
    this.mastodon.disconnect();
    this.toasts.ok('Mastodon disconnected from this browser.');
  }

  private async finishMastodon(code: string, state: string): Promise<void> {
    try {
      this.toasts.ok(`Connected Mastodon as ${await this.mastodon.finish(code, state)}.`);
    } catch (e) {
      this.toasts.error(e instanceof Error ? e.message : 'Could not finish Mastodon sign-in.');
    }
  }
}
