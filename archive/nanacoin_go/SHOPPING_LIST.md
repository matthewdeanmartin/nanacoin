# Shopping list

Written while getting NanaCoin onto the ESP32-S3 you already own.

**Read the first section before ordering anything.** The reason this list
exists no longer applies.

---

## You do not need a new board

The original plan was to buy a RISC-V board because Go could not target the
Xtensa ESP32-S3. That was true for years and is now out of date.

TinyGo 0.41 (April 2026) added Xtensa ESP32-S3 support *with* WiFi, through
the new `espradio` package. TinyGo 0.42 is what this project uses.

NanaCoin now runs on the board on your desk:

| | |
|---|---|
| Board | ESP32-S3-N16R8, the one from `BOARD_SKILL_ESP32_S3_N16R8.md` |
| Firmware | `cmd/nanacoin-esp32`, built with `tinygo` |
| Flash used | 1.1 MB of 16 MB |
| RAM used | 154 KB static |
| Verified | PKCE login, grants, transfers, idempotent retry, atomic purchase, authorization refusals, balanced ledger — all over WiFi |

So the shopping list below is **optional**. It is worth reading only for the
three genuine reasons to buy hardware anyway, which follow.

---

## Reason 1: a second board is useful

Not because the S3 is inadequate, but because flashing the S3 erases whatever
was on it. Getting NanaCoin on there cost you the MicroPython
`hello_wifi_s3_py` dashboard. A second board means the next experiment does
not have to displace a working thing.

### ESP32-C3 SuperMini — about $4–6

The cheapest way to have a spare.

| | |
|---|---|
| Chip | ESP32-C3, single-core RISC-V |
| Flash | 4 MB |
| PSRAM | none |
| TinyGo target | `esp32c3-supermini` (also `xiao-esp32c3`) |
| Why this one | RISC-V, so it rides upstream LLVM rather than Espressif's Xtensa fork. Supported in TinyGo longer than the S3 and correspondingly less bumpy. |

4 MB of flash and no PSRAM is ample: NanaCoin's whole binary is ~1 MB and the
journal projects to about 5 MB over *twenty years* at household volume, which
an external SD card or a larger-flash variant covers if it ever matters.

### Seeed XIAO ESP32-C6 — about $8–10

If you want the better-supported radio and BLE headroom.

| | |
|---|---|
| Chip | ESP32-C6, RISC-V, WiFi 6 + BLE 5.3 + 802.15.4 |
| TinyGo target | `xiao-esp32c6` |
| Why this one | Newest radio, and the 802.15.4 opens Thread/Zigbee for later projects that have nothing to do with NanaCoin. |

---

## Reason 2: the WiFi is marginal where the board lives

This is the real hardware problem you have, and it is not the board's fault.

The access point is four floors down and reads about **-81 dBm** at the S3.
The board associates fine, but DHCP's four-packet exchange loses packets often
enough that espradio's three built-in attempts run out. NanaCoin works around
it by patching the retry count up to 20 (see `patches/README.md`), and
connects after about 15 tries. MicroPython managed the same link without
complaint, so this is a software retry budget rather than an unusable signal.

Patching around it works. Fixing the link would be better.

### An ESP32 board with an external antenna connector — about $8–15

Look for a board with a **U.FL / IPEX connector** rather than a PCB trace
antenna, plus a 2.4 GHz antenna (often bundled). Espressif's own
`ESP32-S3-DevKitC-1-N16R8V` comes in an antenna-connector variant.

Realistically worth **6–10 dB**, which at -81 dBm is the difference between
"DHCP eventually" and "DHCP first try."

### Or: a cheap WiFi extender / mesh node — about $20–30

Fixes the problem for every device on that floor, not just this board, and
needs no code. If anything else up there has trouble, this is the better buy.

### Or: nothing

The patch works. It just takes ~45 seconds to boot.

---

## Reason 3: real persistence needs somewhere to put it

**This is the one genuine functional gap.** The board build currently uses the
in-memory journal, so it forgets every user, balance and transaction on
reboot. It is a working demonstration, not a household ledger.

Closing that gap is mostly software — `internal/storage` exists precisely so
that only one line of `cmd/nanacoin-esp32/main.go` changes — but it needs a
decision about the medium:

| Option | Cost | Notes |
|---|---|---|
| **Onboard flash partition** | $0 | The intended answer. 16 MB is enormous next to a 5 MB / 20 year projection. Needs an ESP32 partition binding written against `storage.Journal`. |
| microSD breakout + card | $3–8 | More parts and a filesystem, but trivially removable and backup-able. TinyGo has SPI SD support in `tinygo.org/x/drivers`. |
| FRAM breakout (e.g. MB85RC256V) | $8–12 | Effectively unlimited write endurance, which is lovely and complete overkill for ten writes a week. |

**Recommendation: none of these.** Write the flash-partition backend against
the existing `storage.Journal` interface. The append-only design in
`internal/storage/flashlog` was written specifically to make that a
same-shaped port, and its crash-safety tests already pass at every byte
offset.

---

## If you only buy one thing

An **ESP32-C3 SuperMini** (~$5), so you have a spare board and can put
MicroPython back somewhere without giving up NanaCoin.

## If you buy nothing

Nothing is blocked. The remaining work is the flash journal backend, which is
code.
