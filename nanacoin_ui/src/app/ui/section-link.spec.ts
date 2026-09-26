import { Component } from '@angular/core';
import { TestBed } from '@angular/core/testing';
import { afterEach, expect, it, vi } from 'vitest';
import { SectionLink } from './section-link';

const sections = ['security', 'storage', 'members', 'money', 'demo-data', 'full-ledger'];
@Component({ imports: [SectionLink], template: `@for (id of sections; track id) { <button [appSectionLink]="id">{{ id }}</button>@if (id === 'demo-data') { <details [id]="id"><summary>Demo data</summary></details> } @else { <section [id]="id"></section> } }` })
class Page { readonly sections = sections; }
afterEach(() => { TestBed.resetTestingModule(); vi.restoreAllMocks(); });
it('all household section buttons scroll and focus without changing the hash route', () => {
  const fixture = TestBed.createComponent(Page);
  fixture.detectChanges();
  const before = location.hash;
  const root = fixture.nativeElement as HTMLElement;
  for (const [i, id] of sections.entries()) {
    const section = root.querySelector<HTMLElement>(`#${id}`)!;
    const scroll = vi.fn(); section.scrollIntoView = scroll;
    const focus = vi.spyOn(section, 'focus');
    root.querySelectorAll('button')[i].click();
    expect(scroll).toHaveBeenCalledOnce();
    expect(focus).toHaveBeenCalledWith({ preventScroll: true });
    expect(section.getAttribute('tabindex')).toBe('-1');
    if (id === 'demo-data') expect((section as HTMLDetailsElement).open).toBe(true);
    expect(location.hash).toBe(before);
  }
});
