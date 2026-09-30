// styles.css holds every colour theme: the device-following default, its dark
// twin, and one [data-theme] block per pinned palette. A palette that forgets
// a token silently inherits the default's colour, which in a dark theme means
// dark text on a dark page. These checks keep every palette complete, and the
// app's theme list and the Storybook toolbar in step with the CSS.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const read = (path) =>
  readFileSync(fileURLToPath(new URL(path, import.meta.url)), "utf8");
const app = read("../../nanacoin_ui/src/styles.css");
const themeTs = read("../../nanacoin_ui/src/app/ui/theme.ts");
const storybook = read("../.storybook/preview.ts");

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

const colors = (all) =>
  Object.fromEntries(Object.entries(all).filter(([, v]) => v.startsWith("#")));

const defaults = colors(tokens(app, ":root {"));
const deviceDark = tokens(app, "@media (prefers-color-scheme: dark)");
const pinned = [...app.matchAll(/:root\[data-theme="([\w-]+)"\]/g)].map(
  ([, id]) => id,
);

test("every pinned palette defines every colour token", () => {
  assert.equal(pinned.length, 6);
  for (const id of pinned) {
    assert.deepEqual(
      Object.keys(tokens(app, `:root[data-theme="${id}"]`)).sort(),
      Object.keys(defaults).sort(),
      id,
    );
  }
});

test("warm light and warm dark are exactly the device-following palettes", () => {
  assert.deepEqual(tokens(app, ':root[data-theme="light"]'), defaults);
  assert.deepEqual(tokens(app, ':root[data-theme="dark"]'), deviceDark);
});

test("the Appearance picker and the Storybook toolbar list the same palettes", () => {
  const picker = [...themeTs.matchAll(/\{ id: '([\w-]+)'/g)].map(
    ([, id]) => id,
  );
  assert.deepEqual(picker, ["auto", ...pinned]);
  for (const id of picker) assert.ok(storybook.includes(`"${id}"`), id);
});
