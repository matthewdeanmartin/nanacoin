# Playwright showcase runbook

The GitHub Pages workflow tests the built, static demo with
`scripts/showcase-check.py`. It runs the whole showcase as one browser session,
including desktop and mobile navigation, seeded account flows, and a check that
the page makes no API or outside requests. Run it after building the demo; the
development server and NanaCoin API are not involved.

## Reproduce the CI run

From `nanacoin_ui/` on Linux or in GitHub Actions:

```sh
npm ci
npx ng build --configuration demo --base-href /nanacoin/
python -m pip install playwright==1.63.0
python -m playwright install --with-deps chromium
python scripts/showcase-check.py
```

The workflow sets `BASE_HREF=/nanacoin/`; set the same value for a local run
against that Pages build. The harness defaults to Playwright Chromium on Linux.
Its output reports the browser version and base path so CI and local runs can be
compared directly.

On Windows, the harness defaults to installed Microsoft Edge. From PowerShell,
you can use the project's documented `uv` workflow:

```powershell
npx ng build --configuration demo --base-href /nanacoin/
uv run --with playwright==1.63.0 playwright install chromium
uv run --with playwright==1.63.0 python scripts/showcase-check.py
```

For a closer comparison when Edge and Chromium behave differently, set
`$env:SHOWCASE_BROWSER = 'chromium'` before the last command. Install the
matching browser with `uv run --with playwright==1.63.0 playwright install chromium`.
On Windows this comparison may still differ from CI because CI uses Linux fonts
and Chromium's Linux headless shell.

## When a run fails

1. Start with the first Python traceback and identify the exact assertion in
   `scripts/showcase-check.py`. Later failures can be consequences of that
   first failure.
2. Rebuild from a clean dependency install and rerun the whole script in the
   same browser channel. It shares one demo session, so a failure can depend on
   navigation or state established earlier in the run.
3. For layout failures, inspect the reported viewport and element bounds. Check
   the CSS breakpoint, intrinsic text sizing, flex/grid minimum sizes, and
   whether the failure is overflow, clipping, wrapping, or delayed rendering.
   Keep assertions focused on visible behavior and include measured values in
   assertion messages.
4. For timing failures, wait for a user-visible state or DOM condition with
   Playwright's `expect`/`wait_for_function`. Avoid fixed sleeps; they hide
   scheduling differences and make failures intermittent.
5. For browser-only failures, compare the printed Playwright and browser
   versions, viewport, operating system, fonts, and build command. Reproduce in
   the CI Linux Chromium environment before changing browser-specific code.
6. For request failures, check the route guard and demo build configuration.
   The showcase must stay self-contained and must not reach the API or external
   sites.
7. Rerun the exact failing workflow command after a fix. Do not add retries or
   weaken a meaningful layout or behavior assertion just to make a flaky run
   green. If the failure cannot be reproduced locally, retain the CI traceback
   and browser details and compare the next run before changing the test.

The workflow lives in `.github/workflows/nanacoin-pages.yml`. It pins the
Playwright Python package, installs Chromium and its Linux dependencies, builds
the Pages demo with the required base href, and runs this harness.
