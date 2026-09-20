import { Component, inject } from '@angular/core';

import { Toasts } from './toasts';

@Component({
  selector: 'app-toast-list',
  template: `
    <div class="toasts" role="status" aria-live="polite">
      @for (t of toasts.items(); track t.id) {
        <div class="toast" [class.toast--error]="t.kind === 'error'">
          <span>{{ t.text }}</span>
          <button class="toast__close" (click)="toasts.dismiss(t.id)" aria-label="Dismiss">
            &times;
          </button>
        </div>
      }
    </div>
  `,
  styles: `
    .toasts {
      /* Fixed to the bottom rather than the top: the top bar carries the
         balance, which is the number people look at right after an action, so
         a toast must not cover it. */
      position: fixed;
      bottom: calc(1rem + env(safe-area-inset-bottom, 0px));
      left: 50%;
      transform: translateX(-50%);
      z-index: 20;
      display: grid;
      gap: 0.5rem;
      width: min(34rem, calc(100vw - 1.5rem));
    }
    .toast {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      padding: 0.6rem 0.75rem 0.6rem 1rem;
      border-radius: 10px;
      background: var(--ink);
      color: var(--bg);
      font-weight: 600;
      font-size: 0.9rem;
      box-shadow: 0 6px 20px rgb(0 0 0 / 0.22);
    }
    .toast--error {
      background: var(--warn-ink);
      color: #fff;
    }
    .toast span { flex: 1; min-width: 0; }
    .toast__close {
      background: none;
      border: 0;
      color: inherit;
      font-size: 1.2rem;
      line-height: 1;
      cursor: pointer;
      opacity: 0.7;
    }
    .toast__close:hover { opacity: 1; }
  `,
})
export class ToastList {
  protected readonly toasts = inject(Toasts);
}
