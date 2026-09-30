import { type Meta, type StoryObj } from "@storybook/angular";

const meta: Meta = { title: "Forms" };
export default meta;

export const Fields: StoryObj = {
  render: () => ({
    template: `
      <form class="ds-sheet" (submit)="$event.preventDefault()">
        <label>To whom
          <select><option>Grandma</option><option>Timmy</option></select>
        </label>
        <label>How many coins?
          <input type="number" min="1" placeholder="10" />
        </label>
        <label>What for?
          <input type="text" placeholder="Mowing the lawn" />
        </label>
        <label>A longer note
          <textarea rows="3" placeholder="Optional"></textarea>
        </label>
        <label class="checkbox">
          <input type="checkbox" checked /> <span>Remind me next week</span>
        </label>
        <button class="btn" type="submit">Send</button>
      </form>
    `,
  }),
};

export const Panel: StoryObj = {
  render: () => ({
    template: `
      <section class="panel ds-sheet" style="margin-top:0">
        <h2 style="margin-top:0">Weekly allowance</h2>
        <form (submit)="$event.preventDefault()">
          <label>Amount <input type="number" value="5" /></label>
          <label>Day <select><option>Saturday</option></select></label>
          <button class="btn" type="submit">Save allowance</button>
        </form>
      </section>
    `,
  }),
};

export const Disclosure: StoryObj = {
  render: () => ({
    template: `
      <div class="ds-sheet">
        <details class="disclosure">
          <summary>Buy, sell, hire</summary>
          <form (submit)="$event.preventDefault()">
            <label>Title <input name="title" /></label>
            <label>Price <input type="number" /></label>
            <button class="btn" type="submit">List it</button>
          </form>
        </details>
      </div>
    `,
  }),
};

export const Warning: StoryObj = {
  render: () => ({
    template: `
      <div class="ds-sheet">
        <p class="warning">This board is read-only until the owner reconnects it.</p>
      </div>
    `,
  }),
};

export const Segmented: StoryObj = {
  name: "Segmented control",
  render: () => ({
    props: { units: "metric" },
    template: `
      <div class="ds-sheet">
        <p class="muted small">
          A radio group that reads as one switch. Use for two to four views of
          the same content, such as the recipe measurements. Arrow keys move
          between choices, as with any radio group.
        </p>
        <fieldset class="segmented" aria-label="Measurements">
          <label><input type="radio" name="ds-units" value="metric" [checked]="units === 'metric'" (change)="units = 'metric'" /><span>Metric</span></label>
          <label><input type="radio" name="ds-units" value="imperial" [checked]="units === 'imperial'" (change)="units = 'imperial'" /><span>US cups</span></label>
        </fieldset>
        <p>Showing {{ units }}.</p>
      </div>
    `,
  }),
};

export const ThemePicker: StoryObj = {
  name: "Theme picker",
  render: () => ({
    template: `
      <fieldset class="theme-picker ds-sheet">
        <legend>Colour theme</legend>
        <label class="theme-picker__choice">
          <input type="radio" name="ds-theme" checked />
          <span class="theme-picker__swatch" aria-hidden="true"><i style="background:#fbf7f0"></i><i style="background:#fffdfa"></i><i style="background:#b8562f"></i><i style="background:#2a2520"></i></span>
          <span class="theme-picker__text"><strong>Warm light</strong><span>Cream paper and terracotta</span></span>
        </label>
        <label class="theme-picker__choice">
          <input type="radio" name="ds-theme" />
          <span class="theme-picker__swatch" aria-hidden="true"><i style="background:#0e0e0e"></i><i style="background:#191919"></i><i style="background:#e6e6e6"></i><i style="background:#ededed"></i></span>
          <span class="theme-picker__text"><strong>Monochrome dark</strong><span>White on black, no colour</span></span>
        </label>
      </fieldset>
    `,
  }),
};

export const SecretReveal: StoryObj = {
  name: "Secret shown once",
  render: () => ({
    template: `
      <div class="ds-sheet">
        <div class="secret-reveal" role="status">
          <p><strong>Copy your new key now.</strong> NanaCoin keeps only a fingerprint of it and cannot show it again.</p>
          <div class="secret-reveal__value">
            <input readonly value="nc_Q2hhbmdlIG1lIGJlZm9yZSB5b3UgcGFzdGUgbWU" aria-label="Your new API key" />
            <button class="btn" type="button">Copy</button>
          </div>
          <button class="btn btn--quiet" type="button">I have saved it</button>
        </div>
      </div>
    `,
  }),
};
