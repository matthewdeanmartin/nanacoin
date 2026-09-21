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
required substitute: it compares the served index to the exact local build and
checks the anonymous notebook and Board Health endpoints. Report the browser
checks as unavailable rather than claiming they were clicked.

## Stop conditions and what they mean

| Symptom | Action |
|---|---|
| No unambiguous serial port | Stop; reconnect/identify the physical board. |
| Port access denied/in use | Close monitors and retry. Do not change flash commands. |
| Partition-layout refusal | Stop. This is not an upgradeable current-layout board. |
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
