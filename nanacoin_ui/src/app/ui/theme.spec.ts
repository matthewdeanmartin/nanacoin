import { TestBed } from '@angular/core/testing';

import { THEMES, Theme, applyStoredTheme } from './theme';

describe('colour themes', () => {
  afterEach(() => { localStorage.clear(); document.documentElement.removeAttribute('data-theme'); });

  it('offers six palettes plus following the device', () => {
    expect(THEMES.filter((t) => t.id !== 'auto')).toHaveLength(6);
    expect(THEMES.map((t) => t.id)).toContain('mono-light');
    expect(THEMES.map((t) => t.id)).toContain('mono-dark');
  });

  it('pins a palette on <html>, remembers it, and forgets it for auto', () => {
    const theme = TestBed.inject(Theme);
    theme.set('mono-dark');
    expect(document.documentElement.dataset['theme']).toBe('mono-dark');
    document.documentElement.removeAttribute('data-theme');
    applyStoredTheme();
    expect(document.documentElement.dataset['theme']).toBe('mono-dark');
    theme.set('auto');
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
    expect(localStorage.getItem('nanacoin:theme')).toBeNull();
  });

});
