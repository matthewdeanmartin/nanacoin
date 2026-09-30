# NanaCoin design system

A live Storybook catalogue of the NanaCoin client's visual language: colour
tokens, typography, buttons, forms, navigation, market cards, ledger rows, and
the shared Angular components in `nanacoin_ui/src/app/ui/`.

The catalogue renders the real `nanacoin_ui/src/styles.css` and the real
components, so it cannot drift from the app. Setup follows
`../mimb/mawkingbird/design_system`.

## Run it

From Git Bash:

```sh
cd nanacoin_ui
npm ci
npm run design        # http://127.0.0.1:6010
```

| Script                  | What it does                                               |
| ----------------------- | ---------------------------------------------------------- |
| `npm run design`        | Storybook dev server on 127.0.0.1:6010                     |
| `npm run design:build`  | Static catalogue in `design_system/dist/` (ignored)        |
| `npm run design:check`  | Prettier check plus TypeScript check of stories and config |
| `npm run design:format` | Prettier write                                             |
| `npm run design:rules`  | Node tests in `rules/` (theme palette sync)                |

## Layout

- `.storybook/` Storybook config. Dependencies live in `nanacoin_ui/node_modules`;
  `angular.json` has `storybook` and `build-storybook` targets pointing here.
- `stories/` One file per area: Foundations, Actions, Forms, Navigation,
  Content, Feedback (toasts, dialogs), Components.
- `preview.css` Catalogue-only styles plus forced light/dark palettes.
- `rules/` Checks that keep the catalogue honest.

## Theme toolbar

The app has six colour themes, all in `styles.css`: warm light and warm dark
(what "auto" shows, following `prefers-color-scheme`), garden, night sky, and
monochrome light and dark. My Settings > Appearance pins one by setting
`data-theme` on `<html>` (`app/ui/theme.ts`); the toolbar does the same.

`rules/theme-sync.test.mjs` fails if a palette misses a colour token, if warm
light/dark drift from the device-following palettes, or if the picker, CSS and
toolbar disagree about which themes exist. To add a theme, add its block to
`styles.css`, its entry (with swatch) to `THEMES`, and its id to the toolbar.

Use tokens, never literal colours, in components: `--on-accent` is the text
colour on an `--accent` fill (white in most themes, near-black in monochrome
dark and night sky).

## Adding a story

Import the component from `../../nanacoin_ui/src/app/...` and render it with
the global classes it uses in the app. Components that inject root services
(`Toasts`, `Dialogs`) work unchanged; drive them from a small demo component in
the story file, as `stories/feedback.stories.ts` does. Anything that needs the
router gets `applicationConfig({ providers: [provideRouter([])] })`.

## Versions

Storybook packages are pinned to **10.6.0** (supports Angular 18–22). Adding the
Angular adapter's peers required aligning all Angular packages on **22.2.0**.
