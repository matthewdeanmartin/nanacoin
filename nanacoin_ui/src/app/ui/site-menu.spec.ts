import { TestBed } from '@angular/core/testing';
import { provideRouter } from '@angular/router';
import { signal } from '@angular/core';
import { Session } from '../api/session';
import { SiteMenu } from './site-menu';

describe('responsive site navigation', () => {
 afterEach(() => TestBed.resetTestingModule());
 function setup(nana = false) {
   TestBed.configureTestingModule({ providers: [provideRouter([]), { provide: Session, useValue: {
     signedIn: signal(true), isNana: signal(nana), diagAvailable: signal(true), logsAvailable: signal(false),
   } }] });
   const fixture = TestBed.createComponent(SiteMenu); fixture.detectChanges(); return fixture;
 }
 it('uses one navigation landmark and hides Nana controls for members', () => {
   const fixture = setup();
   expect(fixture.nativeElement.querySelectorAll('nav')).toHaveLength(1);
   expect(fixture.nativeElement.textContent).not.toContain('Household');
   expect(fixture.nativeElement.textContent).toContain('About');
   const buySell = [...fixture.nativeElement.querySelectorAll('details')].find((group: Element) => group.textContent?.includes('Buy/Sell'));
   expect(buySell?.textContent).toContain('Market');
   expect(buySell?.textContent).toContain('Buy, sell, hire');
   expect(buySell?.textContent).not.toContain('Offers');
   const mail = [...fixture.nativeElement.querySelectorAll('details')].find((g: Element) => g.querySelector('summary')?.textContent === 'Mail');
   expect([...mail!.querySelectorAll('a')].map((a: Element)=>a.textContent)).toEqual(['Send Money','Messages','Offers','Invitations']);
   const loans = [...fixture.nativeElement.querySelectorAll('details')].find((g: Element) => g.querySelector('summary')?.textContent === 'Loans');
   expect([...loans!.querySelectorAll('a')].map((a: Element)=>a.textContent)).toEqual(['Loans','Lotto']);
   expect(fixture.nativeElement.textContent).toContain('Board Health');
 });
 it('exposes the new System Info pages without signing in', () => {
   TestBed.configureTestingModule({ providers: [provideRouter([]), { provide: Session, useValue: {
     signedIn: signal(false), isNana: signal(false), diagAvailable: signal(false), logsAvailable: signal(false),
   } }] });
   const fixture = TestBed.createComponent(SiteMenu); fixture.detectChanges();
   const links = [...fixture.nativeElement.querySelectorAll('a')].map((a: HTMLAnchorElement) => a.textContent?.trim());
   expect(links).toContain('Error Log'); expect(links).toContain('Database'); expect(links).toContain('Configuration');
   expect(links).not.toContain('Household');
 });
 it('toggles expanded state and dismisses on Escape or outside click', () => {
   const fixture = setup(true);
   const button = fixture.nativeElement.querySelector('button');
   expect(button.getAttribute('aria-expanded')).toBe('false');
   button.click(); fixture.detectChanges();
   expect(button.getAttribute('aria-expanded')).toBe('true');
   button.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); fixture.detectChanges();
   expect(button.getAttribute('aria-expanded')).toBe('false');
   button.click(); fixture.detectChanges();
   document.body.click(); fixture.detectChanges();
   expect(button.getAttribute('aria-expanded')).toBe('false');
   expect(fixture.nativeElement.textContent).toContain('Household');
   const groups=Array.from(fixture.nativeElement.querySelectorAll('details')) as HTMLDetailsElement[];
   expect(groups.find(g=>g.querySelector('summary')?.textContent==='Accounts')?.textContent).toContain('My Account');
   const help=groups.find(g=>g.querySelector('summary')?.textContent==='Help')!;
   expect(Array.from(help.querySelectorAll('a')).map(a=>a.textContent)).toEqual(['About','Docs']);
 });
});
