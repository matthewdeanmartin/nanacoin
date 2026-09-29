import { provideRouter, withHashLocation } from "@angular/router";
import {
  type Meta,
  type StoryObj,
  applicationConfig,
  moduleMetadata,
} from "@storybook/angular";
import { KeyboardHelp } from "../../nanacoin_ui/src/app/ui/keyboard-help";

const meta: Meta = {
  title: "Components/Keyboard help",
  parameters: { layout: "fullscreen" },
  decorators: [
    // Shortcuts navigate with the router; an empty route table is enough here.
    applicationConfig({ providers: [provideRouter([], withHashLocation())] }),
    moduleMetadata({ imports: [KeyboardHelp] }),
  ],
};
export default meta;

export const Footer: StoryObj = {
  render: () => ({
    template: `<app-keyboard-help />`,
  }),
};
