import { Directive, input } from '@angular/core';

/** In-page navigation must not replace Angular's #/route with #section. */
@Directive({ selector: 'button[appSectionLink]', host: { '(click)': 'scroll()' } })
export class SectionLink {
  readonly appSectionLink = input.required<string>();
  protected scroll(): void {
    const target = document.getElementById(this.appSectionLink());
    if (!target) return;
    if (target instanceof HTMLDetailsElement) target.open = true;
    target.setAttribute('tabindex', '-1');
    target.focus({ preventScroll: true });
    target.scrollIntoView({ block: 'start' });
  }
}
