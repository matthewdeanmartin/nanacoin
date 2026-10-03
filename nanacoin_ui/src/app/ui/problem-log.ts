import { DOCUMENT } from '@angular/common';
import { DestroyRef, Injectable, inject } from '@angular/core';
import { Log } from '../api/log';

/** Keep inline warnings and validation messages as well as thrown errors.
 * Observe only explicit problem surfaces, never inputs or page contents. */
@Injectable({ providedIn: 'root' })
export class ProblemLog {
  private readonly log = inject(Log);
  private readonly document = inject(DOCUMENT);
  private readonly seen = new WeakMap<Element, string>();
  constructor() {
    const observer = new MutationObserver(() => this.record());
    observer.observe(this.document.body, {
      subtree: true,
      childList: true,
      characterData: true,
      attributes: true,
      attributeFilter: ['hidden', 'class', 'open'],
    });
    inject(DestroyRef).onDestroy(() => observer.disconnect());
    this.record();
  }
  record(): void {
    for (const element of this.document.querySelectorAll(
      '[role="alert"],.warning,.field-problem,.form-problems,.dlg__error',
    )) {
      // Toasts already record the underlying error, including its stack.
      if (element.closest('app-toast-list,[hidden],dialog:not([open])')) continue;
      const text = element.textContent?.trim();
      if (!text || this.seen.get(element) === text) continue;
      this.seen.set(element, text);
      this.log.warn('ui', text, { page: location.hash.split('?')[0] });
    }
  }
}
