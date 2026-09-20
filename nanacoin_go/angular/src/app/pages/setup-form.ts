// First run: no Nana exists yet, so whoever fills this in becomes one.
//
// The server permits this exactly once. There is no window in which a
// provisioned household can be re-provisioned, which is what keeps this
// unauthenticated form from being a way in.

import { Component, inject, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { NanacoinService } from '../api/nanacoin.service';
import { Toasts } from '../ui/toasts';

@Component({
  selector: 'app-setup-form',
  imports: [FormsModule],
  template: `
    <div class="panel">
      <h1>Set up NanaCoin</h1>
      <p class="muted">
        This server has no household yet. Whoever fills this in becomes Nana:
        the one who issues coins, adds members and fixes mistakes.
      </p>

      <form (ngSubmit)="submit()">
        <label>
          Household name
          <input name="household" [(ngModel)]="household" required maxlength="40"
                 placeholder="The Martin House" />
        </label>
        <label>
          Your username
          <input name="username" [(ngModel)]="username" required maxlength="40"
                 autocomplete="username" placeholder="nana" />
        </label>
        <label>
          Your display name
          <input name="display" [(ngModel)]="displayName" maxlength="40" placeholder="Nana" />
        </label>
        <label>
          PIN or password
          <input name="password" type="password" [(ngModel)]="password" required minlength="4"
                 autocomplete="new-password" />
        </label>
        <button class="btn" title="Create the household and its first Nana account" type="submit" [disabled]="busy()">
          {{ busy() ? 'Creating…' : 'Create household' }}
        </button>
      </form>
    </div>
  `,
})
export class SetupForm {
  private readonly api = inject(NanacoinService);
  private readonly toasts = inject(Toasts);

  readonly done = output<void>();

  protected household = '';
  protected username = '';
  protected displayName = '';
  protected password = '';
  protected readonly busy = signal(false);

  protected async submit(): Promise<void> {
    if (this.busy()) return;
    this.busy.set(true);
    try {
      await this.api.provision(
        this.username.trim(),
        this.displayName.trim(),
        this.password,
        this.household.trim(),
      );
      this.password = '';
      this.done.emit();
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.busy.set(false);
    }
  }
}
