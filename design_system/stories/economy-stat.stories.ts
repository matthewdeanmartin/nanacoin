import { type Meta, type StoryObj } from "@storybook/angular";
import { EconomyStat } from "../../nanacoin_ui/src/app/ui/economy-stat";

const meta: Meta<EconomyStat> = {
  title: "Components/Economy stat",
  component: EconomyStat,
  tags: ["autodocs"],
  args: {
    id: "supply",
    value: "4,210 NC",
    label: "Money supply",
    help: "Every coin that exists right now: minted, minus anything burned.",
  },
  render: (args) => ({
    props: args,
    template: `
      <div class="stats economy-stats">
        <app-economy-stat class="stat" [id]="id" [value]="value" [label]="label" [help]="help" />
      </div>
    `,
  }),
};
export default meta;
type Story = StoryObj<EconomyStat>;

export const Single: Story = {};

export const Grid: Story = {
  render: () => ({
    template: `
      <div class="stats economy-stats">
        <app-economy-stat class="stat" id="s1" value="4,210 NC" label="Money supply"
          help="Every coin that exists right now." />
        <app-economy-stat class="stat" id="s2" value="1.4" label="Velocity"
          help="How many times the average coin changed hands this month." />
        <app-economy-stat class="stat" id="s3" value="2.1%" label="Inflation"
          help="Change in the average market price over 30 days." />
      </div>
    `,
  }),
};
