import { Component } from '@angular/core';
import { RouterLink } from '@angular/router';

@Component({
 selector: 'app-recipes', imports: [RouterLink],
 template: `<h1>The lemon-bar admission policy</h1>
 <p>Everyone is welcome. No login, purchase, dietary disclosure or actual eating required. <a routerLink="/ledger">Look at the ledger</a> while the kettle boils.</p>
 <p>Two original, untested home-kitchen recipes, each for about 16 squares in a lined 20 cm / 8-inch square tin. Results vary by oven and ingredients. Both contain wheat; check every ingredient label for allergies and cross-contact.</p>
 <section class="panel"><h2>Vegetarian lemon bars (with eggs and dairy)</h2>
 <h3>Ingredients</h3><p>Base: 150 g plain flour, 50 g icing sugar, 115 g softened unsalted butter, a pinch of salt.
 Filling: 3 large eggs, 180 g caster sugar, 120 ml lemon juice, zest of 2 washed lemons, 25 g plain flour. Optional icing sugar for serving.</p>
 <ol><li>Heat oven to 175 °C / 350 °F (conventional). Line the tin with baking paper, leaving lifting handles.</li>
 <li>Mix the base ingredients into a crumbly dough, press evenly into the tin and bake 18–22 minutes until lightly golden.</li>
 <li>Whisk the filling until smooth. Pour onto the hot base; bake 20–25 minutes until the centre is set, not liquid. If checking with a thermometer, the egg filling should reach 71 °C / 160 °F.</li>
 <li>Cool, refrigerate at least 2 hours, then lift out and cut. Dust lightly if desired. Refrigerate promptly and eat within 3 days.</li></ol>
 <p>Contains eggs, milk and wheat. Not vegan. <a href="https://ask.fsis.usda.gov/article/What-is-a-safe-internal-temperature-for-food-made-with-eggs">USDA egg-dish temperature guidance</a>.</p></section>
 <section class="panel"><h2>Vegan lemon bars (egg-free and dairy-free)</h2>
 <h3>Ingredients</h3><p>Base: 150 g plain flour, 50 g icing sugar, 115 g firm vegan baking butter, a pinch of salt.
 Filling: 200 ml unsweetened oat milk, 120 ml lemon juice, 150 g caster sugar, 35 g cornflour (cornstarch), zest of 2 washed lemons, 30 g vegan butter. A tiny pinch of turmeric is optional for colour.</p>
 <ol><li>Heat oven to 175 °C / 350 °F. Line the tin. Mix and press in the base; bake 20–25 minutes until lightly golden.</li>
 <li>In a saucepan whisk the cornflour with the cold oat milk until lump-free. Add sugar, juice and zest. Stir over medium heat until bubbling and thick; cook, stirring, for about 1 minute.</li>
 <li>Remove from heat and stir in the vegan butter. Spread onto the baked base. Let cool, then refrigerate at least 4 hours until firm before slicing.</li>
 <li>Keep refrigerated and eat within 3 days. The starch-set filling will be softer and more opaque than the egg version.</li></ol>
 <p>Contains wheat; oat milk or vegan butter may contain other allergens. Use products labelled vegan, including sugar if relevant to your practice. “Vegan” does not mean allergy-safe.</p></section>`,
})
export class RecipesPage {}
