import { Injectable, signal } from '@angular/core';

/** 'auto' follows the device's light/dark setting; the rest pin a palette in styles.css. */
export type ThemeId = 'auto' | 'light' | 'dark' | 'garden' | 'night' | 'mono-light' | 'mono-dark';

export interface ThemeChoice {
  id: ThemeId;
  label: string;
  hint: string;
  /** Background, surface, accent, ink: enough to recognise the palette in a picker. */
  swatch: readonly [string, string, string, string];
}

/** Keep swatches in step with the palettes in styles.css. */
export const THEMES: readonly ThemeChoice[] = [
  { id: 'auto', label: 'Match my device', hint: 'Warm light or warm dark, following your device', swatch: ['#fbf7f0', '#241f1b', '#b8562f', '#e08050'] },
  { id: 'light', label: 'Warm light', hint: 'Cream paper and terracotta', swatch: ['#fbf7f0', '#fffdfa', '#b8562f', '#2a2520'] },
  { id: 'dark', label: 'Warm dark', hint: 'Cocoa and ember', swatch: ['#1b1815', '#241f1b', '#e08050', '#efe7dc'] },
  { id: 'garden', label: 'Garden', hint: 'Light, with leafy greens', swatch: ['#f3f7ef', '#fbfdf8', '#3b7336', '#1f2a1f'] },
  { id: 'night', label: 'Night sky', hint: 'Dark, with cool blues', swatch: ['#10151f', '#171e2b', '#7aa7ff', '#e4e9f2'] },
  { id: 'mono-light', label: 'Monochrome light', hint: 'Black on white, no colour', swatch: ['#ffffff', '#f7f7f7', '#222222', '#111111'] },
  { id: 'mono-dark', label: 'Monochrome dark', hint: 'White on black, no colour', swatch: ['#0e0e0e', '#191919', '#e6e6e6', '#ededed'] },
];

const THEME_KEY = 'nanacoin:theme';

function stored(): ThemeId {
  try {
    const saved = localStorage.getItem(THEME_KEY);
    if (THEMES.some((t) => t.id === saved)) return saved as ThemeId;
  } catch { /* Storage blocked: follow the device. */ }
  return 'auto';
}

function apply(theme: ThemeId): void {
  const root = document.documentElement;
  if (theme === 'auto') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', theme);
}

/** Called from main.ts before Angular starts, so the first paint is already in the chosen palette. */
export function applyStoredTheme(): void {
  apply(stored());
}

/**
 * The viewer's colour theme. A per-browser preference, never sent to the
 * server: two people sharing one account on different devices may well want
 * different themes.
 */
@Injectable({ providedIn: 'root' })
export class Theme {
  readonly current = signal<ThemeId>(stored());

  set(theme: ThemeId): void {
    this.current.set(theme);
    apply(theme);
    try {
      if (theme === 'auto') localStorage.removeItem(THEME_KEY);
      else localStorage.setItem(THEME_KEY, theme);
    } catch { /* The theme still applies for this visit. */ }
  }
}
