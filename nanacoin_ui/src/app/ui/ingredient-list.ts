import { Component, input } from '@angular/core';

export interface Ingredient {
  /** Already in the reader's units: '150 g', '1¼ cups'. Empty for 'a pinch of salt'. */
  amount?: string;
  name: string;
  /** Preparation or an alternative, shown after the name: 'softened', 'optional'. */
  note?: string;
}

export interface IngredientGroup {
  /** 'Base', 'Filling'. Omit for a recipe with one list. */
  name?: string;
  items: readonly Ingredient[];
}

/**
 * A recipe's ingredients, grouped, one per bulleted line so they can be ticked
 * off while cooking. Amounts come first and in bold because that is what the
 * eye scans for; conversion between unit systems is the caller's business.
 */
@Component({
  selector: 'app-ingredient-list',
  template: `<div class="ingredient-list">
    @for (group of groups(); track $index) {
      @if (group.name) { <h4 class="ingredient-list__group">{{ group.name }}</h4> }
      <ul>
        @for (item of group.items; track $index) {
          <li>
            @if (item.amount) { <strong class="ingredient-list__amount">{{ item.amount }}</strong> }
            {{ item.name }}@if (item.note) {<span class="ingredient-list__note">{{ item.note }}</span>}
          </li>
        }
      </ul>
    }
  </div>`,
})
export class IngredientList {
  readonly groups = input.required<readonly IngredientGroup[]>();
}
