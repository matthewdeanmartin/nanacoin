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
