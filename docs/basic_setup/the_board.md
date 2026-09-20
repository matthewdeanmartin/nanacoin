# The Board

## What it is

The DiGiYes ESP32-S2 Mini V1.0.0 carries an **ESP32-S2FN4R2**:

| Property | Value |
|----------|-------|
| Core | Single-core Xtensa LX7, 240MHz |
| Flash | 4MB (embedded) |
| PSRAM | 2MB (embedded) |
| WiFi | 802.11 b/g/n, **2.4GHz only** |
| Bluetooth | **None** |
| USB | Native USB-OTG |

Two of those matter more than the rest.

**No 5GHz radio.** If your router advertises 2.4GHz and 5GHz under one merged
SSID, this usually works. If it only offers 5GHz where the board is sitting, the
board cannot see the network at all, and reports it the same way it reports a
typo in the SSID.

**No Bluetooth.** The S2 has none. The S3 does. Code and tutorials for the S3
will compile and then fail at runtime.

## Native USB, and why the board looks dead

Most dev boards have a **separate USB-to-serial chip** — a CP2102 or CH340 —
sitting between the USB port and the microcontroller. That chip enumerates the
moment you plug it in, regardless of what the microcontroller is doing. A blank
board still gets you a COM port.

The ESP32-S2 has no such chip. USB goes **straight into the ESP32-S2 itself**.

The consequence: if the flash is empty, or holds firmware that never initialises
USB, the board enumerates as *nothing at all*. No COM port. No unknown device in
Device Manager. Windows shows no evidence it is plugged in.

This is normal for a new S2. It is not a broken board, and not a broken cable.

!!! note "Listings that say 'Fit for MicroPython'"

    These boards frequently ship blank regardless. Do not read the absence of a
    COM port as a defect.

## Bootloader mode

To talk to a board that is not already running USB-aware firmware, put it into
the **ROM bootloader**, which lives in silicon and always works:

1. **Hold** the **BOOT** button (marked `0`)
2. **Tap** the **RESET** button (marked `RST`)
3. **Release** BOOT

A COM port appears within a second or two.

Check from PowerShell:

```powershell
[System.IO.Ports.SerialPort]::GetPortNames()
```

```text
COM3
COM4
```

`COM3` here is a motherboard serial port, not the board — most desktops have one
and it is always present. The port that **appears when you plug the board in** is
the board.

Confirm it is really the ESP32:

```powershell
Get-CimInstance Win32_PnPEntity |
    Where-Object { $_.DeviceID -match 'VID_303A' } |
    Select-Object Name, DeviceID, Status | Format-List
```

```text
Name     : USB Serial Device (COM4)
DeviceID : USB\VID_303A&PID_0002&MI_00\8&1FE9EA56&0&0000
Status   : OK

Name     : ESP32-S2
DeviceID : USB\VID_303A&PID_0002&MI_02\8&1FE9EA56&0&0002
Status   : Error
```

`VID_303A` is Espressif. `PID_0002` is an S2 in ROM bootloader mode.

The `Status : Error` on the third entry is the **JTAG interface** having no
driver installed. It is irrelevant for flashing and can be ignored.

## The COM port moves

This is the single most confusing behaviour of the S2, and it follows directly
from native USB.

Because the USB device *is* the chip, the USB device changes when the chip
changes what it is running:

| Board state | Enumerates as |
|-------------|---------------|
| ROM bootloader (after BOOT/RESET) | One COM port, e.g. `COM4` |
| Running your app | A **different** COM port, e.g. `COM5` |

So the port you flash on is frequently **not** the port you monitor on. A
`flash monitor` command will flash successfully and then fail to monitor:

```text
Leaving...
Hard resetting with a watchdog...
...
could not open port '\\.\COM4': FileNotFoundError(2, ...)
--- Connection to \\.\COM4 failed. Available ports:
--- COM3
```

**The flash succeeded.** `Leaving...` and `Hard resetting` say so. Only the
monitor failed, because COM4 ceased to exist when the chip reset.

Recover by re-checking the port and monitoring on the new one:

```powershell
[System.IO.Ports.SerialPort]::GetPortNames()
idf.py -p COM5 monitor
```

Note `monitor` alone — the firmware is already flashed.

## Verifying the board responds

Before writing any code, confirm the toolchain can talk to the hardware. With
the board in bootloader mode:

```powershell
esptool.py --port COM4 chip_id
```

```text
Detecting chip type... ESP32-S2
Chip is ESP32-S2FNR2 (revision v1.0)
Features: WiFi, Embedded Flash 4MB, Embedded PSRAM 2MB
Crystal is 40MHz
USB mode: USB-OTG
MAC: 80:65:99:f0:1c:9c
```

A MAC address means the board, the cable, and the toolchain are all good. Worth
doing once at the start; it separates hardware problems from software ones
before you have written anything.

!!! warning "This command resets the board"

    `esptool` finishes with `Hard resetting`, which drops the chip **out** of
    bootloader mode. Redo BOOT/RESET before flashing.
