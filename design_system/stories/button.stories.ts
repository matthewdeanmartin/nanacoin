import { type Meta, type StoryObj } from "@storybook/angular";

interface ButtonArgs {
  label: string;
  variant: "" | "btn--quiet" | "btn--danger";
  small: boolean;
  disabled: boolean;
}

const meta: Meta<ButtonArgs> = {
  title: "Actions/Button",
  tags: ["autodocs"],
  argTypes: {
    variant: {
      control: "select",
      options: ["", "btn--quiet", "btn--danger"],
      labels: { "": "primary" },
    },
  },
  args: { label: "Send coins", variant: "", small: false, disabled: false },
  render: (args) => ({
    props: args,
    template: `<button type="button" class="btn {{ variant }}" [class.btn--small]="small"
      [disabled]="disabled">{{ label }}</button>`,
  }),
};
export default meta;
type Story = StoryObj<ButtonArgs>;

export const Primary: Story = {};
export const Quiet: Story = {
  args: { variant: "btn--quiet", label: "Cancel" },
};
export const Danger: Story = {
  args: { variant: "btn--danger", label: "Reverse payment" },
};
export const Small: Story = { args: { small: true, label: "Buy" } };
export const Disabled: Story = { args: { disabled: true } };

export const ButtonRow: Story = {
  name: "Button row",
  render: () => ({
    template: `
      <div class="button-row">
        <button type="button" class="btn">Save</button>
        <button type="button" class="btn btn--quiet">Cancel</button>
        <button type="button" class="btn btn--danger">Delete listing</button>
      </div>
    `,
  }),
};
