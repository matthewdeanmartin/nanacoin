# Deploy Rust NanaCoin and the Angular app to the board

This is the mechanical upgrade procedure for an **existing NanaCoin Rust board
with the current partition layout**. It builds the current Angular client,
embeds it in the Rust firmware, writes only the application partition, and
checks the running board afterward.

It preserves the ledger, household, HTTPS policy, bootloader, and partition
table. It is not an initial installation, TinyGo migration, factory reset, or
recovery procedure.

## Rules for humans and automation

- Work from `nanacoin_rs` in Git Bash on Windows.
- Record `git status --short` before building. A dirty tree is allowed; do not
  discard, reset, stash, or overwrite somebody else's changes.
- Never run `erase_flash`, erase NVS, write the partition table, flash a merged
  image, reset the economy, rotate certificates, or use HTTP recovery as part
  of an ordinary deployment.
- Never print Wi-Fi passwords, private keys, tokens, or credential-file contents.
- Do not run `nanacoin_web/deploy.ps1` or `deploy.sh` afterward. The Rust image
  already contains Angular; deploying the S2 web board would be a separate,
  explicitly requested legacy two-board operation.
- If the serial port or target board is ambiguous, stop and ask. Do not guess.
- If the partition check refuses the board, stop. Do not bypass it or “fix” the
  layout during a routine deployment.
- A successful build or flash is not completion. The strict live-board probe
  must pass.

## Prerequisites

The build computer needs:

- the repository's existing Espressif Rust/ESP-IDF environment;
- Node dependencies in `../nanacoin_ui/node_modules` (run `npm ci` there
  once if they are absent);
- ignored Wi-Fi configuration supported by `build.rs`;
- the existing ignored CA and server certificate under `.local/ca` and `certs`;
- Python `esptool` from the ESP-IDF environment;
- a USB data cable connected to the ESP32-S3 board.

Do not regenerate or rotate working certificates just to deploy. Rotation would
make every household device trust a new CA.

## 1. Identify the serial port

In PowerShell, list present port devices without opening them:

```powershell
Get-PnpDevice -Class Ports -PresentOnly |
  Select-Object FriendlyName, InstanceId |
  Format-Table -AutoSize
```

Use the PnP entity list when the port class omits the USB identity details:

```powershell
Get-CimInstance Win32_PnPEntity |
  Where-Object { $_.Name -match 'COM\d+' -or $_.PNPDeviceID -match 'VID_303A' } |
  Select-Object Name, PNPDeviceID |
  Format-Table -AutoSize
```

`Win32_SerialPort` is an optional additional view, but it does not enumerate
every ESP32 USB Serial/JTAG device reliably on all Windows systems.

Set `PORT` to the unambiguous ESP32-S3 port, such as `COM9`. If several boards
or serial adapters are present, unplug/replug the intended board and list again.
Do not choose based only on an old example in documentation.

Close serial monitors and other programs holding the port before continuing.

### Prove the port is the live NanaCoin board

"A board is plugged in" does not mean "the NanaCoin server is plugged in". On
September 26, 2026 the only USB board present (COM11) was a different
ESP32-S3 with an unrelated partition table (`store`, `media`, `coredump`),
while the real server kept running untouched over Wi-Fi. `deploy.py`'s
partition check caught it, but identify the board before relying on that:

1. Note the live server's uptime:
   `curl -s http://<board-ip>/api/v1/diag` → `uptime_seconds` (`/diag` is
   served over HTTP too; find `<board-ip>` as in step 4).
2. Read the attached chip's MAC. This resets that board:
   `python -m esptool --chip esp32s3 --port "$PORT" read_mac`
3. Read `uptime_seconds` again, retrying for about 30 seconds while it boots.
   If it restarted near zero, the port is the server. If it kept counting, the
   port is **another board**: stop and ask. Do not deploy to it.

A quicker hint that needs no reset: the ESP32-S3's composite USB device ID ends
in its MAC, for example `USB\VID_303A&PID_1001\AC:A7:04:2C:2C:04` in the
`Win32_PnPEntity` listing above. The live server (September 2026) is
`AC:A7:04:2C:2C:04` on 192.168.1.158. Its serial port is the
`USB Serial Device (COMn)` entry with the same instance prefix.

Comparing the esptool MAC with the router or ARP table is only a hint. An ARP
entry in `Probe`/`Stale` state may be old. The uptime test is decisive.
Charge-only USB cables power a board without creating a serial port, so a
powered server can still be absent from the port list.

## 2. Record the tree and run a dry deployment

Open Git Bash:

```bash
cd /c/github/nanacoin/nanacoin_rs
git status --short
PORT=COM9                         # replace with the port found above
bash scripts/deploy.sh "$PORT" --dry-run
```

The dry run does all builds but never opens the port. It must:

1. build the Angular production app;
2. package identity and gzip assets for the same-origin API;
3. validate the existing TLS certificate/key/CA inputs;
4. build the ESP32-S3 release firmware;
5. create a checked application `.bin` no larger than the 4 MiB application
   partition; and
6. print a plan to verify the partition table and write only address `0x10000`.

Warnings are not automatically failures, but any nonzero exit is. Fix the
specific failure; do not skip its check.

## 3. Deploy the same source tree

With the intended board still attached and the port free:

```bash
bash scripts/deploy.sh "$PORT"
```

The deployment script rebuilds, then:

1. reads 4 KiB at `0x8000` from the board;
2. requires exactly the expected `nvs`, `phy_init`, `factory`, and `ledger`
   layout;
3. writes the new application image at `0x10000`; and
4. lets the board restart.

Expected final text includes:

```text
Application updated. Open https://nanacoin.local/ after restart.
```

That sentence proves only that esptool completed. Continue to verification.

## 4. Find the board address

First try its mDNS name:

```powershell
Resolve-DnsName nanacoin.local
```

If mDNS is unavailable on the build computer, use the board's known DHCP address
from the router or its prior deployment record. An IP address is fine for the
probe; the probe still sends `nanacoin.local` for TLS SNI and hostname validation.

Set the result in Git Bash:

```bash
ADDRESS=192.168.1.158             # example only; use the actual address
```

## 5. Prove the live board is correct

Run the strict probe (the direct Python form is equivalent to the Make target):

```bash
python scripts/probe-board.py --address "$ADDRESS"
# equivalent: make probe-board ADDRESS="$ADDRESS"
```

It retries during boot, trusts only `certs/home-ca.crt`, verifies the certificate
for `nanacoin.local`, and checks:

- TLS succeeds without `-k` or a warning bypass;
- `/api/v1/status` returns JSON and says the ledger balances;
- `/` serves the **exact Angular index produced by this build**, configured for
  the same-origin API;
- every bundled asset matches the build byte-for-byte in identity and gzip
  form, with correct lengths, ETags and conditional responses, across concurrent
  keep-alive connections; the largest asset also passes a slow-reader check;
- the anonymous public notebook returns a correctly shaped ledger response;
- anonymous Board Health returns current machine data;
- the served public CA is byte-for-byte the one used for the build.

The command must end with `Board probe passed`. A DNS response, ping, serial boot
line, HTTP 200 alone, or browser certificate bypass is not equivalent.

When a browser surface is available, perform these short product checks in a
browser that already trusts the CA:

1. Open `https://nanacoin.local/?api=` in a private window. The Notebook should
   load without signing in because the ledger is public.
2. Open **Board Health** without signing in. It should show one fresh reading,
   say **Start live updates**, and remain paused until selected.
3. Sign in and confirm the existing household and balances survived.
4. Open Household and My Account to confirm the new section navigation renders.

Do not create a payment, reset the economy, close the journal, or seed demo data
merely to prove deployment.

For non-interactive automation with no browser surface, the strict probe is the
required substitute: it compares every served asset to the exact local build and
checks the anonymous notebook and Board Health endpoints. Report the browser
checks as unavailable rather than claiming they were clicked.

## Stop conditions and what they mean

| Symptom | Action |
|---|---|
| No unambiguous serial port | Stop; reconnect/identify the physical board. |
| Port access denied/in use | Close monitors and retry. Do not change flash commands. |
| Partition-layout refusal | Stop. This is not an upgradeable current-layout board, and often not the NanaCoin server at all (see "Prove the port is the live NanaCoin board"). |
| Live server uptime did not reset after `read_mac` | Stop. The port is a different board. |
| Firmware image too large | Stop and reduce/review the image. Never enlarge or rewrite partitions casually. |
| Certificate validation fails | Stop and inspect the existing certificate inputs. Do not use `-k` and do not rotate automatically. |
| Flash succeeds but strict probe fails | Treat deployment as unverified; check boot, address, Wi-Fi, TLS, and serial evidence. Do not erase the board. |
| Ledger reports unbalanced | Stop all money-writing tests and preserve evidence. |
| Household appears empty | Stop. Do not provision or seed; verify that the intended board/address was used. |

## Equivalent Make target

After the port is known, this is equivalent to the non-dry deployment command:

```bash
make deploy PORT=COM9
```

The explicit `bash scripts/deploy.sh` form is used above because it exposes the
safe `--dry-run` step and makes the accepted arguments obvious.

## Completion report

A deployment report should state:

- source working tree/revision and whether it was dirty;
- selected serial port and how it was identified;
- dry-run result;
- partition verification and application write result;
- board hostname/IP used for verification;
- strict probe result, including balanced-ledger confirmation;
- whether the four browser checks passed;
- any checks not performed and why.

Do not include credentials, tokens, private-key material, or household transaction
contents in the report.

## Optional RGB boot and health diagnostics

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
| Red | Startup/storage failure, or a server worker has stopped progressing for 2 seconds. |

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
