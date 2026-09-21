import { Component, input, signal } from '@angular/core';

/** Definitions work with a mouse, keyboard, and touch without truncating values. */
@Component({
  selector: 'app-economy-stat',
  template: `
    <div class="stat" (mouseenter)="hovered.set(true); dismissed.set(false)" (mouseleave)="hovered.set(false)">
      <span class="value">{{ value() }}</span>
      <button type="button" class="definition" [attr.aria-expanded]="expanded()"
        [attr.aria-controls]="id() + '-description'" (focus)="focused.set(true); dismissed.set(false)" (blur)="focused.set(false)"
        (click)="toggle()" (keydown.escape)="dismiss()">
        {{ label() }} <small aria-hidden="true">ⓘ</small>
      </button>
      <p [id]="id() + '-description'" class="help" [hidden]="!expanded()">{{ help() }}</p>
    </div>
  `,
  styles: `
    :host { display: block; min-width: 0; }
    .stat { height: 100%; box-sizing: border-box; min-width: 0; }
    .value { overflow-wrap: anywhere; }
    .definition { background: none; border: 0; padding: .25rem; color: var(--ink-soft);
      font: inherit; font-size: .8rem; cursor: help; max-width: 100%; }
    .definition:focus-visible { outline: 2px solid var(--accent); border-radius: .2rem; }
    .help { font-size: .8rem; line-height: 1.45; text-align: start; margin: .6rem 0 0;
      overflow-wrap: anywhere; color: var(--ink-soft); }
  `,
})
export class EconomyStat {
  readonly id = input.required<string>();
  readonly value = input.required<string>();
  readonly label = input.required<string>();
  readonly help = input.required<string>();
  protected readonly hovered = signal(false);
  protected readonly focused = signal(false);
  protected readonly pinned = signal(false);
  protected readonly dismissed = signal(false);
  protected expanded(): boolean { return !this.dismissed() && (this.hovered() || this.focused() || this.pinned()); }
  protected toggle(): void {
    if (this.pinned()) this.dismiss();
    else { this.pinned.set(true); this.dismissed.set(false); }
  }
  protected dismiss(): void { this.dismissed.set(true); this.pinned.set(false); }
}
