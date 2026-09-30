import { TestBed } from '@angular/core/testing';
import { provideRouter } from '@angular/router';

import { RecipesPage } from './recipes';

describe('lemon bars', () => {
  afterEach(() => { TestBed.resetTestingModule(); localStorage.clear(); });

  it('switches every measure between metric and US and remembers the choice', () => {
    localStorage.setItem('nanacoin:recipe-units', 'metric');
    TestBed.configureTestingModule({ providers: [provideRouter([])] });
    const fixture = TestBed.createComponent(RecipesPage); fixture.detectChanges();
    const root = fixture.nativeElement as HTMLElement;
    expect(root.textContent).toContain('150 g');
    expect(root.textContent).toContain('175 °C');
    expect(root.textContent).not.toContain('350 °F');

    (root.querySelector('input[value="imperial"]') as HTMLInputElement).click(); fixture.detectChanges();
    expect(root.textContent).toContain('1¼ cups');
    expect(root.textContent).toContain('all-purpose flour');
    expect(root.textContent).toContain('350 °F');
    expect(root.textContent).not.toContain('175 °C');
    expect(localStorage.getItem('nanacoin:recipe-units')).toBe('imperial');
  });

  it('lists ingredients as grouped bullets and leaves allergen labelling to the packets', () => {
    TestBed.configureTestingModule({ providers: [provideRouter([])] });
    const fixture = TestBed.createComponent(RecipesPage); fixture.detectChanges();
    const root = fixture.nativeElement as HTMLElement;
    const groups = [...root.querySelectorAll('.ingredient-list__group')].map((g) => g.textContent);
    expect(groups).toEqual(['Base', 'Filling', 'To finish', 'Base', 'Filling']);
    expect(root.querySelectorAll('.ingredient-list li').length).toBeGreaterThan(15);
    expect(root.textContent).not.toMatch(/allerg|Contains eggs|USDA|160 °F/i);
  });
});
