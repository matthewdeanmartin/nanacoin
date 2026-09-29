import { type Meta, type StoryObj } from "@storybook/angular";

const meta: Meta = { title: "Foundations" };
export default meta;

const palette = [
  { token: "--bg", use: "Page background, input fill" },
  { token: "--surface", use: "Cards, panels, rows" },
  { token: "--ink", use: "Body text, headings" },
  { token: "--ink-soft", use: "Labels, metadata, hints" },
  { token: "--line", use: "Borders and dividers" },
  { token: "--accent", use: "Primary action, emphasis" },
  { token: "--accent-soft", use: "Top bar, active tab, notes" },
  { token: "--credit", use: "Money coming in" },
  { token: "--debit", use: "Money going out" },
  { token: "--warn-bg", use: "Warning and error fill" },
  { token: "--warn-ink", use: "Warning and error text" },
];

export const Colors: StoryObj = {
  render: () => ({
    props: { palette },
    template: `
      <div class="ds-sheet">
        <h1>Colour tokens</h1>
        <p class="lede">Use the token, never the hex. Each has a light and dark value.</p>
        <div class="ds-swatches">
          @for (c of palette; track c.token) {
            <div class="ds-swatch">
              <div class="ds-swatch__chip" [style.background]="'var(' + c.token + ')'"></div>
              <code class="ds-swatch__name">{{ c.token }}</code>
              <span class="ds-swatch__use">{{ c.use }}</span>
            </div>
          }
        </div>
      </div>
    `,
  }),
};

export const Typography: StoryObj = {
  render: () => ({
    template: `
      <div class="ds-sheet">
        <h1>Heading 1 · page title</h1>
        <p class="lede">.lede · the sentence under a page title, in --ink-soft.</p>
        <h2>Heading 2 · section</h2>
        <h3>Heading 3 · subsection, softened</h3>
        <p>Body text is 16px rounded system UI at 1.5 line height. Grandma
          should be able to read it without her glasses.</p>
        <p class="muted">.muted · secondary copy.</p>
        <p class="small">.small · fine print.</p>
        <p class="empty">.empty · nothing here yet.</p>
        <p><span class="txn__amount">1,234.5 NC</span> · amounts use tabular numerals.</p>
      </div>
    `,
  }),
};

export const ShapeAndSurface: StoryObj = {
  name: "Shape and surface",
  render: () => ({
    template: `
      <div class="ds-sheet">
        <h1>Shape and surface</h1>
        <p class="lede"><code>--radius</code> (10px) for panels and cards; 8px for
          controls and rows; pills (999px) for tabs, tags and postings.</p>
        <section class="panel" style="margin-top:1rem">
          <h2 style="margin-top:0">.panel</h2>
          <p class="muted">A grouped block of settings or a form.</p>
        </section>
        <details class="disclosure" open>
          <summary>.disclosure</summary>
          <p class="muted">Collapsible secondary content, built on &lt;details&gt;.</p>
        </details>
      </div>
    `,
  }),
};
