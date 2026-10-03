import { Component, Injectable, computed, effect, inject, signal } from '@angular/core';
import { NavigationEnd, Router } from '@angular/router';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { Session } from '../api/session';
import { IS_DEMO } from '../demo/demo';
import { Toasts } from './toasts';

export interface TourStep {
  path: string;
  title: string;
  description: string;
}
export function tourSteps(
  nana: boolean,
  demo: boolean,
  diagnostics = false,
  logs = false,
): TourStep[] {
  const steps: TourStep[] = [
    {
      path: '/market',
      title: 'The household market',
      description:
        'Browse what people want and what they can sell. Open a listing to buy or propose a price.',
    },
    {
      path: '/list',
      title: 'Buy, sell, hire',
      description:
        'Offer goods or work, or post a want-ad. Classifying the payment keeps household accounting useful.',
    },
    {
      path: '/send',
      title: 'Send money or a message',
      description:
        'Pay another member, arrange allowances, or send a zero-value message. Optional copies go to Mastodon or the kitchen screen.',
    },
    {
      path: '/messages',
      title: 'Your mailbox',
      description:
        'Read messages and review offers, loan requests, and payments that need your attention.',
    },
    {
      path: '/offers',
      title: 'Make a deal',
      description: 'Review proposed prices and terms. Money moves only when an offer is accepted.',
    },
    {
      path: '/gifts',
      title: 'Gift requests',
      description:
        'Ask for help with a household goal or contribute coins to somebody else’s request.',
    },
    {
      path: '/history',
      title: 'My Account',
      description:
        'Follow your transactions, promises, loans, and purchases that still need to be fulfilled.',
    },
    {
      path: '/wealth',
      title: 'My wealth',
      description:
        'See your assets, debts, income, spending, and savings. Set a realistic cash-out rate to estimate dollar values.',
    },
    {
      path: '/loans',
      title: 'Loans and credit',
      description:
        'Apply for a loan, lend to a member, or review principal, interest, and scheduled repayments.',
    },
    {
      path: '/lotto',
      title: 'Lotto and savings',
      description:
        'Explore household pools, including savings pools that return principal with interest. Read the terms before joining.',
    },
    {
      path: '/forex',
      title: 'Dollar exchange',
      description:
        'Browse bids and asks to exchange coins for recorded dollars. A marginal trade price is no guarantee that all your coins can sell at that price.',
    },
    {
      path: '/art',
      title: 'Digital art',
      description:
        'Discover household artwork and collections. Art ownership lives alongside the household marketplace.',
    },
    {
      path: '/people',
      title: 'Meet the household',
      description: 'Explore members and their public profiles, activity, and contributions.',
    },
    {
      path: '/economy',
      title: 'The whole economy',
      description:
        'See household money supply, production, employment, prices, and exchange rates over the retained history.',
    },
    {
      path: '/central-bank',
      title: 'Nana as central bank',
      description:
        'See how issuance, reserves, exchange offers, lending, and lotto promises fit together.',
    },
    {
      path: '/ledger',
      title: 'The notebook',
      description: 'The shared ledger makes every recorded movement of coins auditable.',
    },
  ];
  if (nana)
    steps.push(
      {
        path: '/good-deeds',
        title: 'Reward good deeds',
        description: 'As Nana, set standing rewards for helpful work around the household.',
      },
      {
        path: '/nana?tab=members',
        title: 'Nana’s household desk',
        description:
          'Manage people and bots. Nana can also issue coins, correct transactions, and manage household settings.',
      },
      {
        path: '/nana?tab=money',
        title: 'Issue and record household money',
        description: 'Nana creates household coins and records physical dollars here. Issuance changes the money supply; it is separate from earning income.',
      },
      {
        path: '/nana?tab=market-desk',
        title: 'Nana’s market desk',
        description:
          'Plan exchange offers, lending, and lotto series. Review the batch before taking any action.',
      },
    );
  if (demo)
    steps.push({
      path: '/nickles',
      title: 'Nana-nickles',
      description:
        'The demo also shows bearer vouchers: give someone a code or QR code that they can redeem once.',
    });
  steps.push(
    {
      path: '/invite',
      title: 'Invite someone',
      description: 'Share your household’s address and help another member connect.',
    },
    {
      path: '/settings?tab=settings-appearance',
      title: 'Make it yours',
      description: 'Choose an appearance, manage API keys, and connect your own Mastodon account.',
    },
    {
      path: '/clientlog',
      title: 'Browser log',
      description:
        'Errors and warnings stay here after a popup disappears. Copy the log when reporting a problem.',
    },
  );
  if (demo || diagnostics)
    steps.push({
      path: '/diagnostics',
      title: demo ? 'Demo browser health' : 'Board health',
      description: demo
        ? 'This demo runs in the browser. Its health page describes this tab rather than a connected board.'
        : 'Inspect the connected board’s health and connection diagnostics.',
    });
  if (!demo && logs)
    steps.push({
      path: '/logs',
      title: 'Server log',
      description:
        'Read the board’s recorded events alongside the browser log when diagnosing a connection or request.',
    });
  steps.push(
    { path: '/about', title: 'Household money, no crypto', description: 'See how a household currency differs from cryptocurrency: people, shared rules, and a notebook instead of mining.' },
    { path: '/recipes', title: 'Lemon bars', description: 'Household money needs something worth buying. Here is a recipe, with metric and US measurements.' },
    { path: '/docs', title: 'Run your own household bank', description: 'Find setup and connection instructions for your own Nanacoin computer.' },
  );
  steps.push({
    path: '/specification',
    title: 'The Nanacoin 2026 specification',
    description:
      'Read the rules behind the household currency. The Help menu also links to setup docs, the demo, and lemon bars.',
  });
  return steps;
}

@Injectable({ providedIn: 'root' })
export class Tour {
  private readonly session = inject(Session);
  private readonly router = inject(Router);
  private readonly toasts = inject(Toasts);
  readonly index = signal<number | null>(null);
  readonly busy = signal(false);
  readonly steps = computed(() =>
    tourSteps(
      this.session.isNana(),
      IS_DEMO,
      this.session.diagAvailable(),
      this.session.logsAvailable(),
    ),
  );
  readonly step = computed(() => (this.index() === null ? null : this.steps()[this.index()!]));
  private account: string | undefined;
  private generation = 0;
  constructor() {
    this.router.events.pipe(takeUntilDestroyed()).subscribe(event => {
      if (!(event instanceof NavigationEnd) || this.index() === null || this.busy()) return;
      const steps = this.steps();
      let index = steps.findIndex(step => step.path === event.urlAfterRedirects);
      if (index < 0) index = steps.findIndex(step => step.path.split('?')[0] === event.urlAfterRedirects.split('?')[0]);
      this.index.set(index < 0 ? null : index);
    });
    effect(() => {
      const account = this.session.me()?.id;
      if (account !== this.account) {
        this.stop();
        this.account = account;
      }
    });
  }
  async start(): Promise<void> {
    if (!this.session.signedIn()) {
      this.toasts.error('Sign in to start the tour for your account.');
      return;
    }
    this.account = this.session.me()?.id;
    await this.go(0);
  }
  stop(): void {
    this.generation++;
    this.index.set(null);
  }
  async go(index: number): Promise<void> {
    if (this.busy()) return;
    if (index >= this.steps().length) {
      this.stop();
      return;
    }
    const step = this.steps()[index];
    if (!step) return;
    const account = this.session.me()?.id;
    const generation = this.generation;
    this.busy.set(true);
    try {
      // The welcome page and the first tour step both use /market. Angular
      // skips navigation to the current URL; that still means we are ready.
      const arrived = this.router.url === step.path || await this.router.navigateByUrl(step.path);
      if (arrived && generation === this.generation && account === this.session.me()?.id)
        this.index.set(index);
    } catch (error) {
      this.toasts.fromError(error);
    } finally {
      this.busy.set(false);
    }
  }
}

@Component({
  selector: 'app-tour',
  template: `@if (tour.step(); as step) {
    <aside class="tour" aria-label="Guided tour">
      <div aria-live="polite">
        <p class="muted small">
          {{ demo ? 'Demo tour' : 'Household tour' }} · {{ session.isNana() ? 'Nana' : 'Member' }} ·
          {{ tour.index()! + 1 }} / {{ tour.steps().length }}
        </p>
        <div class="tour-progress" role="progressbar" aria-label="Tour progress"
          [attr.aria-valuenow]="tour.index()! + 1" aria-valuemin="1" [attr.aria-valuemax]="tour.steps().length">
          <span [style.width.%]="(tour.index()! + 1) / tour.steps().length * 100"></span>
        </div>
        <h2>{{ step.title }}</h2>
        <p>{{ step.description }}</p>
      </div>
      <div class="tour-actions">
        <button
          class="btn btn--quiet"
          [disabled]="tour.index() === 0 || tour.busy()"
          (click)="tour.go(tour.index()! - 1)"
        >
          Previous
        </button>
        <button class="btn" [disabled]="tour.busy()" (click)="tour.go(tour.index()! + 1)">
          {{ tour.index()! === tour.steps().length - 1 ? 'Finish tour' : 'Next' }}
        </button>
        <button class="btn btn--quiet" (click)="tour.stop()">Exit tour</button>
      </div>
    </aside>
  }`,
  styles: `
    .tour {
      position: fixed;
      right: 1rem;
      bottom: 1rem;
      z-index: 15;
      width: min(360px, calc(100vw - 2rem));
      box-sizing: border-box;
      max-height: calc(100dvh - 2rem);
      overflow-y: auto;
      padding: 1rem 1.3rem;
      border: 2px solid var(--accent);
      border-radius: 8px;
      background: var(--surface);
      box-shadow: 0 8px 28px rgb(0 0 0 / 22%);
      color: var(--ink);
    }
    .tour-progress { height: 4px; background: var(--line); border-radius: 4px; overflow: hidden; margin: .6rem 0; }
    .tour-progress span { display: block; height: 100%; background: var(--accent); }
    .tour h2 {
      font-size: 1.2rem;
      margin: 0.2rem 0;
    }
    .tour p {
      margin: 0.3rem 0;
    }
    .tour-actions {
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
      margin-top: 1rem;
    }
    @media (max-width: 750px) {
      .tour {
        max-height: 60dvh;
      }
    }
  `,
})
export class TourPanel {
  readonly tour = inject(Tour);
  readonly session = inject(Session);
  readonly demo = IS_DEMO;
}
