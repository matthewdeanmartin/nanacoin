// The shell. Decides which of the three states the app is in - unprovisioned,
// logged out, or running - and renders the frame around the routed page.

import { Component, computed, inject, signal } from '@angular/core';
import { NavigationEnd, Router, RouterOutlet } from '@angular/router';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';

import { ApiBase } from './api/api-base';
import { ApiError } from './api/nanacoin.service';
import { candidates, Discovery } from './api/discovery';
import { IS_DEMO } from './demo/demo';
import { Log } from './api/log';
import { ConnectForm } from './pages/connect-form';
import { LoginForm } from './pages/login-form';
import { LogsPage } from './pages/logs';
import { Session } from './api/session';
import { SetupForm } from './pages/setup-form';
import { DialogHost } from './ui/dialog-host';
import { ToastList } from './ui/toast-list';
import { Toasts } from './ui/toasts';
import { SiteMenu } from './ui/site-menu';
import { KeyboardHelp } from './ui/keyboard-help';

type Phase = 'loading' | 'connect' | 'setup' | 'login' | 'app' | 'logs';

@Component({
  selector: 'app-root',
  imports: [
    RouterOutlet,
    SiteMenu,
    KeyboardHelp,
    ConnectForm,
    LoginForm,
    LogsPage,
    SetupForm,
    ToastList,
    DialogHost,
  ],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App {
  protected readonly publicPage = signal(false);
  protected readonly aboutPage = signal(false);
  private readonly router = inject(Router);
  protected readonly session = inject(Session);
  protected readonly apiBase = inject(ApiBase);
  private readonly toasts = inject(Toasts);
  private readonly log = inject(Log);
  private readonly discovery = inject(Discovery);
  private readonly discoveryLifetime = new AbortController();

  protected readonly isDemo = IS_DEMO;
  protected readonly plainHttp = computed(() =>
    location.protocol !== 'https:' || new URL(this.apiBase.current(), location.href).protocol !== 'https:');

  protected readonly phase = signal<Phase>('loading');

  /** Why the connect screen is showing, when an error put it there. */
  protected readonly connectReason = signal('');

  /**
   * True while the login screen is being used to ADD an account rather than
   * to sign in from scratch. Changes what that screen says and gives it a way
   * back, since the two look identical otherwise.
   */
  protected readonly addingAccount = signal(false);

  /**
   * Whether to offer the logs link on the connect screen.
   *
   * Deliberately more permissive than session.logsAvailable(): the connect
   * screen is shown precisely when the server could not be reached, and a
   * status that never arrived says nothing about whether logs exist. Offering
   * a link that might 404 beats withholding the one diagnostic that works
   * when login does not - which is the whole reason it is on this screen.
   */
  protected readonly logsOfferable = computed(
    () => this.session.status() === null || this.session.logsAvailable(),
  );

  protected readonly household = computed(
    () => this.session.status()?.household ?? 'NanaCoin',
  );

  constructor() {
    this.router.events.pipe(takeUntilDestroyed()).subscribe(event => {
      if (event instanceof NavigationEnd) {
        const path = event.urlAfterRedirects.split('?')[0];
        this.aboutPage.set(path === '/about');
        this.publicPage.set(['/about', '/recipes', '/ledger', '/diagnostics'].includes(path));
        requestAnimationFrame(() => document.getElementById('main-content')?.focus());
      }
    });
    // Recorded once at startup, because it silently changes what the platform
    // will do. On an insecure origin crypto.subtle does not exist, and the
    // login has to hash its own PKCE challenge - which worked on localhost
    // (a secure context by exemption) and threw on the board, where the same
    // code runs at an IP or an mDNS name.
    this.log.info('boot', 'page context', {
      origin: location.origin,
      secureContext: window.isSecureContext,
      webCrypto: typeof crypto !== 'undefined' && !!crypto.subtle,
    });
    void this.boot();
  }

  protected async boot(allowDiscovery = true): Promise<void> {
    this.phase.set('loading');
    this.log.info('boot', 'starting', { api: this.apiBase.description() });
    try {
      const status = await this.session.loadStatus();
      this.log.info('boot', 'found NanaCoin', {
        household: status.household,
        provisioned: status.provisioned,
        users: status.users,
      });
      if (!status.provisioned) {
        this.phase.set('setup');
        return;
      }
      const restored = await this.session.restore();
      this.phase.set(restored ? 'app' : 'login');
      this.log.info('boot', `showing the ${restored ? 'app' : 'login'} screen`);
    } catch (e) {
      // A default or remembered HTTP endpoint may now run Rust over HTTPS.
      // Prove a candidate using only public status before changing ApiBase.
      if (allowDiscovery && !this.isDemo && (!(e instanceof ApiError) || e.isNetwork || e.code === 'not_nanacoin')) {
        const meta = document.querySelector('meta[name="nanacoin-api"]')?.getAttribute('content') ?? '';
        const found = await this.discovery.find(candidates(this.apiBase.current(), meta),
          this.discoveryLifetime.signal, () => {});
        if (this.discoveryLifetime.signal.aborted) return;
        if (found) {
          this.apiBase.set(found);
          await this.boot(false);
          return;
        }
      }
      // Being unable to reach NanaCoin is not a dead end: the address is
      // something the user can supply, so ask for it rather than showing a
      // gateway error they can do nothing about.
      this.log.error('boot', 'could not reach NanaCoin; asking for an address', {
        api: this.apiBase.current(),
        error: e instanceof ApiError ? { status: e.status, code: e.code } : String(e),
      });
      this.connectReason.set(
        e instanceof ApiError ? e.message : 'Could not reach NanaCoin.',
      );
      this.phase.set('connect');
    }
  }

  ngOnDestroy(): void { this.discoveryLifetime.abort(); }

  /** Called once the connect screen has proved an address answers. */
  protected onConnected(): void {
    this.connectReason.set('');
    void this.boot();
  }

  /** Lets someone who is logged in point the app at a different NanaCoin. */
  protected changeServer(): void {
    this.connectReason.set('');
    this.phase.set('connect');
  }

  /**
   * Shows the logs without a session. The failure most worth diagnosing is the
   * one that stops you logging in, so the logs cannot be behind the login.
   */
  protected showLogs(): void {
    this.phase.set('logs');
  }

  /** Back from the standalone logs view to wherever the app belongs. */
  protected leaveLogs(): void {
    void this.boot();
  }

  protected onProvisioned(): void {
    this.toasts.ok('Household created. Log in to continue.');
    void this.boot();
  }

  protected onLoggedIn(): void {
    this.addingAccount.set(false);
    this.phase.set('app');
  }

  /**
   * Leaves the active account, staying in the app if another remains.
   *
   * Session.logout has already loaded whichever account it fell back to, so
   * the only decision here is which screen to show.
   */
  protected async logout(): Promise<void> {
    await this.session.logout();
    this.phase.set(this.session.signedIn() ? 'app' : 'login');
  }

  /** Ends every session at once, for leaving a shared computer. */
  protected async logoutAll(): Promise<void> {
    await this.session.logoutAll();
    this.phase.set('login');
  }

  /**
   * Shows the login form again to add a second account without leaving the
   * first. onLoggedIn returns to the app, and the new account is active.
   *
   * addingAccount is what makes this legible: without it the login screen is
   * indistinguishable from having been signed out, there is no way back if
   * you opened it by mistake, and the one thing a parent wants to do here -
   * hold their own account and Nana's at once - looks like it is not
   * supported.
   */
  protected addAccount(): void {
    this.addingAccount.set(true);
    this.phase.set('login');
  }

  /** Abandons adding an account and returns to the one already signed in. */
  protected cancelAddAccount(): void {
    this.addingAccount.set(false);
    this.phase.set('app');
  }

  /** Switches to another signed-in account, or to login if its token died. */
  protected async switchTo(userId: string): Promise<void> {
    try {
      await this.session.switchTo(userId);
      this.phase.set('app');
    } catch (e) {
      this.toasts.fromError(e);
      // switchTo's 401 has already dropped that account. Whatever remains
      // active is where the user lands; with nothing left, that is login.
      this.phase.set(this.session.signedIn() ? 'app' : 'login');
    }
  }

}
