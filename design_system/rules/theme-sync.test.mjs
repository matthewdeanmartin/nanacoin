// preview.css repeats the app palettes under [data-theme] so the Storybook
// toolbar can force light or dark. This keeps those copies honest.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const read = (path) =>
  readFileSync(fileURLToPath(new URL(path, import.meta.url)), "utf8");
const app = read("../../nanacoin_ui/src/styles.css");
const preview = read("../preview.css");

/** Custom properties declared in the first block after `marker`. */
function tokens(css, marker) {
  const start = css.indexOf(marker);
  assert.ok(start >= 0, `missing ${marker}`);
  const open = css.indexOf("{", css.indexOf(":root", start));
  const body = css.slice(open + 1, css.indexOf("}", open));
  return Object.fromEntries(
    [...body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map(([, k, v]) => [
      k,
      v.trim(),
    ]),
  );
}

const appLight = tokens(app, ":root {");
const appDark = tokens(app, "@media (prefers-color-scheme: dark)");

const colors = (all) =>
  Object.fromEntries(Object.entries(all).filter(([, v]) => v.startsWith("#")));

test("forced light palette matches styles.css", () => {
  assert.deepEqual(
    tokens(preview, ':root[data-theme="light"]'),
    colors(appLight),
  );
});

test("forced dark palette matches styles.css", () => {
  assert.deepEqual(tokens(preview, ':root[data-theme="dark"]'), appDark);
});
