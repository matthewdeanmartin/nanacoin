import { TestBed } from '@angular/core/testing';
import { Notebook, NotebookPreferences } from './notebook';

describe('notebook appearance preferences', () => {
  beforeEach(() => { localStorage.removeItem('nanacoin-notebook-paper'); localStorage.removeItem('nanacoin-notebook-cursive'); });
  afterEach(() => { TestBed.resetTestingModule(); localStorage.removeItem('nanacoin-notebook-paper'); localStorage.removeItem('nanacoin-notebook-cursive'); });
  it('defaults on and keeps independent choices across component and service recreation', () => {
    const fixture=TestBed.createComponent(Notebook); fixture.detectChanges();
    const inputs=fixture.nativeElement.querySelectorAll('input');
    expect(inputs[0].checked).toBe(true); expect(inputs[1].checked).toBe(true);
    inputs[0].click(); inputs[1].click(); fixture.detectChanges();
    TestBed.resetTestingModule();
    const preferences=TestBed.inject(NotebookPreferences);
    expect(preferences.paper()).toBe(false); expect(preferences.cursive()).toBe(false);
    preferences.set('paper',true);
    TestBed.resetTestingModule();
    expect(TestBed.inject(NotebookPreferences).paper()).toBe(true);
    expect(TestBed.inject(NotebookPreferences).cursive()).toBe(false);
  });
});
