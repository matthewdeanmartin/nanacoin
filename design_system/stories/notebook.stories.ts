import { type Meta, type StoryObj, moduleMetadata } from "@storybook/angular";
import { Notebook } from "../../nanacoin_ui/src/app/ui/notebook";

const meta: Meta = {
  title: "Components/Notebook",
  decorators: [moduleMetadata({ imports: [Notebook] })],
};
export default meta;

export const Ledger: StoryObj = {
  render: () => ({
    template: `
      <app-notebook>
        <div class="ledger">
          <article class="ledger-row">
            <div class="ledger-row__head">
              <span class="ledger-row__kind">Transfer</span>
              <span class="ledger-row__desc">Mowing the lawn</span>
              <span class="ledger-row__when">Sep 28</span>
            </div>
            <div class="ledger-row__postings">
              <span class="posting posting--debit">Nana −15</span>
              <span class="posting posting--credit">Timmy +15</span>
            </div>
          </article>
          <article class="ledger-row">
            <div class="ledger-row__head">
              <span class="ledger-row__kind">Allowance</span>
              <span class="ledger-row__desc">Weekly</span>
              <span class="ledger-row__when">Sep 27</span>
            </div>
            <div class="ledger-row__postings">
              <span class="posting posting--credit">Timmy +5</span>
            </div>
          </article>
        </div>
      </app-notebook>
    `,
  }),
};
