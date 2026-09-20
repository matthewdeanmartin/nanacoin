# NanaCoin load laboratory

Python **3.14**, **uv**, and **Locust** live here, separate from the firmware.
Tests target the real HTTP service at `http://192.168.1.158` by default.
This is a disposable development household: preparation can provision the
board, create `load_alice`/`load_bob`, and fund their accounts. Tests write real
transactions/listings. Nothing flashes, resets, or automatically reboots it.

## Quick start

```powershell
cd C:\github\microcontroller\nanacoin_load
make install
make test
make prepare
make e2e
make load SCENARIO=browse USERS=1 DURATION=60
make ramp SCENARIO=browse STEPS=1,2,4,8,12 STAGE=30
make reports
# Open http://127.0.0.1:8090
```

`make open` opens the offline HTML index directly. `make ui SCENARIO=browse`
opens Locust's control server at `http://127.0.0.1:8089`; press Start there.
The configured shape determines user counts/duration, rather than the form's
user-count field. Ctrl+C finishes the UI process and builds the evidence report.
`make soak` runs 30 minutes; `make suite` tries each isolated workload and stops
at the first failed run. No Make available? Every recipe is an ordinary
`uv run ncload ...` command; `uv run ncload --help` lists them.

Defaults for a newly provisioned test board are `nana` / `nana-pin` and member
password `locust-test-pin`. For an existing household, set `NANA_USERNAME`,
`NANA_PASSWORD`, and optionally `NANA_MEMBER_PASSWORD` in the shell. They are
read from the environment, never command-line arguments or reports.
`NANA_ORIGIN` defaults to `http://localhost:4200`. `HOST=...` overrides Make's
target; `NANA_HOST` overrides the CLI default. `uv.lock` pins dependencies.

Run `make prepare` after a reboot: the device loses its household and sessions.
Tokens are saved only in gitignored `.state/fixture.json`. Preparation does not
erase an existing household. If your preexisting credentials differ, login fails
and the setup report identifies the endpoint. Each preparation logs in again;
do not use preparation as a login-load loop (use the auth scenario).

## Workloads

| Scenario | What one virtual user does | Main question |
|---|---|---|
| `status` | Public status reads | Basic request/connection ceiling |
| `browse` | Five simultaneous reads: me, users, listings, status, history | SPA-style bursts and overlapping page loads |
| `ledger` | Nana reads 30 recent transactions | Encoding and socket transmission |
| `write` | New issuance with a fresh idempotency key | Write churn, ledger wrap, allocation pressure |
| `replay` | Four simultaneous issuances sharing one key; compare returned IDs | Retry contention and duplicate execution |
| `market` | Seller creates an offer, buyer purchases it | Multi-record updates, listing/text reuse |
| `auth` | PKCE login, token exchange, logout | Password hashing and session churn |

Virtual users share the two fixture identities (multiple tabs/devices), rather
than consuming the 16-person household table. Authentication is prepared before
ordinary workloads; only `auth` measures repeated login cost. The two members
start with 10,000 units each; a sufficiently long marketplace run can exhaust
buyer funds, which appears as a business refusal rather than a crash.

The default wait between journeys is 0.3–1 second. `browse` and `replay` create
5× and 4× the virtual-user request concurrency respectively. No Angular code,
client cache, client rate limiter, or client retry behavior is changed. These
are HTTP end-to-end user journeys; Locust does **not** execute browser JavaScript,
render Angular, measure browser paint, or automatically send CORS preflights.
The SPA burst is explicitly reproduced by concurrent requests.

Examples with finer controls:

```powershell
uv run ncload run --scenario status --users 4 --duration 60 --wait-min 0 --wait-max 0
uv run ncload run --scenario write --steps 1,2,4 --stage 45
uv run ncload run --scenario browse --users 4 --duration 120 --serial COM9
```

Optional serial capture uses the **native USB log port**, never the COM8 flashing
bridge. If the board is downstairs without a serial connection, omit it. HTTP
alone cannot establish whether a timeout was OOM, a deadlock, a radio failure,
or connection saturation. This firmware's crash record does not survive reboot.
Temperature is not exposed, so reports cannot diagnose thermals.

## Evidence and reports

Each run gets its own gitignored `reports/<timestamp>-<scenario>/` directory:

- `index.html`: offline overview, latency/heap plots, stages, incidents, checks.
- `locust.html`: Locust's endpoint report; generated on normal Locust shutdown.
- `locust_*.csv`: endpoint totals, failures, and full time-series history.
- `events.jsonl`: flushed request starts/completions, response health, diagnostics,
  anomalies and recovery observations. Survives interruption.
- `meta.json`, `summary.json`, optional `serial.log`.

Telemetry calls `/diag` about every five seconds, separately from workload
statistics. It adds real traffic (including a connection) and can be adjusted
with `--monitor-interval`. Free heap from ordinary response headers provides
additional samples without additional requests. The `blk` metric is an average,
**not the largest allocatable block**; reports do not equate it with fragmentation.

Three consecutive failed diagnostic probes stop load. Uptime decreasing,
invalid ledger invariants, stale authentication, or multiple transaction IDs
for the same replay key also stop it. After load stops, bounded probes check
for recovery without rebooting. A timeout is labelled **unresponsive**, not
**crashed**. Low free memory is recorded but intentionally does not stop load.
The subprocess also has an outer duration budget to bound hung harness runs.

For fair comparisons, keep data size, uptime/heap baseline, waits and polling
interval comparable. A suite on an aging board measures cumulative effects;
use reboot + preparation between runs to isolate fresh-board behavior. The suite
never performs that reboot for you. E2E checks should run with unrelated traffic
stopped because they assert exact balance changes.

The implementation uses Locust's documented [event hooks](https://docs.locust.io/en/stable/extending-locust.html)
and [HTML/CSV reporting options](https://docs.locust.io/en/stable/configuration.html).
