import { Component, effect, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { NanacoinService, EmailSettings } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { IS_DEMO } from '../demo/demo';

@Component({
  selector: 'app-email', imports: [FormsModule, RouterLink],
  template: `
    <h1>Email</h1>
    @if (demo) { <p>Email is unavailable in the web demo.</p> }
    @else {
      <p>Account recovery, household invitations, and messages people choose to send. No automatic payment emails.</p>
      @if (notice()) { <p role="status">{{ notice() }}</p> }
      @if (error()) { <p class="warning" role="alert">{{ error() }}</p> }
      @if (session.signedIn()) {
        @if (settings(); as s) {
          <section class="panel">
            <h2>Your email address</h2>
            <p>{{ s.address || 'No verified address yet.' }} · Provider: {{ s.provider }}</p>
            @if (s.last_error) { <p class="warning">Last delivery problem: {{ s.last_error }}</p> }
            <form (ngSubmit)="verifyAddress()">
              <label>Email address <input type="email" name="address" [(ngModel)]="address" required maxlength="254" autocomplete="email" /></label>
              <label>Current PIN or password <input type="password" name="currentPassword" [(ngModel)]="currentPassword" required autocomplete="current-password" /></label>
              <button class="btn" [disabled]="busy()">Email a verification code</button>
            </form>
            <form (ngSubmit)="confirmAddress()">
              <label>Verification code <input name="verification" [(ngModel)]="verification" required maxlength="32" autocomplete="one-time-code" /></label>
              <button class="btn" [disabled]="busy()">Verify address</button>
            </form>
            @if (s.address) {
              <label><input type="checkbox" [ngModel]="s.notifications" (ngModelChange)="preferences($event)" [disabled]="busy()" /> Receive emails sent by household members</label>
              <p>Recovery emails are still available when member emails are off.</p>
              <button class="btn btn--quiet" (click)="preferences(false, true)" [disabled]="busy()">Remove my email address</button>
            }
          </section>
          <section class="panel">
            <h2>Email a household member</h2>
            <p>The recipient must have a verified address and allow member emails.</p>
            <form (ngSubmit)="notify()">
              <label>To <select name="recipient" [(ngModel)]="recipient" required>
                <option value="">Choose a member</option>
                @for (person of session.household(); track person.id) {
                  @if (person.status === 'ACTIVE' && person.kind !== 'bot') { <option [value]="person.id">{{ person.display_name }}</option> }
                }
              </select></label>
              <label>Subject <input name="subject" [(ngModel)]="subject" maxlength="80" required /></label>
              <label>Message <textarea name="text" [(ngModel)]="text" maxlength="512" required rows="4"></textarea></label>
              <button class="btn" [disabled]="busy()">Send email</button>
            </form>
          </section>
          <section class="panel">
            <h2>Invite someone to the household</h2>
            <p>Your invitation gives directions to NanaCoin. Nana still creates their account.</p>
            <form (ngSubmit)="invite()">
              <label>Email address <input type="email" name="inviteAddress" [(ngModel)]="inviteAddress" maxlength="254" required /></label>
              <button class="btn" [disabled]="busy()">Email invitation</button>
            </form>
          </section>
        } @else { <button class="btn" (click)="load()" [disabled]="busy()">Check email availability</button> }
      } @else {
        <section class="panel">
          <h2>Forgot your PIN or password?</h2>
          <p>Use the email address you verified before losing access. Codes expire after 15 minutes. If you have no verified address, ask Nana for help.</p>
          <form (ngSubmit)="recover()">
            <label>Username <input name="username" [(ngModel)]="username" required maxlength="32" autocomplete="username" /></label>
            <button class="btn" [disabled]="busy()">Email a recovery code</button>
          </form>
          <form (ngSubmit)="reset()">
            <label>Recovery code <input name="code" [(ngModel)]="code" required maxlength="32" autocomplete="one-time-code" /></label>
            <label>New PIN or password <input type="password" name="password" [(ngModel)]="password" required minlength="4" maxlength="128" autocomplete="new-password" /></label>
            <button class="btn" [disabled]="busy()">Set new password</button>
          </form>
          <p><a routerLink="/">Back to sign in</a></p>
        </section>
      }
      <p class="muted">Sending is limited to one request per minute and the household's daily allowance. Queued does not mean delivered; check the recipient's inbox and spam folder.</p>
    }
  `,
  styles: `section { margin-block: 1.5rem } form { display: grid; gap: .7rem; margin-block: 1rem; max-width: 38rem } label { display: grid; gap: .35rem }`,
})
export class EmailPage {
  protected readonly session = inject(Session);
  private readonly api = inject(NanacoinService);
  protected readonly demo = IS_DEMO;
  protected readonly settings = signal<EmailSettings | null>(null);
  protected readonly busy = signal(false);
  protected readonly notice = signal('');
  protected readonly error = signal('');
  protected address = ''; protected currentPassword = ''; protected verification = '';
  protected recipient = ''; protected subject = ''; protected text = ''; protected inviteAddress = '';
  protected username = ''; protected code = ''; protected password = '';
  private account = '';
  constructor() {
    effect(() => {
      const account = this.session.me()?.id ?? '';
      this.account = account;
      this.settings.set(null);
      this.address = ''; this.currentPassword = ''; this.verification = '';
      if (!IS_DEMO && account) void this.load();
    });
  }
  protected async load(): Promise<void> {
    if (IS_DEMO) return;
    const account = this.account;
    try { const settings = await this.api.emailSettings(); if (account === this.account) { this.settings.set(settings); this.address = settings.address; } }
    catch { if (account === this.account) this.error.set('Email is not available. Check the connection and the bank’s email configuration.'); }
  }
  private async send(action: string, body: unknown, notice: string): Promise<boolean> {
    if (IS_DEMO || this.busy()) return false;
    this.busy.set(true); this.error.set(''); this.notice.set('');
    try { await this.api.emailAction(action, body); this.notice.set(notice); if (this.session.signedIn()) await this.load(); return true; }
    catch { this.error.set('Could not complete this request. Check the address or code; for sending limits, wait a minute. The recipient must allow email.'); return false; }
    finally { this.busy.set(false); }
  }
  protected async verifyAddress(): Promise<void> {
    const password = this.currentPassword; this.currentPassword = '';
    await this.send('address', { address: this.address, password }, 'Verification email queued. Enter its code below.');
  }
  protected async confirmAddress(): Promise<void> {
    if (await this.send('verify', { code: this.verification }, 'Address verified. Account recovery is ready.')) this.verification = '';
  }
  protected async preferences(notifications: boolean, remove = false): Promise<void> { await this.send('preferences', { notifications, remove }, 'Email preferences saved.'); }
  protected async notify(): Promise<void> {
    if (await this.send('notify', { member: Number(this.recipient.replace('user-', '')), subject: this.subject, text: this.text }, 'Email queued.')) { this.subject = ''; this.text = ''; }
  }
  protected async invite(): Promise<void> { if (await this.send('invite', { address: this.inviteAddress }, 'Invitation queued.')) this.inviteAddress = ''; }
  protected async recover(): Promise<void> { await this.send('recover', { username: this.username }, 'If this account has a verified address and sending is available, a recovery email is queued.'); }
  protected async reset(): Promise<void> {
    const password = this.password; this.password = '';
    if (await this.send('reset', { code: this.code, password }, 'Password changed. Return to sign in with your new password.')) this.code = '';
  }
}
