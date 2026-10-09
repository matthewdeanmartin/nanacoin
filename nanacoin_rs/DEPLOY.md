# Deploy Rust NanaCoin and the Angular app to a bank

## October 7, 2026: optional COBOL bank deployed to P4

The owner attached the P4 and explicitly requested a COBOL deployment. COM19
independently identified ESP32-P4 revision 1.3, MAC `e8:f6:0a:e3:6f:a2`.
The current working tree was built with the normal board profile, existing
Wi-Fi configuration and static GnuCOBOL module. No emulator networking, clock,
revision, debug or SDIO experiments were enabled. The default Rust build
remains available separately.

```powershell
python nanacoin_rs/cobol/build_firmware.py --board p4 --engine cobol --target-dir C:/nc-cob-p4 --static-module .local/cobol-target-esp32p4
& C:/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe nanacoin_rs/scripts/deploy.py --board p4 --port COM19 --image C:/nc-cob-p4/riscv32imafc-esp-espidf/release/nanacoin-esp32.bin
& C:/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe nanacoin_rs/scripts/probe-board.py --board p4 --address 192.168.1.164
```

Image: 4,009,632 / 8,388,608 bytes, SHA-256
`85b302b213bd6cc16d2de1919828aba925f53c3a8f15bb67eb7b03cc4d799bfe`.
Ten deployment safety tests and the image dry run passed. The writer verified
the chip, recorded MAC and exact partition table, then hash-verified an
application-only write at `0x10000`. Erasure ended at `0x003e2fff`; ledger,
NVS, bootloader, partition table, certificates and C6 firmware were retained.

Fresh UART boot confirms `Banking engine: cobol` and ready at
`https://nanacoin-p4.local` after 10,246 ms. The strict live probe passed
hostname/CA TLS, all 75 exact bundled assets in identity/gzip forms, ETags,
concurrent keep-alive and slow-reader checks, public APIs, balanced ledger,
health and matching CA. Status remained sequence 1, one member, zero
transactions, epoch 0 and journal generation 0; circulation also matched.
No financial transaction, provisioning or economy reset was performed.

At 95 seconds uptime diagnostics reported 317 requests, zero request errors
and 259,671 bytes minimum free internal heap. PSRAM minimum was 30,050,920
bytes; UART stack samples showed 22,500 bytes free for main/serving and 22,104
for TLS. An 85-second fresh-boot UART capture found no panic or actual RPC
timeout. It retained the existing companion-version mismatch warning and two
startup `system_api` MAC-type diagnostic errors; these did not prevent network
association or the strict probe. No C6 upgrade was attempted.

This verifies real P4 startup and HTTPS/asset-load operation with COBOL; it
does not claim hardware financial writes, maximum-capacity bank workloads,
power-cut recovery or browser visual testing. The previous emulator
pre-listener failure is not reproduced on this board. Ignored evidence:
`.local/p4-cobol-{deploy,boot,probe}.log`, pre/post status snapshots and
`.local/firmware-cobol-p4/report.json` at the repository root.

## October 5, 2026: P4-WIFI6 first installation

The owner supplied Meshnology ESP32-P4-WIFI6 / ESP32-C6-MINI-1 markings.
Esptool identification on CH343 COM19 confirmed ESP32-P4 revision 1.3,
MAC `e8:f6:0a:e3:6f:a2`, and 32 MiB flash. With the owner's explicit
permission, the factory `phone_p4_function_ev_board` demo was erased and
NanaCoin installed. No other board was flashed. No physical buttons were needed.

The `p4` profile uses `nanacoin-p4.local`, Rust target
`riscv32imafc-esp-espidf`, and full-size bank limits. The layout has an
8 MiB application at `0x10000` and 16 MiB ledger at `0x810000`.
Boot confirmed 32 MiB PSRAM. IDF 5.5.3 selects the supported 360 MHz setting
for revision 1.3; 400 MHz on revisions below 3.0 requires separately qualified
chips. The optional diagnostic LED is disabled.

Wi-Fi uses pinned `esp_wifi_remote` 1.6.5 and `esp_hosted` 2.11.0 through SDIO
slot 1: CLK18, CMD19, D0..D3 on 14..17, C6 EN54, matching the
[P4-WIFI6 schematic](https://files.waveshare.com/wiki/ESP32-P4-WIFI6/ESP32-P4-WIFI6-datasheet.pdf).
The existing C6 firmware is retained. It reports version `0.0.0`, causing an
ESP-Hosted host/companion mismatch warning recommending a companion upgrade
to avoid RPC timeouts. Association and strict network probes succeeded;
a subsequent 35-second passive UART observation found no RPC timeout or
disconnect warnings. This verifies present operation, not long-term firmware
compatibility. No companion OTA was performed.

The P4 acquired `192.168.1.164`; `nanacoin-p4.local` resolved to that address.
The new TLS leaf is signed by the existing household CA; no new CA was created.
The first live probe passed strict hostname/CA TLS, P4 identity, balanced ledger,
all 75 exact bundled assets (identity/gzip/ETags), concurrent keep-alive,
slow-reader checks, public notebook, health and matching `/ca`.
The bank was unprovisioned, sequence 0, with zero users and transactions.
The owner creates the new household through the UI; no data was migrated or seeded.

Live health testing exposed missing RSSI samples being counted as Wi-Fi
transitions. RSSI can be unavailable during station-lock contention. Incident
connectivity now uses actual framework events, and a regression test alternates
missing/present telemetry while checking real disconnect/reconnect events.
It passed in default, S2 and P4 profiles; P4 strict lint passed.

Final image: `C:/ncr-p4/riscv32imafc-esp-espidf/release/nanacoin-esp32.bin`,
3,684,608 / 8,388,608 bytes, SHA-256
`ceb5c8f13dfc2464fc465d0952256eb8c2f8964ce4f1052c3e3cf61e87c539dc`.
The bootloader, table and initial provisioning command are in ignored
`.local/p4-prepared.json`. The initial writer independently checked chip, MAC
and the factory table, then erased only this P4 and hash-verified the
bootloader at `0x2000`, table at `0x8000`, and application at `0x10000`.
The health fix was installed with the app-only writer, with independent MAC
and exact partition checks and a verified write hash. The final live probe
passed again; the household remained empty and unprovisioned, with zero
request errors after more than 300 requests. The final image uses 43.9% of
its application partition.

Pre-install validation passed: NanaCoin default/S2 checks and HTTP/restart smoke,
P4 desktop tests and strict lint, ten offline deployment tests, certificate
validation, release build, framework `make check`, Minicloud tests/HTTP/MQTT
smoke, and mastomini tests/smoke/client/conformance. Mastomini's fingerprint
check was rerun after framework sources stopped changing. Logs are `.local/p4-*`
in the relevant repositories. Browser visual checks were not performed.

## October 4, 2026: NanaCoin runs on miniframework (S3 deployed)

**S3 deployed October 4, 2026** with `HTTP2=1` (HTTP, HTTPS/1.1 and
HTTP/2; ALPN offers `h2`). COM9, MAC `ac:a7:04:2c:2c:04` confirmed by the
uptime test, dry run clean, image 3,783,120 / 4,194,304 bytes, s3 partition
layout verified, app-only write hash-verified. Strict probe passed at
192.168.1.158 (balanced ledger, 75 exact assets). The Martin House survived:
5 users, 17 transactions, sequence 74. Internal free heap after boot 89.9 KB
(old firmware ~67 KB). Browser checks not performed. The transport baseline
taken before the upgrade (`nanacoin_load/baselines/`) is for the
`uv run ncbench --label s3-after --modes h1 h2` comparison, run with the
board back in its usual spot. The S2 has not been flashed with it.

The remainder of this section was written before the deployment.


The HTTP/HTTPS server, static files, `/trust`, `/ca`, `/metrics` and the
board runner (Wi-Fi, SNTP, mDNS, task placement) now come from
`../../microcontroller/miniframework`; NanaCoin keeps the ledger, its API,
its RGB/Morse light, incidents and Board Health. See
[spec/MINIFRAMEWORK_MIGRATION.md](spec/MINIFRAMEWORK_MIGRATION.md). Both
bank images compile; **neither board has been flashed with it**. It is an
ordinary app-only upgrade: same partition tables, ledger untouched.

What a deployment check should now expect:

- Startup step numbers changed (table under "The S2's blue LED"):
  Wi-Fi now comes up before the ledger opens, so `http://<ip>:8080/` explains
  any failure from step 5 on, including ledger and journal errors.
- `/metrics` is miniframework's line, with the same field names:
  `board,host=nanacoin.local,app=nanacoin,bank=s3 ...` (tag order changed;
  Influx tags are unordered). It adds `tls_open`, `http_open`, `rejected`
  and drops NanaCoin-only `tasks` and `sampler_stack_free_min`.
- `/api/v1/sys` and `/api/v1/log` exist (framework built-ins). `HEAD` of a
  bundled file is 200 without a body (was 405). CORS preflight is 204.
- A refused request (body over 1 KiB, bad framing) carries CORS headers and
  the connection lingers briefly so the client reads the error.
- The deploy checks and probe are miniframework's `tools/boardsafe`;
  `scripts/deploy.py`, `firmware-image.py` and `probe-board.py` bind them to
  `scripts/boards.py`. Same commands, same refusals. The probe also checks
  that `/api/v1/sys` reports `app=nanacoin` on this bank's hostname.

## October 3, 2026: current Angular UI deployed to S3

The owner authorized deployment to the connected board. Windows showed only
the S3 bank on COM9, native USB MAC `ac:a7:04:2c:2c:04`. esptool independently
confirmed that MAC and ESP32-S3; reading the MAC reset the live bank's uptime
from 6,395 to 15 seconds, tying the USB port to `192.168.1.158`.

Built the current working tree with `bash scripts/deploy.sh s3 COM9 --dry-run`,
then deployed that same prepared image with the underlying `deploy.py` writer.
The application is **3,665,200 / 4,194,304 bytes**, SHA-256
`92b299ce33d5e769bb732d634c13aa01e737e3fd99c7bdc21a114df2ec8175d7`.
The writer verified the MAC and exact S3 partition layout, wrote only the app
at `0x10000` (erased app sectors `0x10000–0x38efff`), verified its hash and
restarted the board. No provisioning, economy reset or certificate rotation
was performed. Existing working-tree changes were retained.

Verification passed:

- Angular live and demo coverage suites, Rust default and S2 coverage suites,
  and all nine deployment-safety tests passed before deployment.
- The strict live probe passed CA and hostname validation, correct S3 identity,
  all **75 exact bundled assets** in identity/gzip forms, ETags, concurrent
  keep-alive and slow-reader checks, public notebook, Board Health and `/ca`.
- Private pre/post status and public-ledger snapshots matched exactly; the
  ledger remained balanced. No financial transaction was created for testing.
- Both HTTP and trusted HTTPS `/metrics` returned 200 with
  `board,app=nanacoin,bank=s3,host=nanacoin.local`.
- The browser connector exposed no browsers, so manual navigation and signed-in
  product checks were not performed. The strict probe verified the live assets
  and public endpoints; it does not claim browser interactions were clicked.
- S2 was not connected or flashed.

Notes from this deployment:

- Moving Go into `../archive/nanacoin_go` does not change the active firmware
  workflow. The build still obtains ignored Wi-Fi configuration from
  `../nanacoin_web/config.py`; no credential values should be printed or copied
  into this document.
- The live production UI is 521.29 kB initially: the 500 kB advisory warning
  remains, but the build passes its 1 MB error limit. Coverage tooling is a
  development dependency and is not included in the board's application assets.
- The build reused and validated the existing S3 certificate/key. A direct
  prepared-image deployment avoids an unnecessary second firmware build.

Evidence stays in ignored `.embuild/current-deploy-build-s3.log`,
`current-deploy-write-s3.log`, `current-deploy-probe-s3.log` and the
`current-deploy-before-s3.json` / `current-deploy-after-s3.json` snapshots.
Keep the snapshots private; do not print their household data.

## October 3 observability fix (deployed to S3)

The live S3 bank returned 404 at `http://nanacoin.local/metrics`. The source
had an Influx health handler, but the bundled website handler answered a
missing-asset 404 before it could run. The metrics handler now runs first,
before website routing and HTTP onboarding restrictions, without taking the
ledger lock. Static routing also reserves `/metrics` for the health handler.
The endpoint reports machine facts, not financial records or credentials.

Deploy this fix using the existing **S3 app-only upgrade** after identifying
MAC `ac:a7:04:2c:2c:04`; preserve the ledger, credentials and certificates.
Do not infer that a desktop gate proves the firmware is live. After deployment,
require HTTP 200 and a `board,app=nanacoin,bank=s3,host=nanacoin.local` Influx
line from `/metrics`, in addition to the normal strict board probe.

The initial desktop check encountered a running server holding
`target/debug/nanacoin.exe`; checks were rerun with
`CARGO_TARGET_DIR=target-observability`, leaving that server running.
Those full checks passed, including both bank capacity profiles and the HTTP
restart smoke. The S3 app is 3,558,896 / 4,194,304 bytes.
Logs: `.embuild/observability-check.log` and
`.embuild/observability-firmware-s3.log`.

### October 3, 2026: S3 deployment verified

- Owner attached NanaCoin and authorized deployment. Windows native USB
  COM9 identified `ac:a7:04:2c:2c:04`; the writer independently verified
  ESP32-S3, that MAC and the exact S3 partition layout before writing.
- Reused the prepared image through `deploy.py` dry-run and write, without
  recompiling. App SHA-256:
  `340f97e66fb4d72e6fcef52684fd75f38665f86c3d9561d65534e096e82fcba0`.
  Application hash verified; automatic reset completed.
- Only `0x10000–0x374fff` was erased/written. NVS, ledger at `0x410000`,
  bootloader, partition table and existing certificates were preserved.
  Private pre/post public-ledger snapshots matched exactly and the ledger
  remained balanced. No payment, reset, seeding or credential change.
- Strict S3 live probe passed at `192.168.1.158`: hostname-verified TLS,
  correct bank, exact bundled assets, public notebook, Board Health and CA.
  Product navigation was not manually clicked during this deployment.
- `http://nanacoin.local/metrics` returned 200 with
  `board,app=nanacoin,bank=s3,host=nanacoin.local`. Housemetrics confirmed
  a successful configured scrape: 32 samples, empty `last_error`.
- Logs: `.embuild/observability-deploy-s3.log` and
  `.embuild/observability-probe-s3.log`. Private ledger evidence stays in
  the ignored `.embuild` directory; never print its contents.
- S2 bank was not attached or flashed in this deployment.

This household runs **two independent NanaCoin banks** (see
`spec/SECOND_BANK.md`). Each has its own board, hostname, certificate,
firmware build, ledger and currency. **Every command below names the board.**
There is no default board, and a deployment for one bank cannot be written to
the other.

| | **S3 bank** (`s3`) | **S2 bank** (`s2`) |
|---|---|---|
| Address | `https://nanacoin.local` | `https://nanacoin-s2.local` |
| Board | ESP32-S3-N16R8 | ESP32-S2 Mini (S2FN4R2) |
| Chip / MAC | `esp32s3`, `ac:a7:04:2c:2c:04` | `esp32s2`, `80:65:99:f0:1c:9c` |
| USB | native USB (`VID_303A&PID_1001`, COM9 in Sept 2026) + CH343 bridge | native USB only (`VID_303A&PID_0002`); **the COM number moves on every reset** |
| Known IP | 192.168.1.158 | 192.168.1.157 (September 28, 2026) |
| Flash / app partition | 16 MiB / 4 MiB at `0x10000` | 4 MiB / 2.375 MiB at `0x10000` |
| Build profile | `sdkconfig.defaults`, `partitions.csv` | `boards/s2/sdkconfig.defaults`, `boards/s2/partitions.csv`, `--features board-s2` |
| Cargo target dir | `C:/ncr` | `C:/ncr-s2` |
| Web bundle | `.embuild/web` (identity + gzip) | `.embuild/web-s2` (gzip only) |
| Server certificate | `certs/nanacoin-ca-signed.*` | `certs/nanacoin-s2-ca-signed.*` |

Both leaves are signed by the one household CA (`certs/home-ca.crt`), so a
device that trusts one bank trusts the other. `scripts/boards.py` is the single
source for the table above; the scripts read their board from it.

Which CA that is, is a choice. By default NanaCoin makes and uses its own
(`.local/ca`). A household that already runs a CA can sign with it instead, so
its devices trust one CA for everything: `make adopt-ca CA_DIR=<dir>`, where
the folder holds `rootCA.pem` and `rootCA-key.pem`. The key stays in that
folder; the choice is remembered in `.local/ca-dir`; the previous CA and leaves
are archived under `.local/cert-backups/`. `make adopt-ca CA_DIR=.local/ca`
goes back to NanaCoin's own. This household adopted mastomini's CA
(`../../mastomini/mastomini_rs/.local/ca`, SHA-256 `C2:8F:EE:1E:…:54:FD`) on
October 1, 2026; the S3 was redeployed with it, and the S2 leaf was re-signed
too and switches at its next deployment. A board serves a new certificate only
after it is deployed.

## How the two banks are kept apart

A deployment must pass **all** of these checks before it writes anything, and
the probe must pass afterwards. Each check alone would stop a cross-board flash.

1. **Board is required.** `deploy.sh`, `provision.sh`, `build-esp32.sh`,
   `probe-board.py` and the Make targets refuse to run without `s3` or `s2`.
   The old form `bash scripts/deploy.sh COM9` is now an error.
2. **The image names its board.** Each firmware image embeds
   `NANACOIN-BOARD:<board>:<hostname>;`, and its header carries the chip ID
   (9 = S3, 2 = S2). `firmware-image.py`/`deploy.py` refuse an image that lacks
   the board's marker, carries the other board's marker, has the wrong chip ID,
   or is larger than that board's application partition.
3. **esptool uses the board's `--chip`.** An S2 image cannot be written to an
   S3, or the reverse: esptool refuses a chip-type mismatch.
4. **The MAC must be that bank's recorded board.** If the chip is the other
   bank, the error says so ("Port is the s3 bank ..."). If a board is
   deliberately replaced, update `mac` in `scripts/boards.py` in a reviewed
   change first.
5. **The partition table must be that bank's layout.** The other bank's layout
   is named in the refusal.
6. **The live probe checks identity.** `/api/v1/diag/static` reports `board`
   and `hostname`; the probe fails if they are not the requested bank, and TLS
   is verified for that bank's hostname only.

Separate target directories, bundle directories, certificates and sdkconfig
files mean one board's build products are never picked up for the other.

## Rules for humans and automation

- Work from `nanacoin_rs` in Git Bash on Windows.
- Record `git status --short` before building. A dirty tree is allowed; do not
  discard, reset, stash, or overwrite somebody else's changes.
- Decide which bank you are deploying **before** looking for a port. Write it
  in the deployment report.
- Never run `erase_flash`, erase NVS, write the partition table, flash a merged
  image, reset the economy, rotate certificates, or use HTTP recovery as part
  of an ordinary deployment. First installation (`provision.sh`) is a separate,
  explicitly requested operation.
- Never print Wi-Fi passwords, private keys, tokens, or credential-file contents.
- Do not run `nanacoin_web/deploy.ps1` or `deploy.sh` afterward. The Rust image
  already contains Angular.
- If the serial port or target board is ambiguous, stop and ask. Do not guess.
- If an identity check (image, chip, MAC, partition table, probe) refuses the
  board, stop. Do not bypass it, edit `boards.py` to make it pass, or "fix"
  the layout during a routine deployment.
- A successful build or flash is not completion. The strict live-board probe
  for **the same board** must pass.

## Prerequisites

The build computer needs:

- the repository's existing Espressif Rust/ESP-IDF environment (the `esp`
  toolchain provides both `xtensa-esp32s3-espidf` and `xtensa-esp32s2-espidf`);
- Node dependencies in `../nanacoin_ui/node_modules` (run `npm ci` there
  once if they are absent);
- ignored Wi-Fi configuration supported by `build.rs` (both banks join the
  same network);
- the existing ignored CA under `.local/ca`, `certs/home-ca.crt` and the
  board's leaf certificate. `scripts/dev-certs.sh s2` signs a missing S2 leaf
  with the existing CA; it never creates a second CA;
- Python `esptool` from the ESP-IDF environment;
- a USB **data** cable connected to the intended board.

Do not regenerate or rotate working certificates just to deploy. Rotation would
make every household device trust a new CA.

`NANACOIN_STATUS_LED_PIN` in `.env` applies to the S3 only; S2 builds force it
`off` because the S2 Mini has no WS2812 pixel.

## 1. Identify the serial port

In PowerShell, list present ESP32 devices without opening them:

```powershell
Get-CimInstance Win32_PnPEntity |
  Where-Object { $_.Name -match 'COM\d+' -or $_.PNPDeviceID -match 'VID_303A|VID_1A86' } |
  Select-Object Name, Status, PNPDeviceID |
  Format-Table -AutoSize
```

Read the **product ID and MAC** in the `USB Composite Device` row, not the
Windows device name:

| Row | Meaning |
|---|---|
| `USB\VID_303A&PID_1001\AC:A7:04:2C:2C:04` | the S3 bank; its `USB Serial Device (COMn)` with the same instance prefix is the port |
| `USB\VID_303A&PID_0002\80:65:99:F0:1C:9C` | the S2 bank; its `MI_00` serial row is the port |
| `VID_1A86&PID_55D3` | the S3's CH343 bridge (not used for deployment) |
| `ESP32-S2` row with status `Error` (`MI_02`) | the S2's debug interface without a driver; harmless |

COM3 on this PC is the motherboard's Intel AMT port; ignore it. Close serial
monitors and other programs holding the port before continuing.

If **both** boards are plugged in, confirm you picked the row whose MAC matches
the bank you are deploying. The deploy script checks the MAC again, but do not
rely on it to choose for you.

### The S2: download mode by hand, RST afterwards

The S2 has no bridge chip. Its USB port is presented by whatever runs on it:

| What runs | What Windows shows |
|---|---|
| ROM download mode | `USB Serial Device (COMn)`, serial number `0` (COM4 on this PC) |
| NanaCoin on the S2 | usually **"USB device not recognized"** and no COM port. This is the current, known state: the S2 app has no usable USB console. Use the blue LED, `/api/v1/diag` and the port-8080 report (below) instead. |

Every S2 deployment therefore starts from ROM download mode, entered by hand:

1. **Unplug** the S2. **Hold BOOT** (`0`). **Plug in** while holding it.
2. Wait 2 seconds, **release BOOT**. The blue LED stays **dark**.
3. List ports; the port with serial `0` is the one to use.

"Hold BOOT, tap RST, release BOOT" also works, but did not always take on
September 28, 2026; the power-on form was reliable. If the LED is still
blinking, the board is not in download mode.

`deploy.py`/`provision.py` never reset the S2 into download mode themselves
(`--before no_reset`). Within one deployment they wait for the port when it
briefly re-enumerates between esptool sessions, verify MAC and layout, write,
and ask for a watchdog reset. **After an S2 write, tap RST once** if the LED
stays dark: the watchdog reset can leave the S2 in download mode. If esptool
cannot connect at all, repeat the power-on routine rather than retrying the
command.

### The S2's blue LED

The S2 Mini's only light is a small blue LED on GPIO15 (there is no power
lamp; no light at all means no power, a charge-only cable, or download mode).

| Light | Meaning |
|---|---|
| On 1.5 s, gap, then 1–6 quick flashes | Firmware started; flashes = previous reset (1 power-on, 2 reset/USB, 3 crash, 4 watchdog, 5 brownout, 6 other) |
| Slow: 1 s on, 1 s off | Joining Wi-Fi |
| Solid on | Wi-Fi up, server starting |
| Morse rotation and three blinks | Healthy and serving (same messages as the S3's RGB light) |
| Mostly on: 1.75 s on, 0.25 s off | Serving but degraded (mDNS failed or a recent allocation/TLS error) |
| Fast, 5 per second | Server stalled or storage failed |
| **N blinks, 1.6 s dark, repeat** | Startup failed at step N. miniframework's steps: 1 system/event loop, 2 system NVS, 3 Wi-Fi driver, 4 Wi-Fi join, 5 time/mDNS, 6 app setup, 7 web server. NanaCoin's (between 6 and 7): 8 ledger partition, 9 journal storage, 10 ledger replay, 11 background tasks |

For a startup failure at step 5 or later, Wi-Fi is already up and the board
keeps it up: `curl http://<board-ip>:8080/` returns the exact error with
internal-RAM and PSRAM figures (both boards). Find the IP from the router, or
ping-sweep and look for the MAC in `arp -a`.

### Prove the port is the live S3 bank

"A board is plugged in" does not mean "the NanaCoin server is plugged in". On
September 26, 2026 the only USB board present (COM11) was a different
ESP32-S3 with an unrelated partition table (`store`, `media`, `coredump`),
while the real server kept running untouched over Wi-Fi. The partition check
caught it, and the MAC check now also would, but identify the board before
relying on either:

1. Note the live server's uptime:
   `curl -s http://<board-ip>/api/v1/diag` → `uptime_seconds` (`/diag` is
   served over HTTP too; find `<board-ip>` as in step 4).
2. Read the attached chip's MAC. This resets that board:
   `python -m esptool --chip esp32s3 --port "$PORT" read_mac`
3. Read `uptime_seconds` again, retrying for about 30 seconds while it boots.
   If it restarted near zero, the port is the server. If it kept counting, the
   port is **another board**: stop and ask. Do not deploy to it.

The same test works for the S2 bank: in download mode the S2 is not serving,
so its `/api/v1/diag` stops answering until it restarts.

Comparing the esptool MAC with the router or ARP table is only a hint. An ARP
entry in `Probe`/`Stale` state may be old. The uptime test is decisive.
Charge-only USB cables power a board without creating a serial port, so a
powered server can still be absent from the port list.

## 2. Record the tree and run a dry deployment

Open Git Bash:

```bash
cd /c/github/nanacoin/nanacoin_rs
git status --short
BOARD=s3                          # s3 = nanacoin.local, s2 = nanacoin-s2.local
PORT=COM9                         # the port found above for THAT board
bash scripts/deploy.sh "$BOARD" "$PORT" --dry-run
```

The shell wrapper dry run does all builds but never opens the port. It must:

1. build the Angular production app;
2. package that board's assets (`.embuild/web` or `.embuild/web-s2`);
3. validate that board's TLS certificate/key against the household CA;
4. build that board's release firmware;
5. check the application `.bin`: chip ID, board marker, and size against that
   board's application partition; and
6. print the board, hostname, chip, expected MAC, and a plan to verify the
   chip, MAC and partition table and write only address `0x10000`.

Warnings are not automatically failures, but any nonzero exit is. Fix the
specific failure; do not skip its check.

### Reuse a prepared build

Build once, then run the underlying Python checker and writer against that
same image. The `deploy.sh` wrapper rebuilds on each invocation, including
`--dry-run`; do not repeat it when the checked images already exist and no
source, config, certificate or web input has changed.

```bash
bash scripts/build-esp32.sh "$BOARD"       # once; omit for a current prepared build
ESP_PY=C:/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe
IMAGE=C:/ncr/xtensa-esp32s3-espidf/release/nanacoin-esp32.bin  # S3 only
# S2 instead: C:/ncr-s2/xtensa-esp32s2-espidf/release/nanacoin-esp32.bin
uv run --no-project --python "$ESP_PY" python scripts/deploy.py \
  --board "$BOARD" --port "$PORT" --image "$IMAGE" --dry-run
uv run --no-project --python "$ESP_PY" python scripts/deploy.py \
  --board "$BOARD" --port "$PORT" --image "$IMAGE"
```

The direct writer enforces the same chip, image marker, MAC and partition
checks. Continue with the strict live probe in step 5. Neither dry-run
success nor serial hash verification replaces that probe.

## 3. Deploy the same source tree

With the intended board attached (the S2 in download mode) and the port free:

```bash
bash scripts/deploy.sh "$BOARD" "$PORT"
```

The deployment script rebuilds, then in one esptool session:

1. connects with the board's `--chip` and reads the chip MAC;
2. refuses unless the MAC is that bank's recorded board;
3. reads 4 KiB at `0x8000` and requires exactly that bank's `nvs`,
   `phy_init`, `factory` and `ledger` layout;
4. writes the new application image at `0x10000`; and
5. lets the board restart.

Expected final text:

```text
Application updated on the s3 bank. Open https://nanacoin.local/ after restart.
```

or, for the S2, `... on the s2 bank. Open https://nanacoin-s2.local/ ...`.
That sentence proves only that esptool completed. Continue to verification.

## 4. Find the board address

First try the bank's mDNS name:

```powershell
Resolve-DnsName nanacoin.local       # S3 bank
Resolve-DnsName nanacoin-s2.local    # S2 bank
```

If mDNS is unavailable on the build computer, use the board's known DHCP address
from the router or its prior deployment record. An IP address is fine for the
probe; the probe still sends the bank's hostname for TLS SNI and hostname
validation, and it checks that the board at that address really is that bank.

```bash
ADDRESS=192.168.1.158             # example only; use the actual address
```

## 5. Prove the live board is correct

```bash
python scripts/probe-board.py --board "$BOARD" --address "$ADDRESS"
# equivalent: make probe-board BOARD="$BOARD" ADDRESS="$ADDRESS"
```

It retries during boot, trusts only `certs/home-ca.crt`, verifies the certificate
for that bank's hostname, and checks:

- TLS succeeds without `-k` or a warning bypass;
- `/api/v1/diag/static` reports the requested `board` and `hostname` (the
  other bank at this address fails with "wrong bank");
- `/api/v1/status` returns JSON and says the ledger balances;
- `/` serves the **exact Angular index produced by this board's build**,
  configured for the same-origin API;
- every bundled asset matches the build byte-for-byte, with correct lengths,
  ETags and conditional responses, across concurrent keep-alive connections;
  the largest asset also passes a slow-reader check. On the S3 both identity
  and gzip are checked; the gzip-only S2 must return the exact gzip bytes and
  answer an identity-only request with 406;
- the anonymous public notebook returns a correctly shaped ledger response;
- anonymous Board Health returns current machine data;
- the served public CA is byte-for-byte the one used for the build.

The command must end with `Board probe passed for <board> (<hostname>, ...)`.
A DNS response, ping, serial boot line, HTTP 200 alone, or browser certificate
bypass is not equivalent.

When a browser surface is available, perform these short product checks in a
browser that already trusts the CA, on **that bank's** hostname:

1. Open `https://<hostname>/?api=` in a private window. The Notebook should
   load without signing in because the ledger is public.
2. Open **Board Health** without signing in. It should show one fresh reading
   naming the right platform (ESP32-S3 or ESP32-S2), say **Start live
   updates**, and remain paused until selected.
3. Sign in and confirm the existing household and balances survived. The two
   banks have different households; seeing the other bank's household means
   the wrong address was used.
4. Open Household and My Account to confirm the section navigation renders.

Do not create a payment, reset the economy, close the journal, or seed demo data
merely to prove deployment.

For non-interactive automation with no browser surface, the strict probe is the
required substitute. Report the browser checks as unavailable rather than
claiming they were clicked.

## First installation of the S2 bank (provisioning)

Only for a board that is **not yet a NanaCoin bank** (the S2 Mini arrived with
MicroPython). This erases the whole chip, including anything the old firmware
stored, then writes the bootloader (`0x1000` on the S2), partition table
(`0x8000`) and application (`0x10000`). It is never part of a routine upgrade.

```bash
bash scripts/provision.sh s2 "$PORT" --dry-run   # builds and checks inputs only
# BOOT+RST into download mode, find the port again, then:
bash scripts/provision.sh s2 "$PORT"
# equivalent: make provision BOARD=s2 PORT="$PORT"
```

`provision.py` refuses:

- a chip whose type or MAC is not the recorded S2 (the S3 bank is named if it
  is the S3's MAC);
- a chip that already has **any** NanaCoin bank layout, because that chip holds
  a ledger. Upgrade it with `deploy.sh` instead. `--replace-existing-bank`
  exists only for an owner's explicit decision to discard that bank's ledger;
  automation must never add it on its own;
- a bootloader, partition table or image that is not the S2 build.

After provisioning:

1. The board restarts, joins Wi-Fi and advertises `nanacoin-s2.local`.
2. Open `http://nanacoin-s2.local/trust`. Devices that already trust the
   household CA need nothing new.
3. Open `https://nanacoin-s2.local/` and create the second bank's household,
   its administrator and **its own currency name**. Do not reuse the S3's
   currency name; the two banks' coins are different currencies.
4. Run the strict probe with `--board s2`.
5. Record the board's DHCP address here for future probes.

## Stop conditions and what they mean

| Symptom | Action |
|---|---|
| No board argument, or an unknown one | Decide which bank. Do not pick a default. |
| No unambiguous serial port | Stop; reconnect/identify the physical board. |
| Port access denied/in use | Close monitors and retry. Do not change flash commands. |
| "Port is the s3 bank" / "Port is the s2 bank" | You have the other bank's port. Stop and pick the right board; never switch `BOARD` to match the port you happen to have. |
| MAC "is not the recorded" board | Another board entirely. Stop and ask. |
| esptool reports a different chip type | Wrong board for this `BOARD`. Stop. |
| S2: esptool cannot connect / no port | Board not in download mode or charge-only cable: the power-on BOOT routine, list ports again. Do not retry in a loop. |
| S2: "USB device not recognized" | Normal while NanaCoin runs on the S2. Not a fault by itself; use the LED and network diagnostics. |
| S2: LED dark after a deployment | Still in download mode after the write. Tap RST once. |
| LED counts N blinks | Startup failed at step N; read `http://<ip>:8080/` for step 5 and later. |
| Probe: board `None`, "wrong bank" on the S3 | The S3 still runs firmware from before the two-bank change, which does not report its board. Deploy the current S3 build first. |
| Partition-layout refusal | Stop. This is not an upgradeable bank of that type, and often not the NanaCoin server at all. |
| Image lacks/has another board marker, wrong chip ID | The wrong build was selected. Rebuild with the right board. |
| Live server uptime did not reset after `read_mac` | Stop. The port is a different board. |
| Firmware image too large | Stop and reduce/review the image. Never enlarge or rewrite partitions casually. The S2 app partition is 2.375 MiB. |
| Certificate validation fails | Stop and inspect the existing certificate inputs. Do not use `-k` and do not rotate automatically. |
| Probe: "wrong bank" | The address answers as the other bank. Fix the address; the deployment is unverified until the right bank passes. |
| Flash succeeds but strict probe fails | Treat deployment as unverified; check boot, address, Wi-Fi, TLS, and serial evidence. Do not erase the board. |
| Ledger reports unbalanced | Stop all money-writing tests and preserve evidence. |
| Household appears empty | Stop. Do not provision or seed; verify that the intended board/address was used. |

## Equivalent Make targets

After the board and port are known:

```bash
make deploy BOARD=s3 PORT=COM9
make probe-board BOARD=s3 ADDRESS=192.168.1.158
make firmware BOARD=s2            # build only, no board access
make test-s2                      # Rust tests with the S2 capacity profile
```

Every board-specific target fails without `BOARD`. The explicit
`bash scripts/deploy.sh` form is used above because it exposes the safe
`--dry-run` step.

## Completion report

A deployment report should state:

- **which bank** (`s3`/`s2`, hostname) was deployed;
- source working tree/revision and whether it was dirty;
- selected serial port and how it was identified (including the MAC);
- dry-run result;
- MAC, partition verification and application write result;
- board hostname/IP used for verification;
- strict probe result, including the reported board identity and
  balanced-ledger confirmation;
- whether the four browser checks passed;
- any checks not performed and why.

Do not include credentials, tokens, private-key material, or household transaction
contents in the report.

## S2 bank history

### September 28, 2026: first installation of the S2 bank

- Board: ESP32-S2FNR2, MAC `80:65:99:f0:1c:9c`, previously MicroPython.
  Provisioned with `provision.py` (identity checks passed, whole-chip erase,
  bootloader/partition table/app). The first boot opened a blank ledger and
  retried Wi-Fi; the USB console then stopped working for later builds.
- Diagnosis used the new LED step codes and the port-8080 report:
  `startup failed at step 9: Not enough space (os error 12); internal free
  50431 largest 31744; psram free 1964860`. The S3's layout of worker
  threads needs 24 + 32 + 24 + 4 KiB of *internal* stacks; the S2 had 31 KiB
  as its largest internal block after Wi-Fi.
- Fix (S2 only): TLS handshake, diagnostics and LED task stacks live in PSRAM
  (they never write flash); the HTTP/API multiplexer runs on the main task's
  existing 32 KiB internal stack, with the scheduled-payment tick and
  housekeeping between turns. Logging from the Wi-Fi event callback was also
  removed; the reason and RSSI are recorded and logged by the main task.
- Result: ready at 23.4 s (first Wi-Fi attempt: reason 201, no AP found;
  second connected at −61 dBm on channel 11). `probe-board.py --board s2
  --address 192.168.1.157` passed: strict TLS for `nanacoin-s2.local`, board
  identity, balanced ledger, all 67 gzip-only assets with 406 identity,
  ETags, concurrent and slow-reader transfers, public notebook, Board Health
  and `/ca`. After the probe: internal free 42,791 (minimum 39,463), PSRAM
  free 1,334,344 (minimum 1,205,084), no allocation/TLS/storage incidents.
  mDNS resolved `nanacoin-s2.local`. The household was not yet created.

### September 28, 2026: first two-bank deployment to the S3

- Uncommitted two-bank working tree on top of `0793cb0` (52 changed paths,
  none discarded). COM9 identified by the composite USB ID ending in
  `AC:A7:04:2C:2C:04`; the board had been re-plugged (116 s uptime).
- Baseline: The Martin House, 5 users, sequence 10, 4 transactions, journal
  generation 0, balanced.
- `bash scripts/deploy.sh s3 COM9 --dry-run` passed, then the real deployment:
  chip, MAC and s3 partition layout verified in one esptool session; only the
  3,349,056-byte application was written at `0x10000`, hash verified.
  The S3 restarted on its own (uptime 5 s at first check).
- `probe-board.py --board s3 --address 192.168.1.158` passed, including the new
  board-identity check (`s3`, `nanacoin.local`) and all 67 identity + gzip
  assets. The ledger matched the baseline exactly.
- Cross-checks: `--board s2` against the S3 and `--board s3` against the S2
  both failed at strict TLS (hostname mismatch) before any request.
- New in this build for the S3: a failed startup blinks its step number in red
  and serves the port-8080 report. No browser checks were performed.

## S3 bank history and diagnostics

Everything below records work on the S3 bank (`nanacoin.local`) before the
second bank existed. Commands in it use the old single-board syntax; today the
S3 forms are `bash scripts/deploy.sh s3 COM9` and
`make probe-board BOARD=s3 ADDRESS=...`.

### Optional RGB boot and health diagnostics

Set `NANACOIN_STATUS_LED_PIN=48` (or `38` for that board's documented RGB
pin) in ignored `nanacoin_rs/.env`, then use the ordinary deployment above.
`off`, an absent setting, or an invalid setting disables the LED. GPIO48 is
configured for the attached September 26 board; optical confirmation is still
needed. Never probe arbitrary pins: WS2812 has no acknowledgement, so firmware
cannot detect a missing pixel or validate the physical pin automatically.
Choose `off` before deploying to a board that uses that pin for something else.

The optional low-priority RMT task starts before ledger/incident initialization,
uses dim output (maximum 12/255), and never takes the ledger or network lock.
Driver/task failure disables only the LED. A missing pixel does not prevent the
website from working. The boot marker begins once the application runs, not in
the ROM bootloader; an unpowered/stuck-in-download CPU cannot light it.

| Light | Meaning |
|---|---|
| White for 1.5 seconds, 0.5-second gap, short white flashes | Application started; flashes identify the previous reset. |
| Pulsing blue | Waiting for Wi-Fi/DHCP or reconnecting. |
| Dim white | Wi-Fi acquired; service initialization continues. |
| Cyan Morse message followed by three green blinks | Both real TLS and HTTP workers are making progress, Wi-Fi is up, and mDNS initialized. |
| Amber | mDNS initialization failed (use the IP), or an allocation/TLS initialization error occurred within 10 seconds. |
| Red blinking N times, dark 1.6 s, repeat | Startup failed at step N (same steps as the S2 table above); `http://<board-ip>:8080/` has the error text once Wi-Fi is up. |
| Solid red | Storage failure while running, or a server worker has stopped progressing for 2 seconds. |

Reset flashes: 1 power-on; 2 software/reset-pin/USB; 3 panic; 4 watchdog;
5 brownout; 6 other. A panic may reset before a live red indication is possible.
The heartbeat observes server workers, not every ledger invariant or the
scheduled-payment task; use Board Health and the strict probe for those checks.
A healthy mDNS indicator means initialization succeeded, not that every client
can receive multicast packets.

If only the red power lamp lights, check the USB cable and power connector.
A similar Mastomini board booted only when powered through its other USB port;
the power lamp alone did not prove the application had started.

Capture a restart and boot log with the ESP-IDF Python (no flash write):

```bash
/c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe scripts/boot-log.py --port COM9
```

Use the identified port. `--no-reset` passively listens. The restart uses the RTC
watchdog because ordinary USB reset can leave an S3 in download mode. A ready
boot log supplements, and does not replace, the strict live-board probe.

### September 26, 2026 deployment record

- Started from clean revision `2f010fa`; deployment includes the uncommitted LED
  changes in this working tree. No pre-existing changes were discarded.
- Identified native USB COM9 through MAC `AC:A7:04:2C:2C:04`. The live
  `192.168.1.158` diagnostic uptime reset from 85 seconds to 5 seconds after
  `read_mac`, confirming this was the running NanaCoin board. COM8 was its
  additional CH343 adapter; COM9 was used for deployment.
- Saved only `NANACOIN_STATUS_LED_PIN=48` in the ignored crate `.env`; existing
  Wi-Fi configuration and certificate inputs were reused.
- Dry deployment passed. Ordinary `scripts/deploy.sh COM9` then accepted the
  partition layout and wrote only the 3,213,248-byte application at `0x10000`;
  esptool verified its hash. Ledger/configuration/bootloader/partitions preserved.
- `make check` passed: formatting, clippy, 154 Rust tests, and the HTTP/restart
  smoke. The subsequently added worker-freshness regression also passed
  (155 tests in total); firmware release build and Python boot-helper syntax
  check passed. The regression covers both workers, stale heartbeats,
  concurrent observations, and millisecond counter wrap.
- Serial restart capture: GPIO48 WS2812 initialized at 1,288 ms, reset class
  power-on; server ready at 6,728 ms. No LED driver failure was reported.
  Serial confirms driver operation, not the physical color/pin; owner visual
  confirmation is still pending.
- `make probe-board ADDRESS=192.168.1.158` passed strict TLS for `nanacoin.local`,
  balanced ledger, exact bundled Angular index, anonymous notebook/Board
  Health, and served CA checks. No payments or provisioning were performed.
- Browser click-through checks were not performed; the strict automated probe
  was used instead. No authenticated household/balance view was opened.
- Local evidence: `.local/nanacoin-led-{check,dry,deploy,boot,probe}.log` at the
  repository root. No credentials were included in the deployment report.

### September 26, 2026: browser polling audit

The reported all-page `/status` traffic matched Session's five-second change
watch, plus unthrottled focus/navigation checks. No immediate response-triggered
loop was found. The UI now allows at most one automatic status check per 30
seconds after completion, skips hidden/signed-out sessions, and removes its
timer/listeners on destruction. A changed status is reused for the subsequent
household refresh instead of requesting `/status` twice. Explicit user edits
still refresh immediately. Separate tabs each have their own polling budget.

Fulfillment, loans, lotto, messages, and account commitments also had independent
10/15-second timers. These now run at 30 seconds, skip hidden tabs and in-flight
resource loads, and retain their existing destroy cleanup and ledger-change
refresh. Account commitments can fetch both loans and lottos once per tick.
Server Logs remains opt-in; follow mode is now five seconds, skips hidden tabs,
and never overlaps its own request. Board Health already starts paused, waits
five seconds after a response, pauses hidden tabs, and aborts on page exit.

UI validation: 291 tests passed, including cooldown on success/failure, hidden
state, listener cleanup, and reuse of changed status. Mastomini's own Angular
and server-generated pages had no recurring network timer; bots UI had a real
async-initialization timer leak plus overlapping three-second polling, fixed in
that repository (see its deployment runbook). Browser traffic was diagnosed
from source and the owner's observed five-second cadence, not a captured HAR.

Deployment completed on the same identified COM9 board: dry run and ordinary
application-only flash passed (3,213,920 bytes, verified hash). Boot confirmed
GPIO48 diagnostics and ready at 6,728 ms. Strict probe at `192.168.1.158` passed,
including balanced ledger and exact updated Angular index. Browser manual
checks were not performed. Reload all existing Nanacoin tabs to replace the old
JavaScript timers. Evidence is in `.local/nanacoin-poll-{dry,deploy,boot,probe}.log`.

### Rotating healthy messages

The current firmware rotates three administrator-editable messages from
**Household → Light**. The defaults and timing are documented in README.md.
They are saved as one bounded `ncmeta/led_phrases` setting, separate from the
financial journal. Routine application-only upgrades preserve them. Do not
change live messages or financial records merely to prove a deployment; use
the ordinary read-only boot and probe checks. Startup and faults take priority.

### 2026-09-27: startup stack corruption during Morse deployment

The first application-only upgrade of revision `0cd3993` on COM9
(MAC `ac:a7:04:2c:2c:04`) wrote 3,312,592 bytes at `0x10000` and passed
esptool hash verification. It did **not** complete deployment: the board
repeated its white startup marker and rebooted before Wi-Fi/HTTP readiness.
The ledger partition was not written. No pre-upgrade ledger-count snapshot
was recorded, so the flash result alone cannot establish before/after counts.

Investigation:

1. Read the bounded serial boot capture. GPIO48 initialized, then Wi-Fi task
   creation was followed by a Core 0 `LoadProhibited` panic: PC `0x4037fe10`,
   `EXCVADDR=0x4`. Do not interpret repeated white as a healthy heartbeat or
   evidence that the LED/power circuit is broken.
2. Decode the backtrace with the **matching flashed ELF**, using
   `xtensa-esp32s3-elf-addr2line -pfiaC -e <elf> <addresses>`. The path was
   Wi-Fi initialization → NVS read → flash cache/other-core coordination →
   `esp_ipc_call_nonblocking` → `xTaskGenericNotify`. The faulting instruction
   accessed a null FreeRTOS task-list container; the Wi-Fi frame was where
   prior corruption surfaced, not proof of a bad Wi-Fi password.
3. Inspect startup stack reservations in the same ELF. This toolchain's
   `.xt.prop` metadata made GNU objdump display some Rust code as raw words.
   Create an **analysis-only copy** with
   `xtensa-esp32s3-elf-objcopy --remove-section=.xt.prop <elf> <analysis-elf>`
   and disassemble that copy with `objdump -d -C`. Never flash this modified
   analysis ELF. Address/name lookup must use the exact build, not addresses
   copied from another release.
4. The nested startup chain reserved 38,288 bytes in `nanacoin_esp32::main`
   (32 + `0x9570`), 24,784 bytes in `Service<NvsJournal>::open` (`0x60d0`),
   and 9,136 bytes in `Auth::default` (`0x23b0`): **72,208 bytes**, before
   additional callees, exceeding `CONFIG_ESP_MAIN_TASK_STACK_SIZE=65536`.
   The canary check did not prevent the later task-list crash. A normal
   desktop build/test pass does not establish embedded stack safety.

Fix: keep the large fixed authentication arrays in `Box<Auth>` inside
`Service`, as the domain `State` already is, so constructing/returning the
service and moving it into `Mutex`/`Arc` no longer copies those arrays through
multiple startup stack frames. Account for the boxed allocation in diagnostic
memory totals. This changes RAM placement, not the journal, credentials,
authentication semantics, or ledger schema. Add a regression bound keeping a
representative `Service` below 1 KiB, rerun desktop checks, inspect the repaired
ESP stack frames, then use the ordinary application-only deployment/probe flow.
No erasing, reprovisioning, certificate rotation or larger flash partition is
part of this repair. Recovery verification is recorded below.

The repaired release ELF reduced these three reservations to 1,872 bytes in
`main` (`0x750`), 11,584 bytes in `Service::open` (`0x2d40`), and 9,136 bytes
in `Auth::default`: **22,592 bytes** for the same chain, versus 72,208 before.
This provides substantial room within the existing 64 KiB stack without
reserving more scarce internal RAM. `make check` passed, including the new
service-size regression and the real HTTP restart smoke. The smoke's stale
expectation that other members' account pages return 403 was aligned with the
already-committed household-visible account-page policy; admin-only raw state
checks remain. ESP firmware build passed at 3,311,456 bytes.

Recovery completed by the diagnosing agent on the same COM9 board, without
handing the repair back to the deployment agent. The dry run and ordinary
`bash scripts/deploy.sh COM9` passed. The application-only write at `0x10000`
was 3,311,456 bytes, with esptool hash verification; the image SHA-256 was
`d629181ea017b21947489c7a624defbcdeb7bc3bffd68b770ca713d526f1a2fd`.
The boot capture reported the expected GPIO48 initialization and reached
`Ready at https://nanacoin.local` at 6,705 ms, without the startup panic.
A subsequent HTTPS diagnostic read showed 186 seconds of uptime, confirming
that the board had stayed up beyond the previous repeating startup failure.

Validation passed: `make check` (164 Rust tests plus HTTP/restart smoke),
302 UI tests across 52 files, the ESP release build, and strict board probes
through both `192.168.1.158` and `nanacoin.local`. Both probes verified the CA
and hostname, TLS, API, balanced ledger, exact Angular build, public notebook,
public board health and matching `/ca`. Ledger/config partitions were preserved
by the application-only deployment; without a pre-upgrade count snapshot,
no claim of numerical before/after ledger-count equality is made. Interactive
browser checks and visual confirmation of the complete Morse rotation were
not performed during recovery.

### 2026-09-27: truncated static downloads after successful boot

The owner's browser reported JavaScript syntax errors at the end of downloaded
scripts. A strict HTTPS reproduction of `/main-CI5QVWSH.js` received 116,736 of
273,005 advertised identity bytes before EOF, while a gzip request and both
index representations matched the build in that sample. The previous board
probe compared only the index, so its "exact Angular build" result did not
establish that the JavaScript/CSS downloads completed.

The response loop imposed a five-second deadline from response creation,
even when writes were making progress. Replace that total-duration cutoff with
a 15-second write-stall timeout, refreshed only by accepted bytes. Retain the
five-second incoming-request deadline; a pipelined request's timer starts when
it can be processed after the previous response. Keep bounded connection and
response-memory limits and one nonblocking write per client per turn. Tests
exercise 273,005-byte responses with concurrent slow writers, partial writes,
identical TLS retry slices, stalled readers and disconnections.

The deployment probe now checks every embedded asset byte-for-byte against the
local build, identity and gzip, Content-Length, decompression, ETags and 304s.
It uses three concurrent keep-alive connections and repeats the largest asset
with a small receive buffer and delayed reads. It fails on premature EOF or
mismatched bytes without silently retrying a broken response. The desktop
bundled-site smoke shares these checks (allowing tiny_http's chunked framing).
An initial run against the old firmware caught a second truncated response:
`/chunk-B8XrW2-32.js` ended after 10,240 of 66,920 bytes. That run also recorded
socket errors, so the deadline repair alone requires hardware verification.

Before the upgrade, the board reported sequence 10, five users, four
transactions, journal generation 0 and a balanced ledger. COM9's USB serial
identity matched MAC `ac:a7:04:2c:2c:04`. Hardware results follow after checking
the repaired application. If a browser still holds an old response after
verified repair, force reload with its cache disabled; hashed asset filenames
continue to change when their contents change, and the HTML shell revalidates.

The first deadline-only repair (3,311,664-byte application, verified flash and
ready at 6,735 ms) still failed the concurrent asset probe. Capturing USB serial
while reproducing it revealed `esp-aes: Failed to allocate memory`, followed by
`esp-tls-mbedtls: write error :-0x0001`. This was an internal DMA allocation
failure, not evidence of an invalid JavaScript bundle. The board's short
socket-error code alone had concealed the underlying allocator error.

Additional repair: change the ordinary malloc internal-preference threshold
from 16 KiB to 1 KiB so multi-KiB authentication/request/response allocations
prefer PSRAM. Retain the existing 64 KiB internal-only reserve and TLS PSRAM
allocation policy. Limit every HTTP write, including headers and coalesced
small bodies, to 1 KiB. ESP-IDF 5.5.3's AES external-RAM path can allocate
internal DMA bounce buffers (up to 1,600 bytes per chunk on S3); small records
reduce that peak, and moving ordinary buffers leaves internal memory for DMA
and Wi-Fi. Do not retry a fatal TLS encryption error on the same TLS context.

Final recovery passed on the same identified COM9 board. Dry run and ordinary
application-only deployment wrote 3,311,648 bytes at `0x10000`, with esptool hash
verification; image SHA-256:
`8d90f6cf8eeff6fbf4b13c2b5bc3bd866caa127c9e0e2f323dc7705a50f95094`.
The generated SDK configuration was checked for
`CONFIG_SPIRAM_MALLOC_ALWAYSINTERNAL=1024`. Boot reached ready at 6,725 ms.

`make check` passed formatting, lint, 167 Rust tests and HTTP/restart smoke;
`make web-check` passed the bundled tests, all-asset desktop smoke and deployment
safety tests. The strict live probe passed all 65 assets (1,224,039 identity
bytes and 473,902 gzip bytes), concurrent keep-alive delivery, conditional
responses and the deliberately slow largest-asset transfer. USB serial capture
during that entire probe contained zero error lines. A separate request via
`nanacoin.local` verified the largest JavaScript again in identity and gzip,
including its ETags. The incident history contained no socket, request-timeout,
allocation, storage or TLS-failure events at the final check.

The before/after status matched: sequence 10, five users, four transactions,
journal generation 0, ledger balanced. During asset verification, public health
reported 79,723 internal bytes free with a 36,864-byte largest block; the final
check at 124 seconds uptime reported 85,819 bytes free and a 38,912-byte largest
block. These are observed snapshots, not a guarantee against every future load.
No browser surface was enabled in the computer-use inventory, so browser
rendering/sign-in checks were unavailable; no visual success is claimed.
Evidence is retained in `.local/nanacoin-transfer-*.log`, including the failing
probe/serial capture and the successful final deployment, boot and full probe.

### September 27, 2026: current source deployment

Started from clean revision `2eda80a`; no prior working-tree changes were
discarded. The present USB composite identified native USB serial COM9 and MAC
`ac:a7:04:2c:2c:04`, matching the live server: HTTP diagnostic uptime reset from
134 seconds to 5 seconds after `read_mac`. `nanacoin.local` resolved to
`192.168.1.158`. The pre-deployment ledger snapshot was sequence 10, five users,
four transactions, journal generation 0, balanced.

The dry run passed the Angular production build, 67-asset identity/gzip
packaging, existing certificate and key validation, and ESP32-S3 release build.
The 3,346,256-byte application fits the existing 4 MiB application partition.
`make check` passed formatting, clippy, 167 Rust tests and the HTTP/restart
smoke. `make web-check` passed bundled-web tests, the all-asset desktop HTTP
smoke and three deployment-safety tests. Angular reports the initial bundle at
503.96 kB, 3.96 kB above its configured 500 kB warning budget.

The ordinary `bash scripts/deploy.sh COM9` confirmed the expected `nvs`,
`phy_init`, `factory` and `ledger` partition layout and wrote only the
3,346,256-byte application at `0x10000`; esptool verified its hash. Image SHA-256
is `8154b497e534bffe1103110681e14e1a8c4a81aacb78b1141e596eafb3e1a9fe`.
Serial restart capture reported reset class power-on, a successful PSRAM test,
and `Ready at https://nanacoin.local` at 7,131 ms. The strict probe passed via
the hostname with trusted CA and certificate hostname validation, API and
balanced-ledger checks, all 67 identity/gzip assets with lengths, ETags,
concurrent keep-alive and slow-reader checks, public notebook, Board Health and
matching `/ca`. The post-deployment ledger matched the baseline at sequence 10,
five users, four transactions and journal generation 0; it remained balanced.
At the final diagnostic snapshot the server had 70 seconds uptime and zero
errors. No money-writing tests, resets, provisioning or credential/certificate
changes were performed.

Interactive browser checks were unavailable because no browser surface was
enabled; the strict live probe is the documented automated substitute. Local
logs are `.local/lunacoin-{dry,check,web-check,deploy,boot,probe}-2026-09-27.log`.

### September 30, 2026: Minicloud notification integration

Deployed the current working tree, including the Minicloud notification outbox
and Angular message checkbox, to the user-confirmed S3 bank. Native USB COM9
reported MAC `ac:a7:04:2c:2c:04`; the live diagnostic uptime reset from 130 to
5 seconds after the USB watchdog restart, tying that port to `nanacoin.local`
at `192.168.1.158`. Existing working-tree changes were retained.

The dry run and ordinary `bash scripts/deploy.sh s3 COM9` verified the board
identity and partition layout. Only the application at `0x10000` was written,
3,497,456 bytes within the existing 4 MiB partition; esptool verified the hash.
Image SHA-256:
`a88653b6ebdf2bba61dd19206a9e373663d9f98e089b677b2615f3451d300d72`.
The image includes the notification destination `http://minicloud.local`.
USB boot capture reached `Ready at https://nanacoin.local` at 6,948 ms.

`make check` and `make web-check` passed, including Rust notification tests,
HTTP/restart smoke, bundled-asset smoke and nine deployment safety tests.
The disposable local NanaCoin/Minicloud integration smoke also passed offline
outbox recovery, Lotto event delivery and idempotency, optional message copies,
sender/recipient authorization, anonymous board posting, read dismissal and
the original 24-hour expiration. It used isolated local data, not the live bank.

The strict live S3 probe passed trusted CA and hostname verification, API and
balanced-ledger checks, all 70 bundled assets in identity/gzip forms, ETags,
concurrent keep-alive and slow-reader transfers, public notebook, public Board
Health and matching `/ca`. Every status field matched the pre-flash snapshot:
sequence 18, five users, seven transactions, three active listings, circulation
4,000,000, money epoch 0 and journal generation 0. The ledger remained balanced.
Minicloud's public status endpoint returned HTTP 200 with a synchronized clock.

No financial transaction was created on the live bank to test notifications;
live event delivery and browser interaction are not claimed. No provisioning,
credential/certificate changes, economy resets or storage erases were performed.
Deployment evidence is in `.embuild/minicloud-deploy-*.log` and the before/after
JSON snapshots beside those logs.


### Prepared kitchen update: September 30, 2026 (not deployed)

The owner requested PC review first and deferred both board flashes and the
GitHub Pages push until morning. See README's Kitchen update section for the
loopback preview launcher and review steps. No board was accessed or modified
while preparing this change.

The S3 release image is 3,523,296 / 4,194,304 bytes. SHA-256:
`a1dd46c9d2851aa846dc4d92292b487357575d368f0ea72a3cb607e43b551daa`.
The C6 landscape image is recorded in Minicloud's deployment image manifest;
its runbook remains the authority for that separate board. Rebuild from the
reviewed tree immediately before any flash; these hashes describe preparation
artifacts, not an assertion about firmware currently running on hardware.

Prepared features: landscape word wrapping and paging, coalesced economic
spacers, expanded committed-event notifications, durable borrower applications
and lender proposals, header request/failure feedback, and marketplace cards
showing responses awaiting owner acceptance. Loan proposals do not authorize
funding; the borrower still reviews and accepts. No bank reset, provisioning,
certificate rotation, flash, Git push or Pages publication occurred.

After PC acceptance, use the existing app-only S3 deployment procedure and
strict live probe, then follow Minicloud's app-only update procedure. Observe
orientation and wrapping physically, measure its wider strip's DMA heap impact,
and test a real opt-in message and recipient read. Finally publish the demo
through the existing Pages workflow after its static browser check passes.
Local logs are .embuild/kitchen-*.log. Git Bash's Pages build needs
`MSYS_NO_PATHCONV=1 npx ng build --configuration demo --base-href /nanacoin/`;
otherwise MSYS can rewrite the base href into a Windows filesystem path.
