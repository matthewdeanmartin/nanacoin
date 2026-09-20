import { Component, ElementRef, inject, signal, viewChild } from '@angular/core';
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
     @if (session.signedIn()) {
       <a routerLink="/history" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">My Account</a>
       @if (session.isNana()) {
         <a routerLink="/nana" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">Household</a>
       }
       <a routerLink="/send" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">Send</a>
       <details class="menu-group" routerLinkActive="current">
         <summary>Buy/Sell</summary>
         <div class="menu-group__items">
           <a routerLink="/market" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">Market</a>
           <a routerLink="/offers" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">Offers</a>
           <a routerLink="/forex" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">Exchange</a>
           @if (demo) {
             <a routerLink="/nickles" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">Nana-nickles</a>
           }
         </div>
       </details>
       <details class="menu-group" routerLinkActive="current">
         <summary>Accounting</summary>
         <div class="menu-group__items">
           <a routerLink="/economy" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">Economy</a>
           <a routerLink="/ledger" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">The Notebook</a>
         </div>
       </details>
       <details class="menu-group" routerLinkActive="current">
         <summary>System Info</summary>
         <div class="menu-group__items">
           <a routerLink="/clientlog" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">Browser Log</a>
           @if (demo) {
             <a routerLink="/diagnostics" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">Browser Health</a>
           } @else if (session.isNana() && session.diagAvailable()) {
             <a routerLink="/diagnostics" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">Board Health</a>
           }
           @if (session.logsAvailable()) {
             <a routerLink="/logs" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">Server Log</a>
           }
         </div>
       </details>
     } @else {
       <a routerLink="/ledger" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">The Notebook</a>
       @if (demo) {
         <details class="menu-group">
           <summary>System Info</summary>
           <div class="menu-group__items">
             <a routerLink="/diagnostics" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">Browser Health</a>
           </div>
         </details>
       }
     }
     <a class="about-link" routerLink="/about" routerLinkActive="current" ariaCurrentWhenActive="page" (click)="close()">About</a>
   </nav>
 </header>`,
 styleUrl: './site-menu.css',
})
export class SiteMenu {
 readonly open = signal(false);
 private readonly host = inject(ElementRef<HTMLElement>);
 private readonly toggle = viewChild<ElementRef<HTMLButtonElement>>('toggle');
 protected readonly session = inject(Session);
 readonly demo = IS_DEMO;
 constructor() {
   inject(Router).events.pipe(takeUntilDestroyed()).subscribe(e => { if (e instanceof NavigationEnd) this.close(); });
 }
 close(restoreFocus = false): void {
   const wasOpen = this.open(); this.open.set(false);
   for (const group of this.host.nativeElement.querySelectorAll('details')) group.open = false;
   if (restoreFocus && wasOpen) this.toggle()?.nativeElement.focus();
 }
 outside(event: Event): void { if (!this.host.nativeElement.contains(event.target as Node)) this.close(); }
 focusOut(event: FocusEvent): void {
   if (event.relatedTarget && !this.host.nativeElement.contains(event.relatedTarget as Node)) this.close();
 }
}
