import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import type { StorybookConfig } from "@storybook/angular";

// Dependencies live in the Angular workspace (nanacoin_ui); the catalogue lives
// beside it at design_system/. The builder runs with nanacoin_ui as its cwd.
const require = createRequire(resolve(process.cwd(), "package.json"));
const packagePath = (name: string) =>
  dirname(require.resolve(`${name}/package.json`));

const config: StorybookConfig = {
  stories: ["../stories/**/*.stories.ts"],
  framework: { name: packagePath("@storybook/angular"), options: {} },
  addons: [
    packagePath("@storybook/addon-docs"),
    packagePath("@storybook/addon-a11y"),
  ],
  core: { disableTelemetry: true },
  webpackFinal: async (config) => {
    config.resolve ??= {};
    config.resolve.modules = [
      resolve(process.cwd(), "node_modules"),
      ...(config.resolve.modules ?? []),
    ];
    return config;
  },
};
export default config;
