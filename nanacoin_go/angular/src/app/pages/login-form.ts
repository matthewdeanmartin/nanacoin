import { Component, inject, input, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { Session } from '../api/session';
import { DEMO_USERS, IS_DEMO } from '../demo/demo';
import { Toasts } from '../ui/toasts';

@Component({
  selector: 'app-login-form',
  imports: [FormsModule],
  template: `
    <div class="panel">
      <h1>{{ household() }}</h1>

      @if (adding()) {
        <!--
          Adding an account rather than signing in. Saying so matters: the two
          screens are otherwise identical, and someone who opened this by
          mistake has no way to tell they are still signed in as someone else,
          nor any way back.
        -->
        <p class="muted small">
          Add another account. @if (current()) { You stay signed in as
          <strong>{{ current() }}</strong>, and can switch between them from
          the top bar. }
        </p>
      }

      @if (isDemo) {
        <!--
          The public demo signs in by picking a person. Asking a visitor to
          type a password printed beside the box would be a puzzle rather than
          a demonstration - and there is nothing here to protect: the whole
          household lives in this tab and vanishes when it closes.
        -->
        <p class="muted small">
          A demonstration household. Pick someone to look around as — everything
          happens in this browser tab, and nothing is saved.
        </p>

        <div class="whoami">
          @for (d of demoUsers; track d.username) {
            <button
              class="whoami__pick"
              type="button"
              [disabled]="busy()"
              (click)="loginAs(d.username)"
            >
              <strong>{{ d.label }}</strong>
              <span class="whoami__hint">{{ d.hint }}</span>
            </button>
          }
        </div>
      } @else {
      <form (ngSubmit)="submit()">
        <label>
          Username
          <input name="username" [(ngModel)]="username" required autocomplete="username" />
        </label>
        <label>
          PIN or password
          <input name="password" type="password" [(ngModel)]="password" required
                 autocomplete="current-password" />
        </label>
        <button class="btn" type="submit" [disabled]="busy()">
          {{ busy() ? (adding() ? 'Adding…' : 'Logging in…') : (adding() ? 'Add account' : 'Log in') }}
        </button>
        @if (adding()) {
          <button class="btn btn--quiet" type="button" (click)="cancel.emit()">
            Cancel
          </button>
        }
      </form>
      }

      @if (isDemo && adding()) {
        <button class="btn btn--quiet" type="button" (click)="cancel.emit()">Cancel</button>
      }
    </div>
  `,
})
export class LoginForm {
  private readonly session = inject(Session);
  private readonly toasts = inject(Toasts);

  readonly household = input('NanaCoin');

  /** True when this is adding a second account rather than signing in. */
  readonly adding = input(false);

  /** Who stays signed in while adding, for the explanatory line. */
  readonly current = input('');

  readonly done = output<void>();

  /** Abandon adding and go back to the account already signed in. */
  readonly cancel = output<void>();

  protected username = '';
  protected password = '';
  protected readonly busy = signal(false);

  protected readonly isDemo = IS_DEMO;
  protected readonly demoUsers = DEMO_USERS;

  /** One click, no password: the demo's whole login. */
  protected async loginAs(username: string): Promise<void> {
    this.username = username;
    // The demo backend ignores it, but the client still runs the real PKCE
    // exchange, so something has to be sent.
    this.password = 'demo';
    await this.submit();
  }

  protected async submit(): Promise<void> {
    if (this.busy()) return;
    this.busy.set(true);
    try {
      await this.session.login(this.username.trim(), this.password);
      // Clear the password whatever happens next; it has served its purpose
      // and there is no reason for it to sit in a component field.
      this.password = '';
      this.done.emit();
    } catch (e) {
      this.password = '';
      this.toasts.fromError(e);
    } finally {
      this.busy.set(false);
    }
  }
}
