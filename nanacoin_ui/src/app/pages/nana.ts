import { Money, MoneyPipe } from '../api/money';
import { inject as moneyInject } from '@angular/core';
// Nana's page: the household, the full ledger, and the privileged actions.
//
// Every control here is also enforced server-side. Hiding this tab from
// ordinary users is a courtesy, not the security boundary.

import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { ConnectionSecurity } from './connection-security';
import { CurrencyReform } from './currency-reform';
import { Notebook } from '../ui/notebook';
import { IS_DEMO } from '../demo/demo';

import { Transaction } from '../api/models';
import { NanacoinService, newIdempotencyKey, StorageStatus } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { LiveSeeder, defaultSeed } from '../demo/seed-live';
import { Toasts } from '../ui/toasts';
import { SectionLink } from '../ui/section-link';

@Component({
  selector: 'app-nana',
  imports: [MoneyPipe, FormsModule, RouterLink, ConnectionSecurity, Notebook, CurrencyReform, SectionLink],
  templateUrl: './nana.html',
})
export class NanaPage {
  protected readonly money = moneyInject(Money);
  protected readonly isDemo = IS_DEMO;
  private readonly api = inject(NanacoinService);
  private readonly toasts = inject(Toasts);
  private readonly dialogs = inject(Dialogs);
  protected readonly seeder = inject(LiveSeeder);
  protected readonly session = inject(Session);

  protected readonly ledger = signal<Transaction[]>([]);
  protected readonly loadingLedger = signal(false);
  protected readonly resolvingLotto = signal(false);
  private lottoResolveKey: string | null = null;

  protected async resolveLottos(): Promise<void> {
    if (!this.isDemo || !this.session.isNana() || this.resolvingLotto()) return;
    this.resolvingLotto.set(true);
    try {
      const result = await this.api.resolveDemoLottos(this.lottoResolveKey ??= newIdempotencyKey());
      this.lottoResolveKey = null;
      await Promise.all([this.session.refresh(), this.loadLedger()]);
      this.toasts.ok(result.resolved ? `Resolved ${result.resolved} lottos. Prizes, returned savings, and interest have been paid.` : 'No pending lottos to resolve.');
    } catch (e) { this.toasts.fromError(e); }
    finally { this.resolvingLotto.set(false); }
  }
  protected readonly storage = signal<StorageStatus | null>(null);
  protected readonly storageBusy = signal(false);
  protected readonly storageError = signal('');

  // Add a member.
  protected newUsername = '';
  protected newDisplayName = '';
  protected newPassword = '';
  protected newMastodonId = '';
  protected grant = true;
  protected readonly adding = signal(false);

  // Issue coins.
  protected issueTo = '';
  protected issueAmount: string | number | null = null;
  protected issueReason = '';
  protected readonly issuing = signal(false);

  // Issue dollars. Kept separate from the coin form rather than adding a
  // currency dropdown to it: issuing dollars is a different act with a
  // different unit, and a dropdown that silently changes what "500" means is
  // exactly the kind of thing that gets someone issued $5 instead of 5 coins.
  protected usdTo = '';
  protected usdCents: number | null = null;
  protected usdReason = '';
  protected readonly issuingUsd = signal(false);

  protected readonly reversing = signal<string | null>(null);

  constructor() {
    void this.loadLedger();
    if (this.session.status()?.checkpoint_supported) void this.loadStorage();
  }

  protected async loadStorage(): Promise<void> {
    try { this.storage.set(await this.api.storage()); this.storageError.set(''); }
    catch (e) { this.storageError.set(e instanceof Error ? e.message : String(e)); }
  }

  protected async closeBooks(): Promise<void> {
    if (this.storageBusy() || this.seeder.running()) return;
    this.storageBusy.set(true);
    try {
      const current = await this.api.storage();
      const answer = await this.dialogs.confirm({ title: 'Close the current journal?',
        message: 'Save a durable checkpoint and reclaim the older journal records.',
        detail: ['Balances, accounts, open deals and recent displayed history are preserved.',
          'Older detailed journal records are retired; this is not an archival backup.'],
        confirmLabel: 'Close journal' });
      if (answer === null) return;
      this.storage.set(await this.api.checkpoint(current));
      await this.session.refresh();
      this.toasts.ok('Checkpoint saved. Journal space reclaimed.');
    } catch (e) { this.toasts.fromError(e); }
    finally { this.storageBusy.set(false); }
  }

  protected async resetEconomy(): Promise<void> {
    if (this.storageBusy() || this.seeder.running()) return;
    this.storageBusy.set(true);
    try {
      const current = await this.api.storage();
      const answer = await this.dialogs.prompt({ title: 'Reset the entire economy?',
        message: 'This removes every account, both currencies’ balances, listings, offers and history, including Nana’s account.',
        detail: ['Everyone will be signed out. The app returns to first-time household setup.',
          'This cannot be undone through the app. Type RESET ECONOMY to confirm.'],
        placeholder: 'RESET ECONOMY', required: true, danger: true, confirmLabel: 'Reset economy' });
      if (answer === null) return;
      if (answer !== 'RESET ECONOMY') { this.toasts.error('Confirmation must be exactly RESET ECONOMY.'); return; }
      await this.api.resetEconomy(current, answer);
      window.location.reload();
    } catch (e) { this.toasts.fromError(e); }
    finally { this.storageBusy.set(false); }
  }

  protected async loadLedger(): Promise<void> {
    this.loadingLedger.set(true);
    try {
      const page = await this.api.ledger(50);
      this.ledger.set(page.transactions);
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.loadingLedger.set(false);
    }
  }

  protected async addMember(): Promise<void> {
    if (this.adding()) return;
    this.adding.set(true);
    try {
      await this.api.createUser(
        this.newUsername.trim(),
        this.newDisplayName.trim(),
        this.newPassword,
        this.grant,
        this.newMastodonId.trim(),
      );
      this.newUsername = '';
      this.newDisplayName = '';
      this.newPassword = '';
      this.newMastodonId = '';
      this.toasts.ok('Member added.');
      await Promise.all([this.session.refresh(), this.loadLedger()]);
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.adding.set(false);
    }
  }

  protected async issue(): Promise<void> {
    if (this.issuing()) return;

    if (!this.issueTo) {
      this.toasts.error('Choose who the coins are for.');
      return;
    }
    let amount: number;
    try { amount = this.money.parse(this.issueAmount ?? ''); } catch (e) { this.toasts.fromError(e); return; }
    if (!Number.isInteger(amount) || amount <= 0) {
      this.toasts.error('Enter a positive amount in NC.');
      return;
    }

    this.issuing.set(true);
    try {
      await this.api.issue(this.issueTo, amount, this.issueReason.trim(), newIdempotencyKey());
      this.issueAmount = null;
      this.issueReason = '';
      this.toasts.ok('Issued.');
      await Promise.all([this.session.refresh(), this.loadLedger()]);
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.issuing.set(false);
    }
  }

  protected async issueDollars(): Promise<void> {
    if (this.issuingUsd()) return;

    if (!this.usdTo) {
      this.toasts.error('Choose who the dollars are for.');
      return;
    }
    const cents = Number(this.usdCents);
    if (!Number.isInteger(cents) || cents <= 0) {
      this.toasts.error('Enter a whole number of cents.');
      return;
    }

    this.issuingUsd.set(true);
    try {
      await this.api.issueUSD(this.usdTo, cents, this.usdReason.trim(), newIdempotencyKey());
      this.usdCents = null;
      this.usdReason = '';
      this.toasts.ok('Issued.');
      await Promise.all([this.session.refresh(), this.loadLedger()]);
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.issuingUsd.set(false);
    }
  }

  /**
   * Fills the household with a year of plausible history.
   *
   * Confirmed first, and the confirmation says what it will actually do: this
   * writes hundreds of real transactions, and on the board that is minutes of
   * work and a meaningful slice of the ledger's 365-record window.
   */
  protected async seedDemo(): Promise<void> {
    const opts = defaultSeed();
    const ok = await this.dialogs.confirm({
      title: 'Add a year of history?',
      message: 'Everything is written through the ordinary API, so it is all real.',
      detail: [
        `${opts.members} members, each with a starting float`,
        `${opts.listings} listings, some of them want-ads`,
        `about ${opts.weeks * opts.perWeek} transactions`,
        'A couple of minutes against the board.',
      ],
      confirmLabel: 'Add it',
    });
    if (ok === null) return;

    await this.seeder.run(opts);
    await Promise.all([this.session.refresh(), this.loadLedger()]);

    const failure = this.seeder.lastError();
    if (!failure) {
      this.toasts.ok('A year of history added.');
      return;
    }
    // A failed seed used to say so only in text inside this panel - which
    // collapses when the page re-renders, so the run appeared to stop for no
    // reason and with nothing on screen. Say it where every other failure is
    // said.
    this.toasts.error(`Seeding stopped: ${failure}`);
  }

  /**
   * Reverses a transaction by appending its mirror image. The original is
   * never edited, so the household gets an audit trail rather than a rewritten
   * history.
   *
   * This may leave an account negative - if the recipient already spent the
   * money - and that is deliberate: the correction matters more than the
   * invariant, and the negative balance is left visible.
   */
  protected async reverse(t: Transaction): Promise<void> {
    if (this.reversing()) return;

    const reason = await this.dialogs.prompt({
      title: 'Reverse this transaction?',
      message:
        'This appends a correction. Nothing is deleted, and the original stays in the ledger.',
      detail: [t.description || t.kind],
      placeholder: 'Why is this being reversed?',
      confirmLabel: 'Reverse',
      required: true,
      danger: true,
    });
    if (reason === null) return;

    this.reversing.set(t.id);
    try {
      await this.api.reverse(t.id, reason, newIdempotencyKey());
      this.toasts.ok('Reversed.');
      await Promise.all([this.session.refresh(), this.loadLedger()]);
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.reversing.set(null);
    }
  }

  protected async setStatus(id: string, status: 'ACTIVE' | 'DISABLED'): Promise<void> {
    try {
      await this.api.setUserStatus(id, status);
      this.toasts.ok(status === 'DISABLED' ? 'Member disabled.' : 'Member re-enabled.');
      await this.session.refresh();
    } catch (e) {
      this.toasts.fromError(e);
    }
  }

  protected async setMastodonId(id: string, displayName: string, current = ''): Promise<void> {
    const value = await this.dialogs.prompt({
      title: `${displayName}'s Mastodon ID`,
      message: 'This is the only Mastodon recipient NanaCoin will allow household messages to use.',
      detail: ['Use a full handle such as @alex@mastodon.social. Leave it empty to remove it.'],
      placeholder: '@name@server.example',
      initial: current,
      confirmLabel: 'Save',
    });
    if (value === null) return;
    try {
      await this.api.setUserMastodonId(id, value.trim());
      this.toasts.ok('Mastodon ID saved.');
      await this.session.refresh();
    } catch (e) { this.toasts.fromError(e); }
  }

  /** Nana can replace a member's forgotten password; the server revokes that
   * member's existing sessions as part of the same operation. */
  protected async resetPassword(id: string, displayName: string): Promise<void> {
    const password = await this.dialogs.password({
      title: `Reset ${displayName}'s password`,
      message: 'Choose a temporary PIN or password and give it to them privately.',
      detail: ["Their other signed-in sessions will end immediately."],
      placeholder: 'At least 4 characters',
      confirmLabel: 'Reset password',
      required: true,
    });
    if (password === null) return;
    if (password.length < 4) {
      this.toasts.error('The password must be at least 4 characters.');
      return;
    }
    try {
      await this.api.setUserPassword(id, password);
      this.toasts.ok(`${displayName}'s password was reset.`);
    } catch (e) {
      this.toasts.fromError(e);
    }
  }

  /** A reversal cannot itself be reversed, and issuance is corrected by retiring. */
  protected reversible(t: Transaction): boolean {
    if (t.reference?.startsWith('lotto-') || t.reference?.startsWith('nickle:')) return false;
    return t.kind !== 'MESSAGE' && !t.reversed_by && t.kind !== 'REVERSAL' && t.kind !== 'ISSUE';
  }

  protected kindLabel(kind: Transaction['kind']): string {
    switch (kind) {
      case 'ISSUE':
        return 'Issued';
      case 'RETIRE':
        return 'Retired';
      case 'PURCHASE':
        return 'Purchase';
      case 'REVERSAL':
        return 'Correction';
      default:
        return 'Transfer';
    }
  }

  protected when(unixSeconds: number): string {
    return new Date(unixSeconds * 1000).toLocaleString(undefined, {
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit',
    });
  }

  protected journalKb(): string {
    const used = this.session.status()?.journal_used ?? 0;
    return `${(used / 1024).toFixed(1)} KB`;
  }
}
