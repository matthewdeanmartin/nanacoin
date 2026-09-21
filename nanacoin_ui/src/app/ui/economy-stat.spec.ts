import { TestBed } from '@angular/core/testing';
import { EconomyStat } from './economy-stat';

describe('economic indicator definitions', () => {
  it('exposes the definition on hover, keyboard focus, and touch, with Escape dismissal', () => {
    const fixture = TestBed.createComponent(EconomyStat);
    for (const [name, value] of Object.entries({ id: 'test-help', value: '12%', label: 'employment', help: 'Available history may be shorter.' })) {
      fixture.componentRef.setInput(name, value);
    }
    fixture.detectChanges();
    const el: HTMLElement = fixture.nativeElement;
    const card = el.querySelector('.stat')!;
    const button = el.querySelector('button')!;
    const help = el.querySelector('p')!;
    expect(help.hidden).toBe(true);
    card.dispatchEvent(new Event('mouseenter')); fixture.detectChanges();
    expect(help.hidden).toBe(false);
    card.dispatchEvent(new Event('mouseleave')); fixture.detectChanges();
    expect(help.hidden).toBe(true);
    button.dispatchEvent(new Event('focus')); fixture.detectChanges();
    expect(button.getAttribute('aria-expanded')).toBe('true');
    button.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' })); fixture.detectChanges();
    expect(help.hidden).toBe(true);
    button.click(); fixture.detectChanges();
    expect(help.hidden).toBe(false);
    // A real touch/click leaves focus on the button. A second tap must close it.
    button.dispatchEvent(new Event('focus')); fixture.detectChanges();
    button.click(); fixture.detectChanges();
    expect(help.hidden).toBe(true);
    button.click(); fixture.detectChanges();
    button.dispatchEvent(new Event('blur')); fixture.detectChanges();
    expect(help.hidden).toBe(false);
    button.click(); fixture.detectChanges();
    expect(help.hidden).toBe(true);
  });
});
