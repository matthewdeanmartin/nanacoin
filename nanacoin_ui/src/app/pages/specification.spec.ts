import { TestBed } from '@angular/core/testing';
import { provideRouter } from '@angular/router';
import { SpecificationPage } from './specification';

describe('NanaCoin specification',()=>{
  afterEach(()=>TestBed.resetTestingModule());
  it('shows the core spec first and switches to the extended spec and roadmap',()=>{
    TestBed.configureTestingModule({providers:[provideRouter([])]});
    const fixture=TestBed.createComponent(SpecificationPage);fixture.detectChanges();
    const root=fixture.nativeElement as HTMLElement;
    expect(root.querySelectorAll('[role="tabpanel"]')).toHaveLength(1);
    expect(root.querySelector('h2')?.textContent).toBe('NanaCoin Specification (NCS 2026)');
    expect(root.textContent).toContain('lemon bar');
    expect(root.textContent).toContain('four (4) digits');
    (root.querySelector('#spec-tab-spec-nces') as HTMLButtonElement).click();fixture.detectChanges();
    expect(root.querySelector('h2')?.textContent).toBe('NanaCoin Extended Specification (NCES 2026)');
    expect(root.textContent).toContain('simple-interest loan');
    (root.querySelector('#spec-tab-spec-roadmap') as HTMLButtonElement).click();fixture.detectChanges();
    for (const item of ['Interbank exchange','Foreign currency exchange','Foreign trade','Corporations']) expect(root.textContent).toContain(item);
  });
});
