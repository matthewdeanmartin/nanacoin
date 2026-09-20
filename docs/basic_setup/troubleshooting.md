# Troubleshooting

Every error hit while getting this board working, with its actual cause.

## Toolchain

### `idf.py` is not recognized

```text
idf.py : The term 'idf.py' is not recognized as the name of a cmdlet...
```

The shell is not activated. `idf.py` is not on PATH until it is:

```powershell
. C:\github\microcontroller\idf-env.ps1
```

Activation is **per terminal window**. A new window needs it again.

If you ran it and still get this, the leading `.` was probably dropped — see
below.

### The script ran but nothing changed

`export.ps1` and `idf-env.ps1` must be **dot-sourced**. Without the leading dot
they run in a subprocess, print `Done! You can now compile ESP-IDF projects.`,
and leave your shell untouched.

```powershell
. C:\github\microcontroller\idf-env.ps1
```

The dot and the space are part of the command. `idf-env.ps1` detects this
mistake and prints the correct form.

### `No module named 'click'`

```text
No module named 'click'
This usually means that "idf.py" was not spawned within an ESP-IDF shell
environment or the python virtual environment used by "idf.py" is corrupted.
```

The virtualenv is almost never corrupt. `idf.py` starts with
`#!/usr/bin/env python`, so it runs under whatever `python` is first on PATH.

Diagnose:

```powershell
(Get-Command python).Source
```

| Result | Fix |
|--------|-----|
| `...\idf5.5_py3.11_env\Scripts\python.exe` | Correct; look elsewhere |
| `...\tools\idf-python\3.11.2\python.exe` | Bare interpreter is shadowing the venv |
| `...\Python312\python.exe` | Never activated |

The middle case is subtle: the bundled interpreter has **no packages**. It needs
to be on PATH during activation so the right Python version is detected, then
removed afterwards so the venv wins. `idf-env.ps1` does this.

### `MSys/Mingw is not supported`

```text
ERROR: MSys/Mingw is not supported. Please follow the getting started guide...
```

`idf_tools.py` aborts if the `MSYSTEM` environment variable is set. Git Bash
always sets it, and it is **inherited** — a PowerShell window launched from Git
Bash carries it while looking clean.

```powershell
Remove-Item Env:MSYSTEM -ErrorAction SilentlyContinue
```

`idf-env.ps1` does this automatically. Use PowerShell for ESP-IDF work; Git Bash
cannot build these projects at all.

### Virtual environment "not found"

```text
ERROR: ESP-IDF Python virtual environment
"C:\Espressif\python_env\idf5.5_py3.12_env\Scripts\python.exe" not found.
Please run the install script to set it up before proceeding.
```

**Do not reinstall.** The venv exists, as `idf5.5_py3.11_env`. A system-wide
Python 3.12 was detected, so it looked for a 3.12 venv that was never created.

Confirm before believing the error:

```powershell
Get-ChildItem C:\Espressif\python_env\
```

Fix by putting the bundled Python first during activation, as `idf-env.ps1`
does.

### `Insufficient hexadecimal digits`

```text
parsing "Git\usr\bin|Git\mingw|msys" - Insufficient hexadecimal digits.
```

A PowerShell regex containing `\u`, which is read as a Unicode escape. Use
`-notlike` with wildcards instead of `-notmatch` with regex, so backslashes stay
literal.

## Hardware

### No COM port appears

Expected for a new ESP32-S2. It has **native USB** and no separate serial chip,
so a blank board enumerates as nothing at all. See
[The Board](the_board.md#native-usb-and-why-the-board-looks-dead).

1. Put it in bootloader mode: hold **BOOT**, tap **RESET**, release **BOOT**
2. Re-check: `[System.IO.Ports.SerialPort]::GetPortNames()`
3. Still nothing — try a **different USB-C cable**. Charge-only cables have no
   data lines, look identical, and fail silently.

### `could not open port '\\.\COM4'`

```text
could not open port '\\.\COM4': FileNotFoundError(2, ...)
--- Connection to \\.\COM4 failed. Available ports:
--- COM3
```

The port moved. The chip re-enumerates when switching between bootloader and
app, so the flash port is often not the monitor port.

```powershell
[System.IO.Ports.SerialPort]::GetPortNames()
idf.py -p COM5 monitor
```

If this appeared right after `Leaving...` / `Hard resetting`, **the flash
succeeded** — only the monitor failed.

### `Status : Error` on the ESP32-S2 device

The JTAG interface has no driver. Irrelevant for flashing; ignore it. What
matters is the `USB Serial Device (COMn)` entry showing `Status : OK`.

### Monitor shows nothing

The console is not routed to USB. This board has no UART bridge, so it needs:

```ini
CONFIG_ESP_CONSOLE_USB_CDC=y
```

In `sdkconfig.defaults`. Note that `idf.py set-target` wipes `sdkconfig` and
regenerates it from the defaults file, so check the defaults rather than
`sdkconfig`.

## Crashes

### `A stack overflow in task main has been detected`

```text
***ERROR*** A stack overflow in task main has been detected.

Backtrace: 0x4002bcf9:0x3ffd4480 ... |<-CORRUPTED
--- 0x4002cdf6: vApplicationStackOverflowHook at ...port.c:563
Rebooting...
```

Then it reboots and does it again — a boot loop.

The usual cause is a **large local variable**. Stacks on an MCU are tiny:

| Task | Default stack |
|------|---------------|
| `main` | 3584 bytes (`CONFIG_ESP_MAIN_TASK_STACK_SIZE`) |
| `httpd` | 4096 bytes (`HTTPD_DEFAULT_CONFIG`) |

A single innocuous-looking array can exceed that:

```c
wifi_ap_record_t aps[20];   /* ~700 bytes each = ~14000 bytes. Boom. */
```

Put anything of that size on the **heap**:

```c
wifi_ap_record_t *aps = malloc(SCAN_MAX_APS * sizeof(wifi_ap_record_t));
if (aps == NULL) { /* handle it */ }
...
free(aps);
```

Note that the backtrace points at the FreeRTOS scheduler, not at the offending
function — the overflow is detected at a context switch, long after the damage.
Look for big locals in whatever ran just before the last log line, not at the
addresses in the trace.

Check a task's stack headroom at runtime with:

```c
ESP_LOGI(TAG, "stack free: %u", uxTaskGetStackHighWaterMark(NULL));
```

Or raise the main task's stack in `idf.py menuconfig` under
**Component config → ESP System Settings → Main task stack size**.

## WiFi

### Repeated `disconnected`

```text
W (3153) hello_wifi: disconnected; retry 1/10
...
E (27393) hello_wifi: failed to connect after 10 tries
```

Check the SSID in the log line above the retries first:

```text
I (733) hello_wifi: connecting to SSID "hello" ...
```

`"hello"` is the leftover default — the credentials were never set. This is the
most common cause and looks identical to a signal problem.

```powershell
idf.py menuconfig          # Hello WiFi -> SSID + password
Select-String -Path sdkconfig -Pattern 'HELLO_WIFI'
```

Otherwise use the reason code:

| Code | Meaning | Fix |
|------|---------|-----|
| 201 | `NO_AP_FOUND` | Wrong SSID, 5GHz-only, out of range, or [the AP's 2.4GHz radio is down](#the-24ghz-radio-disappeared) |
| 202 | `AUTH_FAIL` | Wrong password |
| 15 | `4WAY_HANDSHAKE_TIMEOUT` | Usually wrong password |
| 200 / 205 | Beacon timeout / connection fail | Weak signal |

And the scan output, which says whether the AP is audible at all:

```text
I (1234) hello_wifi: found 6 network(s):
I (1234) hello_wifi:   Fios-Martin   ch6   -58 dBm   <== target
```

- **Target listed** — in range; signal is not the problem
- **Target missing, others listed** — the 2.4GHz radio works, but that SSID is
  not on 2.4GHz here. Likely a dual-band router; split the bands in the router
  admin so 2.4GHz has its own SSID.
- **Nothing listed** — genuinely out of range

### Worked for a while, then stopped

```text
I (874806) wifi:state: run -> init (0x4c0)
W (874836) hello_wifi: disconnected (reason 4); retry 1/10
...
E (893816) hello_wifi: failed to connect after 10 tries
```

The board is not crashed — it is still running, having given up on WiFi.

**Reason 4 is `ASSOC_EXPIRE`**: the *access point* dropped the association.
Routine AP housekeeping, and not a fault on the board's side. The reason codes
that follow during reconnection (2 `AUTH_EXPIRE`, 201, 205) are transient
symptoms of retrying while the AP is still unavailable, not independent
problems.

The real bug is retry policy. Bounded retries are right at **startup**, where a
typo'd SSID should report itself rather than retry silently forever. They are
wrong **after** a working connection, where the network is presumably coming
back.

This project distinguishes the two with a `s_ever_connected` flag:

| Situation | Policy |
|-----------|--------|
| Never connected | 10 fast retries, then report and reboot after 30s |
| Connected before | Retry forever, 1s doubling to a 30s cap |

!!! note "Power save can contribute"

    ```text
    I (874816) wifi:pm stop, total sleep time: 600958888 us / 873908291 us
    ```

    The radio was asleep 69% of the time under the default
    `WIFI_PS_MIN_MODEM`. That is right for battery use, but it adds latency to
    incoming requests and makes a dropped link slower to notice. For a
    USB-powered server, `esp_wifi_set_ps(WIFI_PS_NONE)` keeps the radio awake.

### The 2.4GHz radio disappeared

Worth checking early, because nothing on the board can fix it and it looks like
a board problem.

Symptom — mostly reason 201, occasionally reason 2, never reaching `assoc`:

```text
W (3164) hello_wifi: disconnected (reason 201); retry 1/10 in 1000 ms
I (4274) wifi:state: init -> auth (0xb0)
I (5284) wifi:state: auth -> init (0x200)
W (5304) hello_wifi: disconnected (reason 2); retry 2/10 in 2000 ms
W (9734) hello_wifi: disconnected (reason 201); retry 3/10 in 4000 ms
```

**Reason 201 is `NO_AP_FOUND`, and it can be literally true.** A dual-band
router runs two radios under one SSID, and the 2.4GHz one can stop broadcasting
on its own — a firmware update, a radio restart, or a config change applied
overnight. Every 2.4GHz-only device then drops off, while everything on 5GHz
carries on and nothing appears wrong.

Confirm from the PC rather than guessing, since it uses a different radio
entirely:

```powershell
netsh wlan show networks mode=bssid |
    Select-String -Pattern 'SSID|BSSID|Band|Channel '
```

A healthy dual-band router shows **two BSSIDs** for the SSID, usually differing
in the last hex digit:

```text
BSSID 1 : 3c:bd:c5:1a:7e:72    Band : 2.4 GHz   Channel : 6
BSSID 2 : 3c:bd:c5:1a:7e:73    Band : 5 GHz     Channel : 132
```

Only the 5GHz one listed means the 2.4GHz radio is down. The ESP32-S2 has no
5GHz radio, so it has nothing to join and reports 201 correctly.

!!! note "The PC may under-report"

    An adapter associated to 5GHz sometimes omits other bands. Confirm on a
    phone, or check whether any 2.4GHz-only device in the house (smart plug,
    printer, thermostat) has also dropped offline.

What to do:

1. Check the router admin — is the 2.4GHz radio **enabled** and **broadcasting**
2. Give 2.4GHz **its own SSID** (e.g. `Fios-Martin-24`). This removes band
   steering permanently and does not disturb clients on 5GHz.
3. Wait. In the case that produced this section, the radio returned by itself
   some minutes later with no intervention.

!!! warning "Do not debug this on the board"

    Reverting firmware, re-flashing, and changing WiFi settings all achieve
    nothing here, and each one invites a new bug. The disconnect reason code is
    already telling the truth; verify the AP from another device before
    touching the code.

### Only 5GHz available

The ESP32-S2 has **no 5GHz radio**. A 5GHz-only network can never work. Merged
dual-band SSIDs usually do.

### `Password length matches WPA2 standards`

```text
W (686) wifi:Password length matches WPA2 standards,
        authmode threshold changes from OPEN to WPA2
```

Informational, not a problem. The driver inferred WPA2 from the password length
and raised its minimum security accordingly.

## HTTP

### 404 on `/favicon.ico`

```text
W (36946) httpd_uri: httpd_uri: URI '/favicon.ico' not found
```

Browsers request it automatically. Harmless, but noisy. This project registers a
handler returning `204 No Content` to suppress it.

### Page does not load despite an IP

1. Confirm the server started:
   `I (5756) hello_wifi: HTTP server listening on port 80`
2. Check the client is on the **same network** — not guest WiFi, not a VPN
3. Try `curl http://<ip>/health`, which rules out browser caching
4. Check the router for **AP isolation** / **client isolation**, which blocks
   device-to-device traffic
