// Nana's market-maker desk: set up standing dollar offers, loan offers and a
// series of lottos in one reviewed batch. All client side - each step is an
// ordinary API call the server already validates. The plan itself lives in
// nana/market-maker-plan.ts.

import { Component, computed, inject, input, resource, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { LottoKind } from '../api/models';
import { InputError, Money, MoneyPipe, parseMoney } from '../api/money';
import { Log } from '../api/log';
import { ApiError, NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session, reloadOnLedgerChange } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { Toasts } from '../ui/toasts';
import { LoanTier, Plan, PriceTier, Step, dollars, planMarket } from '../nana/market-maker-plan';

interface TierRow { share: string; price: string }
interface LoanRow { label: string; share: string; rate: string }
type StepState = 'waiting' | 'running' | 'done' | 'failed';
interface RunRow { step: Step; key: string; state: StepState; error?: string }

@Component({
  selector: 'app-market-maker',
  imports: [FormsModule, MoneyPipe, RouterLink],
  template: `
    @if (embedded()) { <h2>Nana's market desk</h2> }
    @else { <p><a routerLink="/nana">← Household</a></p><h1>Nana's market desk</h1> }
    <p class="lede">Put up standing offers in one batch: buy and sell coins for dollars, lend at set rates, and run a year of lottos. Sizes scale to the coins in circulation, now {{ circulation() | nc }} NC.</p>
    @if (!session.isNana()) { <p class="muted">This is Nana's screen.</p> }
    @else {
      <div class="stats" aria-label="Nana's position">
        <div class="stat"><span>{{ session.balance() | nc }}</span><span class="stat__label">Nana's coins</span></div>
        <div class="stat"><span>{{ money$(session.me()?.usd_cents ?? 0) }}</span><span class="stat__label">Nana's dollars</span></div>
        <div class="stat"><span>{{ book.value()?.quotes?.length ?? '…' }}</span><span class="stat__label">open exchange offers</span></div>
      </div>

      <section class="panel">
        <h2><label><input type="checkbox" [(ngModel)]="useForex" name="useForex" /> Dollar exchange ladder</label></h2>
        <p class="muted small">Each rung is one offer, all or nothing. A share is a percent of all coins in circulation. Small cash-outs get a good price; if everyone tried to cash out, the later rungs pay almost nothing.</p>
        @if (useForex) {
          <h3>Nana buys coins (children cash out)</h3>
          <table class="rungs"><thead><tr><th>Share of coins (%)</th><th>Nana pays per coin ($)</th><th></th></tr></thead><tbody>
            @for (row of bids; track $index) {
              <tr><td><input [(ngModel)]="row.share" [name]="'bs' + $index" inputmode="decimal" /></td><td><input [(ngModel)]="row.price" [name]="'bp' + $index" inputmode="decimal" /></td>
              <td><button type="button" class="btn btn--quiet btn--small" (click)="bids.splice($index, 1)">Remove</button></td></tr>
            }
          </tbody></table>
          @if (bids.length < 4) { <button type="button" class="btn btn--quiet btn--small" (click)="bids.push({ share: '10', price: '0.10' })">Add a rung</button> }
          <h3>Nana sells coins (for real dollars)</h3>
          <table class="rungs"><thead><tr><th>Share of coins (%)</th><th>Nana charges per coin ($)</th><th></th></tr></thead><tbody>
            @for (row of asks; track $index) {
              <tr><td><input [(ngModel)]="row.share" [name]="'as' + $index" inputmode="decimal" /></td><td><input [(ngModel)]="row.price" [name]="'ap' + $index" inputmode="decimal" /></td>
              <td><button type="button" class="btn btn--quiet btn--small" (click)="asks.splice($index, 1)">Remove</button></td></tr>
            }
          </tbody></table>
          @if (asks.length < 4) { <button type="button" class="btn btn--quiet btn--small" (click)="asks.push({ share: '10', price: '5.00' })">Add a rung</button> }
          <label>Offers last (days, 0 = until withdrawn) <input [(ngModel)]="forexDays" name="forexDays" inputmode="numeric" /></label>
          <label><input type="checkbox" [(ngModel)]="replaceQuotes" name="replaceQuotes" /> Withdraw Nana's old exchange offers first</label>
          <label><input type="checkbox" [(ngModel)]="topUpUsd" name="topUpUsd" /> If Nana is short of dollars, record the real dollars she is holding</label>
          <label><input type="checkbox" [(ngModel)]="topUpCoins" name="topUpCoins" /> If Nana is short of coins, issue new coins to her</label>
        }
      </section>

      <section class="panel">
        <h2><label><input type="checkbox" [(ngModel)]="useLoans" name="useLoans" /> Standing loan offers</label></h2>
        <p class="muted small">Every chosen person gets one offer per loan size. Nana lends coins she already has. Interest is per 30 days; principal comes back in about four payments.</p>
        @if (useLoans) {
          <fieldset><legend>Who can borrow</legend>
            @for (u of borrowerChoices(); track u.account) {
              <label><input type="checkbox" [checked]="picked().has(u.account)" (change)="toggle(u.account)" /> {{ u.display_name }}</label>
            } @empty { <p class="muted">No one else in the household yet.</p> }
          </fieldset>
          <table class="rungs"><thead><tr><th>Name</th><th>Size (% of coins)</th><th>Interest per 30 days (%)</th><th></th></tr></thead><tbody>
            @for (row of loanRows; track $index) {
              <tr><td><input [(ngModel)]="row.label" [name]="'ll' + $index" maxlength="20" /></td><td><input [(ngModel)]="row.share" [name]="'ls' + $index" inputmode="decimal" /></td><td><input [(ngModel)]="row.rate" [name]="'lr' + $index" inputmode="decimal" /></td>
              <td><button type="button" class="btn btn--quiet btn--small" (click)="loanRows.splice($index, 1)">Remove</button></td></tr>
            }
          </tbody></table>
          @if (loanRows.length < 4) { <button type="button" class="btn btn--quiet btn--small" (click)="loanRows.push({ label: 'Extra', share: '5', rate: '5' })">Add a size</button> }
          <label>Payments every <select [(ngModel)]="paymentDays" name="paymentDays"><option [ngValue]="1">day</option><option [ngValue]="7">week</option><option [ngValue]="30">30 days</option></select></label>
          <label><input type="checkbox" [(ngModel)]="replaceLoans" name="replaceLoans" /> Withdraw Nana's old loan offers that nobody accepted</label>
        }
      </section>

      <section class="panel">
        <h2><label><input type="checkbox" [(ngModel)]="useLotto" name="useLotto" /> A series of lottos</label></h2>
        @if (useLotto) {
          <label>Name <input [(ngModel)]="lottoTitle" name="lottoTitle" maxlength="50" /></label>
          <label>How many (one a month) <input [(ngModel)]="lottoCount" name="lottoCount" inputmode="numeric" /></label>
          <label>First one closes <input type="datetime-local" [(ngModel)]="lottoFirst" name="lottoFirst" /></label>
          <label>Kind <select [(ngModel)]="lottoKind" name="lottoKind"><option value="SIMPLE">Simple: winner takes the pool</option><option value="DELAYED">Delayed: winner takes pool + interest</option><option value="SAVINGS">Savings: everyone gets their money back</option></select></label>
          <label>Ticket price in NC <input [(ngModel)]="lottoPrice" name="lottoPrice" inputmode="decimal" /></label>
          @if (lottoKind !== 'SIMPLE') { <label>Interest for 30 days (%) <input [(ngModel)]="lottoRate" name="lottoRate" inputmode="decimal" /></label> }
        }
      </section>

      <section class="panel" aria-labelledby="preview-title">
        <h2 id="preview-title">Check the batch</h2>
        @if (planned(); as p) {
          @if (p.input) { <p class="form-problems" role="alert">{{ p.input }}</p> }
          @else if (p.plan; as plan) {
            <ul class="totals">
              @if (useForex) { <li>Dollars Nana pays if every buy offer is taken: <strong>{{ money$(plan.totals.bidCents) }}</strong> (she has {{ money$(session.me()?.usd_cents ?? 0) }})</li>
                <li>Coins Nana sells if every sell offer is taken: <strong>{{ plan.totals.askCoins | nc }} NC</strong></li> }
              @if (useLoans) { <li>Coins lent if every loan offer is accepted: <strong>{{ plan.totals.loanCoins | nc }} NC</strong> (Nana has {{ session.balance() | nc }})</li> }
              @if (useLotto) { <li>Lottos: <strong>{{ plan.totals.lottos }}</strong></li> }
            </ul>
            @for (e of plan.errors; track e) { <p class="form-problems">{{ e }}</p> }
            @for (w of plan.warnings; track w) { <p class="warning small">{{ w }}</p> }
            <ol class="steps">
              @for (s of plan.steps; track $index) { <li>{{ s.label }}{{ detail(s) }}</li> }
            </ol>
            <button class="btn" type="button" [disabled]="running() || plan.errors.length > 0 || !plan.steps.length || !book.hasValue()" (click)="start(plan)">Do these {{ plan.steps.length }} things</button>
          }
        }
      </section>

      @if (run().length) {
        <section class="panel" aria-live="polite">
          <h2>Progress</h2>
          <ol class="steps">
            @for (r of run(); track $index) {
              <li [class.failed]="r.state === 'failed'">{{ mark(r.state) }} {{ r.step.label }}@if (r.error) { — {{ r.error }} }</li>
            }
          </ol>
          @if (!running() && failedAt() >= 0) {
            <p>Stopped at step {{ failedAt() + 1 }}. Steps before it are done. Trying again will not repeat them.</p>
            <button class="btn" type="button" (click)="resume()">Try the rest again</button>
            <button class="btn btn--quiet" type="button" (click)="run.set([])">Forget this batch</button>
          }
        </section>
      }
    }
  `,
  styles: [`.panel {margin-block:1rem;} .panel h2 label {display:inline-flex;gap:.5rem;align-items:center;} label {display:block;margin-block:.4rem;}
    .rungs {border-collapse:collapse;} .rungs td,.rungs th {padding:.25rem .5rem .25rem 0;text-align:left;} .rungs input {width:7rem;max-width:100%;}
    .steps li.failed {color:var(--warn-ink);font-weight:600;} .form-problems {color:var(--warn-ink);background:var(--warn-bg);padding:.5rem .75rem;border-radius:8px;}
    .warning {color:var(--warn-ink);} fieldset {border:0;padding:0;margin:0 0 .5rem;}
    @media (max-width: 480px) { .rungs input {width:5rem;} }`],
})
export class MarketMakerPage {
  readonly embedded=input(false);
  protected readonly session = inject(Session);
  private readonly api = inject(NanacoinService);
  private readonly money = inject(Money);
  private readonly dialogs = inject(Dialogs);
  private readonly toasts = inject(Toasts);
  private readonly log = inject(Log);

  protected readonly book = resource({
    params: () => this.session.me()?.account,
    loader: async () => {
      const [quotes, loans, lottos] = await Promise.all([this.api.quotes(), this.api.loans(), this.api.lottos()]);
      return { quotes: quotes.quotes.filter((q) => q.status === 'OPEN'), allQuotes: quotes.quotes, loans: loans.loans, lottos: lottos.lottos };
    },
  });
  private readonly followLedger = reloadOnLedgerChange(this.book);

  protected readonly circulation = computed(() => this.session.status()?.circulation ?? 0);

  // The example ladder from the spec: at 1,000 coins it buys 10 at $1.00,
  // 50 at $0.75, 100 at $0.25 and 1,000 at $0.01.
  protected useForex = true;
  protected bids: TierRow[] = [{ share: '1', price: '1.00' }, { share: '5', price: '0.75' }, { share: '10', price: '0.25' }, { share: '100', price: '0.01' }];
  protected asks: TierRow[] = [{ share: '1', price: '1.50' }, { share: '5', price: '2.00' }, { share: '10', price: '3.00' }];
  protected forexDays = '30';
  protected replaceQuotes = true;
  protected topUpUsd = false;
  protected topUpCoins = false;

  protected useLoans = false;
  protected loanRows: LoanRow[] = [{ label: 'Small', share: '1', rate: '1' }, { label: 'Medium', share: '5', rate: '5' }, { label: 'Large', share: '15', rate: '15' }];
  protected paymentDays: 1 | 7 | 30 = 7;
  protected replaceLoans = true;
  protected readonly picked = signal(new Set<string>());
  protected readonly borrowerChoices = computed(() => this.session.household().filter((u) => u.status === 'ACTIVE' && u.role !== 'nana'));

  protected useLotto = false;
  protected lottoTitle = 'Monthly lotto';
  protected lottoCount = '12';
  protected lottoFirst = firstOfNextMonth();
  protected lottoKind: LottoKind = 'SAVINGS';
  protected lottoPrice = '1';
  protected lottoRate = '2';

  protected readonly run = signal<RunRow[]>([]);
  protected readonly running = signal(false);
  protected readonly failedAt = computed(() => this.run().findIndex((r) => r.state === 'failed'));

  protected money$ = dollars;
  protected toggle(account: string): void {
    this.picked.update((s) => { const next = new Set(s); if (next.has(account)) next.delete(account); else next.add(account); return next; });
  }
  protected mark(state: StepState): string { return state === 'done' ? '✓' : state === 'failed' ? '✗' : state === 'running' ? '…' : '·'; }
  protected detail(s: Step): string {
    switch (s.kind) {
      case 'quote': return `: ${this.money.format(s.coins)} NC for ${dollars(Number((BigInt(s.coins) * BigInt(s.cents_per_coin)) / BigInt(10 ** this.money.decimals())))}`;
      case 'loan': return `: ${this.money.format(s.input.amount)} NC at ${s.input.rate_bps / 100}% per 30 days`;
      case 'issue': return `: ${this.money.format(s.amount)} NC`;
      case 'lotto': return `, ticket ${this.money.format(s.terms.ticket_price)} NC`;
      default: return '';
    }
  }

  /**
   * Recomputed on every change detection pass from the plain form fields. The
   * plan is cheap and pure; a mistake in a box shows as a message, not a throw.
   */
  protected planned(): { plan?: Plan; input?: string } {
    const data = this.book.value(), me = this.session.me();
    if (!data || !me) return { input: 'Loading the current offers…' };
    try {
      const pct = (text: string, what: string) => { try { return parseMoney(text.trim().replace(/\s*%$/, ''), 2, this.money.locale); } catch { throw new InputError(`${what}: type a percent like 5 or 2.5. You typed "${text}".`); } };
      const usd = (text: string) => { try { return parseMoney(text.trim().replace(/^\$/, ''), 2, this.money.locale); } catch { throw new InputError(`Prices are dollars and cents, like 0.75. You typed "${text}".`); } };
      const tiers = (rows: TierRow[]): PriceTier[] => rows.map((r) => ({ shareBps: pct(r.share, 'Share of coins'), cents: usd(r.price) }));
      const days = Number(this.forexDays || '0');
      if (!Number.isInteger(days) || days < 0 || days > 365) throw new InputError('Offers last 0 to 365 days.');
      const now = Math.floor(Date.now() / 1000);
      const loanTiers: LoanTier[] = this.loanRows.map((r) => ({ label: r.label.trim() || 'Loan', shareBps: pct(r.share, 'Loan size'), rateBps: pct(r.rate, 'Interest') }));
      const plan = planMarket({
        circulation: this.circulation(),
        scale: 10 ** this.money.decimals(),
        nana: { account: me.account, balance: me.balance ?? 0, usdCents: me.usd_cents ?? 0 },
        now,
        quotes: data.quotes,
        loans: data.loans,
        lottos: data.lottos,
        forex: this.useForex ? { bids: tiers(this.bids), asks: tiers(this.asks), expiresAt: days ? now + days * 86_400 : 0, replace: this.replaceQuotes, topUpUsd: this.topUpUsd, topUpCoins: this.topUpCoins } : undefined,
        lending: this.useLoans ? { borrowers: this.borrowerChoices().filter((u) => this.picked().has(u.account)).map((u) => ({ account: u.account, name: u.display_name })), tiers: loanTiers, paymentDays: this.paymentDays, replace: this.replaceLoans } : undefined,
        lotto: this.useLotto ? { count: Number(this.lottoCount), firstClose: Math.floor(new Date(this.lottoFirst).getTime() / 1000), kind: this.lottoKind, ticketPrice: this.money.parse(this.lottoPrice), rateBps: this.lottoKind === 'SIMPLE' ? 0 : pct(this.lottoRate, 'Lotto interest'), title: this.lottoTitle } : undefined,
      });
      return { plan };
    } catch (e) {
      return { input: e instanceof Error ? e.message : String(e) };
    }
  }

  protected async start(plan: Plan): Promise<void> {
    const answer = await this.dialogs.confirm({
      title: `Do these ${plan.steps.length} things?`,
      message: 'Nana\'s offers go up one at a time. If one fails, the batch stops and you can try the rest again without repeating anything.',
      detail: plan.warnings,
      confirmLabel: 'Start',
    });
    if (answer === null) return;
    this.log.info('market', 'starting a market-maker batch', { steps: plan.steps.length, kinds: plan.steps.map((s) => s.kind) });
    this.run.set(plan.steps.map((step) => ({ step, key: newIdempotencyKey(), state: 'waiting' })));
    await this.resume();
  }

  /** Runs every step not yet done, in order, reusing each step's key. */
  protected async resume(): Promise<void> {
    if (this.running()) return;
    this.running.set(true);
    try {
      for (let i = 0; i < this.run().length; i++) {
        const row = this.run()[i];
        if (row.state === 'done') continue;
        this.patch(i, { state: 'running', error: undefined });
        try {
          await this.execute(row.step, row.key);
          this.patch(i, { state: 'done' });
        } catch (e) {
          const message = e instanceof ApiError || e instanceof InputError ? e.message : 'Something went wrong.';
          this.log.warn('market', 'a batch step failed', { step: row.step.label, error: String(e) });
          this.patch(i, { state: 'failed', error: message });
          return;
        }
      }
      this.toasts.ok('All of Nana\'s offers are up.');
    } finally {
      this.running.set(false);
      await this.session.refresh().catch(() => undefined);
      this.book.reload();
    }
  }

  private async execute(step: Step, key: string): Promise<void> {
    const me = this.session.me()!.account;
    switch (step.kind) {
      case 'cancel-quote':
        try { await this.api.cancelQuote(step.id); } catch (e) { if (!(e instanceof ApiError && e.status === 409)) throw e; }
        return;
      case 'close-loan':
        try { await this.api.closeLoan(step.id, key); } catch (e) { if (!(e instanceof ApiError && e.status === 409)) throw e; }
        return;
      case 'issue': await this.api.issue(me, step.amount, 'Backing for Nana\'s market offers', key); return;
      case 'issue-usd': await this.api.issueUSD(me, step.cents, 'Real dollars Nana holds for cash-outs', key); return;
      case 'quote': await this.api.postQuote({ side: step.side, cents_per_coin: step.cents_per_coin, coins: step.coins, expires_at: step.expires_at }, key); return;
      case 'loan': await this.api.offerLoan(step.input, key); return;
      case 'lotto': await this.api.createLotto(step.terms, key); return;
    }
  }

  private patch(i: number, change: Partial<RunRow>): void {
    this.run.update((rows) => rows.map((r, j) => (j === i ? { ...r, ...change } : r)));
  }
}

/** The first of next month at 6 pm local, as a datetime-local value. */
function firstOfNextMonth(): string {
  const now = new Date();
  const d = new Date(now.getFullYear(), now.getMonth() + 1, 1, 18, 0);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T18:00`;
}
