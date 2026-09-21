// Renders whichever dialog is open. Mounted once, beside the toasts.
//
// Uses the platform <dialog> element rather than a div with a z-index: the
// browser gives focus trapping, Escape-to-close, inertness of the page behind,
// and the top layer for free. Reimplementing those correctly is more code than
// this whole component.

import {
  Component,
  ElementRef,
  computed,
  effect,
  inject,
  signal,
  viewChild,
} from '@angular/core';
import { FormsModule } from '@angular/forms';

import { Dialogs } from './dialog';

@Component({
  selector: 'app-dialog-host',
  imports: [FormsModule],
  template: `
    <dialog #box class="dlg" (cancel)="onCancel($event)" (click)="onBackdrop($event)">
      @if (req(); as r) {
        <form method="dialog" class="dlg__body" (submit)="onSubmit($event)">
          <h2 class="dlg__title">{{ r.title }}</h2>

          @if (r.message) {
            <p class="dlg__message">{{ r.message }}</p>
          }

          @if (r.detail?.length) {
            <ul class="dlg__detail">
              @for (line of r.detail; track line) {
                <li>{{ line }}</li>
              }
            </ul>
          }

          @if (r.kind === 'offer') {
            <label class="dlg__field">
              How many coins?
              <input
                #field
                class="dlg__input"
                type="number"
                min="1"
                [(ngModel)]="value"
                [ngModelOptions]="{ standalone: true }"
              />
            </label>
            <label class="dlg__field">
              Anything to say with it?
              <input
                class="dlg__input"
                type="text"
                [placeholder]="r.placeholder ?? 'Optional'"
                [(ngModel)]="note"
                [ngModelOptions]="{ standalone: true }"
              />
            </label>
            @if (error()) {
              <p class="dlg__error">{{ error() }}</p>
            }
          } @else if (r.kind !== 'confirm') {
            <label class="dlg__field">
              <span class="visually-hidden">{{ r.title }}</span>
              <input
                #field
                class="dlg__input"
                [type]="r.kind === 'number' ? 'number' : r.kind === 'password' ? 'password' : 'text'"
                [placeholder]="r.placeholder ?? ''"
                [attr.autocomplete]="r.kind === 'password' ? 'new-password' : null"
                [attr.min]="r.min ?? null"
                [attr.max]="r.max ?? null"
                [(ngModel)]="value"
                [ngModelOptions]="{ standalone: true }"
              />
            </label>
            @if (error()) {
              <p class="dlg__error">{{ error() }}</p>
            }
          }

          <div class="dlg__actions">
            <button class="btn btn--quiet" type="button" title="Close without making this change" (click)="cancel()">Cancel</button>
            <button
              class="btn"
              [class.btn--danger]="r.danger"
              type="submit"
              [attr.title]="r.confirmLabel ?? 'Confirm this action'"
            >
              {{ r.confirmLabel ?? 'Confirm' }}
            </button>
          </div>
        </form>
      }
    </dialog>
  `,
})
export class DialogHost {
  private readonly dialogs = inject(Dialogs);

  protected readonly req = computed(() => this.dialogs.current());
  protected value = '';
  /** The second field, used only by the offer dialog. */
  protected note = '';
  protected readonly error = signal('');

  private readonly box = viewChild.required<ElementRef<HTMLDialogElement>>('box');
  private readonly field = viewChild<ElementRef<HTMLInputElement>>('field');

  constructor() {
    effect(() => {
      const r = this.req();
      const el = this.box().nativeElement;

      if (!r) {
        if (el.open) el.close();
        return;
      }

      this.value = r.initial ?? '';
      this.note = '';
      this.error.set('');
      if (!el.open) el.showModal();

      // Focus the input so the keyboard is usable immediately - the native
      // prompt did this, and losing it would be a step backwards.
      queueMicrotask(() => this.field()?.nativeElement.select());
    });
  }

  protected onSubmit(event: Event): void {
    event.preventDefault();
    const r = this.req();
    if (!r) return;

    if (r.kind === 'confirm') {
      this.dialogs.settle('yes');
      return;
    }

    if (r.kind === 'offer') {
      const amount = Number(this.value.trim());
      if (amount < 0) {
        this.error.set("Can't do negative prices.");
        return;
      }
      if (!Number.isInteger(amount) || amount === 0) {
        this.error.set('Enter a whole number of coins.');
        return;
      }
      this.dialogs.settle(JSON.stringify({ amount, message: this.note.trim() }));
      return;
    }

    const text = this.value.trim();
    if (r.required && !text) {
      this.error.set('This cannot be empty.');
      return;
    }
    if (r.kind === 'number') {
      const n = Number(text);
      if (!Number.isInteger(n) || n <= 0) {
        this.error.set('Enter a whole number greater than zero.');
        return;
      }
      if (r.min !== undefined && n < r.min) {
        this.error.set(`Must be at least ${r.min}.`);
        return;
      }
      if (r.max !== undefined && n > r.max) {
        this.error.set(`Must be at most ${r.max}.`);
        return;
      }
    }
    this.dialogs.settle(text);
  }

  protected cancel(): void {
    this.dialogs.settle(null);
  }

  /** Escape, which the browser turns into a cancel event. */
  protected onCancel(event: Event): void {
    event.preventDefault();
    this.cancel();
  }

  /**
   * A click on the backdrop rather than the panel.
   *
   * The <dialog> element itself fills the viewport, so a click whose target is
   * the dialog and not its contents came from outside the panel.
   */
  protected onBackdrop(event: MouseEvent): void {
    if (event.target === this.box().nativeElement) this.cancel();
  }
}
