import { Component, ElementRef, computed, inject, signal, viewChild } from '@angular/core';
import { NavigationEnd, Router, RouterLink, RouterLinkActive } from '@angular/router';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { Session } from '../api/session';
import { IS_DEMO } from '../demo/demo';

@Component({
 selector: 'app-site-menu', imports: [RouterLink, RouterLinkActive],
 host: { '(keydown.escape)': 'close(true)', '(document:click)': 'outside($event)', '(focusout)': 'focusOut($event)' },
 template: `<header class="menu-bar">
   <a class="brand" routerLink="/market" aria-label="NanaCoin app" (click)="close()">NanaCoin<span>the household bank</span></a>
   <button #toggle type="button" class="menu-toggle" aria-controls="site-navigation" [attr.aria-expanded]="open()"
     [attr.aria-label]="open() ? 'Close navigation menu' : 'Open navigation menu'" (click)="open.set(!open())">
     <span aria-hidden="true" class="hamburger"><i></i><i></i><i></i></span><span>Menu</span>
   </button>
   <nav id="site-navigation" aria-label="Main navigation" [class.is-open]="open()">
     @for (link of links(); track link.path) {
       <a [routerLink]="link.path" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">{{ link.label }}</a>
     }
   </nav>
 </header>`,
 styleUrl: './site-menu.css',
})
export class SiteMenu {
 readonly open = signal(false);
 private readonly host = inject(ElementRef<HTMLElement>);
 private readonly toggle = viewChild<ElementRef<HTMLButtonElement>>('toggle');
 private readonly session = inject(Session);
 readonly links = computed(() => {
   const links = this.session.signedIn() ? [
     { path: '/market', label: 'Market' }, { path: '/send', label: 'Send' },
     { path: '/offers', label: 'Offers' }, { path: '/forex', label: 'Exchange' },
     { path: '/history', label: 'History' }, { path: '/economy', label: 'Economy' },
     ...(IS_DEMO ? [{ path: '/nickles', label: 'Nana-nickles' }] : []),
     ...(this.session.isNana() ? [{ path: '/nana', label: 'Household' },
       ...(!IS_DEMO && this.session.diagAvailable() ? [{ path: '/diagnostics', label: 'Machine health' }] : [])] : []),
     ...(this.session.logsAvailable() ? [{ path: '/logs', label: 'Server log' }] : []),
     { path: '/clientlog', label: 'Browser log' },
   ] : [];
   return [...links, { path: '/ledger', label: 'The notebook' }, { path: '/recipes', label: 'Lemon bars' },
     ...(IS_DEMO ? [{ path: '/diagnostics', label: 'Browser health' }] : []), { path: '/about', label: 'About' }];
 });
 constructor() {
   inject(Router).events.pipe(takeUntilDestroyed()).subscribe(e => { if (e instanceof NavigationEnd) this.close(); });
 }
 close(restoreFocus = false): void {
   const wasOpen = this.open(); this.open.set(false);
   if (restoreFocus && wasOpen) this.toggle()?.nativeElement.focus();
 }
 outside(event: Event): void { if (!this.host.nativeElement.contains(event.target as Node)) this.close(); }
 focusOut(event: FocusEvent): void {
   if (event.relatedTarget && !this.host.nativeElement.contains(event.relatedTarget as Node)) this.close();
 }
}
