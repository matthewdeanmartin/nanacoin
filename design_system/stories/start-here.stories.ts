import { type Meta, type StoryObj } from "@storybook/angular";

const meta: Meta = {
  title: "Start here/Overview",
  parameters: { layout: "fullscreen" },
};
export default meta;

export const Overview: StoryObj = {
  render: () => ({
    template: `
      <main class="screen ds-sheet">
        <h1>NanaCoin design system</h1>
        <p class="lede">
          A household currency should feel like a well-kept paper ledger: warm,
          calm, legible, and never flashy about money.
        </p>

        <section class="ds-section">
          <h2>How it is built</h2>
          <p>
            There is no component framework. One stylesheet,
            <code>nanacoin_ui/src/styles.css</code>, holds every token and
            shared class. A handful of Angular components in
            <code>nanacoin_ui/src/app/ui/</code> add behaviour where plain
            HTML is not enough. Every story here renders the real stylesheet
            and the real components, so the catalogue cannot drift from the app.
          </p>
        </section>

        <section class="ds-section">
          <h2>Principles</h2>
          <ul>
            <li><strong>Tokens, not literals.</strong> Colours come from the
              <code>--bg</code>, <code>--ink</code>, <code>--accent</code>
              family so light and dark both work.</li>
            <li><strong>Credit is green, debit is rust.</strong> Money direction
              is always <code>--credit</code> / <code>--debit</code> plus a
              sign, never colour alone.</li>
            <li><strong>Platform first.</strong> Dialogs use
              <code>&lt;dialog&gt;</code>; disclosures use
              <code>&lt;details&gt;</code>. The browser supplies focus and
              Escape handling.</li>
            <li><strong>Phone first.</strong> Inputs are 16px so iOS does not
              zoom; layouts wrap instead of scrolling sideways.</li>
          </ul>
        </section>

        <section class="ds-section">
          <h2>Using the toolbar</h2>
          <p>
            <strong>Theme</strong> switches between the OS setting (auto) and a
            forced light or dark palette. The <strong>Accessibility</strong>
            panel runs axe on every story.
          </p>
        </section>
      </main>
    `,
  }),
};
