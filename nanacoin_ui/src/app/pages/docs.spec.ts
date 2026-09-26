import { TestBed } from '@angular/core/testing';
import { provideRouter } from '@angular/router';
import { DocsPage } from './docs';

describe('visitor help',()=>{
  afterEach(()=>TestBed.resetTestingModule());
  it('shows one expanded topic, supports keyboard tabs and links to the real board runbook',()=>{
    TestBed.configureTestingModule({providers:[provideRouter([])]});
    const fixture=TestBed.createComponent(DocsPage);fixture.detectChanges();
    const root=fixture.nativeElement as HTMLElement;
    expect(root.querySelectorAll('[role="tabpanel"]')).toHaveLength(1);
    expect(root.querySelector('h2')?.textContent).toBe('Start here');
    const start=root.querySelector('[role="tab"]')!;
    start.dispatchEvent(new KeyboardEvent('keydown',{key:'End',bubbles:true}));fixture.detectChanges();
    expect(root.querySelector('h2')?.textContent).toBe('Get your own NanaCoin');
    expect(root.textContent).toContain('ESP32-S3-N16R8');
    expect(root.querySelector('a[href$="/nanacoin_rs/DEPLOY.md"]')).not.toBeNull();
    expect(root.querySelector('[role="tabpanel"]')?.getAttribute('aria-labelledby')).toBe('docs-tab-docs-hardware');
    (root.querySelector('#docs-tab-docs-nana') as HTMLButtonElement).click();fixture.detectChanges();
    expect(root.querySelector('h2')?.textContent).toBe('Nana is the household treasury');
    expect(root.textContent).toContain('cannot sell labor');
    expect(root.querySelectorAll('pre')).toHaveLength(0);
  });
});
