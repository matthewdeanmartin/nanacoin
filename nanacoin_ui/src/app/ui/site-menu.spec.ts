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
   expect(buySell?.textContent).toContain('Offer to Sell');
   expect(buySell?.textContent).toContain('Offers Received');
   expect(fixture.nativeElement.textContent).toContain('Board Health');
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
 });
});
