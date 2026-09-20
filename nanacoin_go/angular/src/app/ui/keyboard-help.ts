import { Component, ElementRef, signal, viewChild } from '@angular/core';
import { inject } from '@angular/core';
import { Router } from '@angular/router';

const SETTING = 'nanacoin.keyboardShortcuts';
function enabledInitially(): boolean {
  try { return localStorage.getItem(SETTING) !== 'off'; } catch { return true; }
}
export function isEditing(target: EventTarget | null): boolean {
  return target instanceof Element && !!target.closest('input,textarea,select,[contenteditable]:not([contenteditable="false"]),[role="textbox"],[role="combobox"],[role="slider"]');
}

/** Mawkingbird/Mastodon interaction conventions; deliberately no money-action hotkeys. */
@Component({
 selector: 'app-keyboard-help',
 host: { '(document:keydown)': 'keydown($event)' },
 template: `<footer class="keyboard-footer">
   <button type="button" (click)="show()" title="Keyboard shortcuts — press ? outside a form"><kbd>?</kbd> for keyboard help</button>
   <span>Tab to navigate · Enter to activate · Escape to close menus</span>
 </footer>
 <dialog #help aria-labelledby="keyboard-help-title" (cancel)="close($event)" (click)="backdrop($event)">
   <header><h2 id="keyboard-help-title">Keyboard shortcuts</h2><button type="button" class="btn btn--quiet" autofocus (click)="close()">Close</button></header>
   <p>Familiar Mawkingbird / Mastodon patterns, adapted where there is a real NanaCoin equivalent. “Then” means a sequence, within one second—not keys held together.</p>
   <h3>Navigation</h3><dl>
     <dt><kbd>?</kbd></dt><dd>Open this help</dd>
     <dt><kbd>g</kbd> then <kbd>h</kbd></dt><dd>Home: Market (or sign-in screen)</dd>
     <dt><kbd>g</kbd> then <kbd>e</kbd></dt><dd>Explore: Market</dd>
     <dt><kbd>g</kbd> then <kbd>l</kbd></dt><dd>The notebook: public fictional ledger in the demo; existing permissions on a real board</dd>
   </dl>
   <h3>Lists and composing</h3><dl>
     <dt><kbd>j</kbd> / <kbd>k</kbd></dt><dd>Focus next / previous listing or transaction</dd>
     <dt><kbd>0</kbd></dt><dd>Focus the first listing or transaction</dd>
     <dt><kbd>Alt</kbd> + <kbd>PageDown</kbd> / <kbd>PageUp</kbd></dt><dd>Next / previous record</dd>
     <dt><kbd>n</kbd></dt><dd>Focus the new-listing title on Market, when its form is available</dd>
   </dl>
   <h3>Standard controls</h3><p><kbd>Tab</kbd> / <kbd>Shift</kbd> + <kbd>Tab</kbd> move between links and buttons.
   <kbd>Enter</kbd> activates the focused link or button; <kbd>Space</kbd> activates a focused button or checkbox.
   <kbd>Escape</kbd> closes the help or mobile menu and returns focus.</p>
   <p>Shortcuts do not run while you type, use an input method, or have a confirmation dialog open. No shortcut directly pays, purchases, mints, redeems or resets anything. Mastodon search, reply, favourite and boost keys have no mapping here.</p>
   <label class="enable"><input type="checkbox" [checked]="enabled()" (change)="toggle()"> Enable letter-key shortcuts (turn off if they conflict with assistive technology)</label>
 </dialog>`,
 styles: `
  .keyboard-footer{display:flex;justify-content:center;align-items:center;flex-wrap:wrap;gap:.65rem 1.5rem;padding:1.4rem 1rem;color:var(--ink-soft);font-size:.8rem;border-top:1px solid var(--line)}
  .keyboard-footer button{border:0;background:none;padding:.4rem;color:inherit;text-decoration:underline;cursor:pointer;font:inherit;min-height:44px}
  dialog{max-width:min(700px,calc(100vw - 2rem));max-height:85dvh;overflow:auto;border:1px solid #aa8a66;border-radius:5px;padding:1.4rem;background:#fffdf7;color:#36291e}
  dialog::backdrop{background:#21180f80}header{display:flex;align-items:center;justify-content:space-between;gap:1rem}h2{margin:0}h3{margin-top:1.2rem}
  dl{display:grid;grid-template-columns:minmax(8rem,1fr) 2fr;gap:.75rem}dd{margin:0}kbd{display:inline-block;border:1px solid #c0b39e;border-bottom-width:2px;border-radius:3px;padding:.05rem .3rem;font: .85em ui-monospace,monospace;background:#f3efe6}
  .enable{display:flex;align-items:center;gap:.65rem;margin-top:1rem}.enable input{width:auto;margin:0}button:focus-visible{outline:3px solid #467791;outline-offset:3px}
  @media(max-width:450px){dl{grid-template-columns:1fr}dd{margin-bottom:.4rem}}@media print{.keyboard-footer{display:none}}
 `,
})
export class KeyboardHelp {
 private readonly router = inject(Router);
 private readonly help = viewChild<ElementRef<HTMLDialogElement>>('help');
 private previousFocus: HTMLElement | null = null;
 private gUntil = 0;
 readonly enabled = signal(enabledInitially());
 show(): void {
   if (document.querySelector('dialog[open],[aria-modal="true"]')) return;
   this.gUntil = 0;
   this.previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
   this.help()?.nativeElement.showModal();
 }
 close(event?: Event): void {
   event?.preventDefault();
   this.help()?.nativeElement.close();
   if (this.previousFocus?.isConnected) this.previousFocus.focus();
 }
 backdrop(event: MouseEvent): void { if (event.target === this.help()?.nativeElement) this.close(); }
 toggle(): void {
   this.enabled.update(v => !v); this.gUntil = 0;
   try { localStorage.setItem(SETTING, this.enabled() ? 'on' : 'off'); } catch { /* session-only preference */ }
 }
 keydown(event: KeyboardEvent): void {
   const target = event.composedPath()[0] ?? event.target;
   if (event.defaultPrevented || event.repeat || event.isComposing || event.ctrlKey || event.metaKey || isEditing(target as EventTarget)
     || document.querySelector('dialog[open],[aria-modal="true"]') || !this.enabled()) { this.gUntil = 0; return; }
   const key = event.key.toLowerCase();
   let handled = false;
   if (event.altKey) {
     this.gUntil = 0;
     if (event.code === 'PageDown' || event.code === 'PageUp') handled = this.move(event.code === 'PageDown' ? 1 : -1);
   } else if (key === '?') { this.show(); handled = true; }
   else if (!event.shiftKey) {
     const sequence = this.gUntil > performance.now(); this.gUntil = 0;
     if (sequence && ['h', 'e', 'l'].includes(key)) {
       void this.router.navigateByUrl(key === 'l' ? '/ledger' : '/market'); handled = true;
     } else if (key === 'g') { this.gUntil = performance.now() + 1000; handled = true; }
     else if (key === 'j' || key === 'k') handled = this.move(key === 'j' ? 1 : -1);
     else if (key === '0') handled = this.focus(this.rows()[0]);
     else if (key === 'n') handled = this.focus(document.querySelector<HTMLElement>('app-market input[name="title"]') ?? undefined);
   }
   if (handled) { event.preventDefault(); event.stopPropagation(); }
 }
 private rows(): HTMLElement[] {
   return Array.from(document.querySelectorAll<HTMLElement>('main [data-keyboard-row]')).filter(e => e.getClientRects().length > 0);
 }
 private move(delta: number): boolean {
   const rows = this.rows();
   const current = document.activeElement?.closest('[data-keyboard-row]');
   const index = rows.findIndex(e => e === current);
   return this.focus(rows[index < 0 ? (delta > 0 ? 0 : rows.length - 1) : Math.max(0, Math.min(rows.length - 1, index + delta))]);
 }
 private focus(element?: HTMLElement): boolean {
   if (!element) return false;
   // Market keeps its composer in a collapsed details panel.
   for (let parent = element.parentElement; parent; parent = parent.parentElement) {
     if (parent instanceof HTMLDetailsElement) parent.open = true;
   }
   element.focus(); element.scrollIntoView({ block: 'center' }); return true;
 }
}
