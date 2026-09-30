import { Component, computed, signal } from '@angular/core';
import { RouterLink } from '@angular/router';

import { IngredientGroup, IngredientList } from '../ui/ingredient-list';

type Units = 'metric' | 'imperial';

const UNITS_KEY = 'nanacoin:recipe-units';

/** A measure written both ways. Kitchen conversions, not arithmetic: 115 g of butter is "½ cup", not "0.49 cups". */
type Both = readonly [metric: string, imperial: string];

interface Line { amount?: Both; name: Both | string; note?: string }
interface Group { name: string; items: readonly Line[] }

const VEGETARIAN: readonly Group[] = [
  { name: 'Base', items: [
    { amount: ['150 g', '1¼ cups'], name: ['plain flour', 'all-purpose flour'] },
    { amount: ['50 g', 'scant ½ cup'], name: ['icing sugar', 'powdered sugar'] },
    { amount: ['115 g', '½ cup (1 stick)'], name: 'unsalted butter', note: 'softened' },
    { name: 'a pinch of salt' },
  ] },
  { name: 'Filling', items: [
    { amount: ['3', '3'], name: 'large eggs' },
    { amount: ['180 g', '¾ cup + 2 tbsp'], name: ['caster sugar', 'superfine sugar'] },
    { amount: ['120 ml', '½ cup'], name: 'lemon juice' },
    { name: 'zest of 2 lemons' },
    { amount: ['25 g', '3 tbsp'], name: ['plain flour', 'all-purpose flour'] },
  ] },
  { name: 'To finish', items: [
    { name: ['icing sugar', 'powdered sugar'], note: 'optional, for dusting' },
  ] },
];

const VEGAN: readonly Group[] = [
  { name: 'Base', items: [
    { amount: ['150 g', '1¼ cups'], name: ['plain flour', 'all-purpose flour'] },
    { amount: ['50 g', 'scant ½ cup'], name: ['icing sugar', 'powdered sugar'] },
    { amount: ['115 g', '½ cup'], name: 'firm vegan baking butter' },
    { name: 'a pinch of salt' },
  ] },
  { name: 'Filling', items: [
    { amount: ['200 ml', '¾ cup + 1 tbsp'], name: 'unsweetened oat milk', note: 'cold' },
    { amount: ['120 ml', '½ cup'], name: 'lemon juice' },
    { amount: ['150 g', '¾ cup'], name: ['caster sugar', 'superfine sugar'] },
    { amount: ['35 g', '¼ cup + 1 tsp'], name: ['cornflour', 'cornstarch'] },
    { name: 'zest of 2 lemons' },
    { amount: ['30 g', '2 tbsp'], name: 'vegan butter' },
    { name: 'a pinch of turmeric', note: 'optional, for colour' },
  ] },
];

function initialUnits(): Units {
  try {
    const saved = localStorage.getItem(UNITS_KEY);
    if (saved === 'metric' || saved === 'imperial') return saved;
  } catch { /* Storage can be blocked; fall through to the locale. */ }
  // The United States, Liberia and Myanmar still cook in cups.
  return /-(US|LR|MM)$/i.test(navigator.language) ? 'imperial' : 'metric';
}

@Component({
  selector: 'app-recipes', imports: [RouterLink, IngredientList],
  template: `<h1>The lemon-bar admission policy</h1>
  <p>Everyone is welcome. No login, purchase, dietary disclosure or actual eating required. <a routerLink="/ledger">Look at the ledger</a> while the kettle boils.</p>
  <p>Two home-kitchen recipes, each for about 16 squares in a lined {{ m('20 cm', '8-inch') }} square tin.</p>

  <fieldset class="segmented" aria-label="Measurements">
    <label><input type="radio" name="units" value="metric" [checked]="units() === 'metric'" (change)="setUnits('metric')" /><span>Metric</span></label>
    <label><input type="radio" name="units" value="imperial" [checked]="units() === 'imperial'" (change)="setUnits('imperial')" /><span>US cups</span></label>
  </fieldset>

  <section class="panel"><h2>Vegetarian lemon bars</h2>
  <h3>Ingredients</h3>
  <app-ingredient-list [groups]="vegetarian()" />
  <h3>Method</h3>
  <ol><li>Heat the oven to {{ m('175 °C', '350 °F') }}. Line the tin with baking paper, leaving lifting handles.</li>
  <li>Mix the base ingredients into a crumbly dough, press evenly into the tin and bake 18–22 minutes until lightly golden.</li>
  <li>Whisk the filling until smooth. Pour onto the hot base; bake 20–25 minutes until the centre is set, not liquid.</li>
  <li>Cool, refrigerate at least 2 hours, then lift out and cut. Dust with sugar if you like.</li></ol></section>

  <section class="panel"><h2>Vegan lemon bars</h2>
  <h3>Ingredients</h3>
  <app-ingredient-list [groups]="vegan()" />
  <h3>Method</h3>
  <ol><li>Heat the oven to {{ m('175 °C', '350 °F') }}. Line the tin. Mix and press in the base; bake 20–25 minutes until lightly golden.</li>
  <li>In a saucepan whisk the {{ m('cornflour', 'cornstarch') }} with the cold oat milk until lump-free. Add sugar, juice and zest. Stir over medium heat until bubbling and thick; cook, stirring, for about 1 minute.</li>
  <li>Remove from heat and stir in the vegan butter. Spread onto the baked base. Let cool, then refrigerate at least 4 hours until firm before slicing.</li>
  <li>The starch-set filling is softer and more opaque than the egg version.</li></ol></section>`,
})
export class RecipesPage {
  protected readonly units = signal<Units>(initialUnits());
  protected readonly vegetarian = computed(() => this.resolve(VEGETARIAN));
  protected readonly vegan = computed(() => this.resolve(VEGAN));

  protected setUnits(units: Units): void {
    this.units.set(units);
    try { localStorage.setItem(UNITS_KEY, units); } catch { /* A convenience only. */ }
  }

  /** Picks the metric or imperial wording. */
  protected m(metric: string, imperial: string): string {
    return this.units() === 'metric' ? metric : imperial;
  }

  private resolve(groups: readonly Group[]): IngredientGroup[] {
    const pick = (v: Both | string) => (typeof v === 'string' ? v : this.m(v[0], v[1]));
    return groups.map((g) => ({
      name: g.name,
      items: g.items.map((i) => ({ amount: i.amount && pick(i.amount), name: pick(i.name), note: i.note })),
    }));
  }
}
