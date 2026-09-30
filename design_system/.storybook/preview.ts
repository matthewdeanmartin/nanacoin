import { applicationConfig, type Preview } from "@storybook/angular";
import { provideZonelessChangeDetection } from "@angular/core";

const preview: Preview = {
  decorators: [
    // The app is zoneless (signals throughout); the catalogue matches it.
    applicationConfig({ providers: [provideZonelessChangeDetection()] }),
    (story, context) => {
      // "auto" follows the OS like the real app; the rest pin one of the
      // palettes in styles.css, as My Settings > Appearance does.
      const root = document.documentElement;
      const theme = context.globals["theme"];
      if (theme === "auto") root.removeAttribute("data-theme");
      else root.setAttribute("data-theme", theme);
      return story();
    },
  ],
  globalTypes: {
    theme: {
      toolbar: {
        title: "Theme",
        icon: "circlehollow",
        items: [
          "auto",
          "light",
          "dark",
          "garden",
          "night",
          "mono-light",
          "mono-dark",
        ],
        dynamicTitle: true,
      },
    },
  },
  initialGlobals: { theme: "auto" },
  parameters: {
    layout: "padded",
    controls: { expanded: true },
    options: {
      storySort: {
        order: [
          "Start here",
          "Foundations",
          "Actions",
          "Forms",
          "Navigation",
          "Content",
          "Feedback",
          "Components",
        ],
      },
    },
  },
};
export default preview;
