# Build, Flash, Monitor

The working cycle, once [the toolchain](toolchain.md) is set up.

## Activate

Once per terminal window, note the leading dot:

```powershell
. C:\github\microcontroller\idf-env.ps1
cd C:\github\microcontroller\hello_wifi
```

`idf.py` must run from the directory containing `CMakeLists.txt`. From anywhere
else it reports no project found.

## Set the target

Once per project. It tells the build which chip to compile for:

```powershell
idf.py set-target esp32s2
```

!!! warning

    This **wipes `sdkconfig`**, including WiFi credentials. Run it before
    `menuconfig`, not after.

## Configure

```powershell
idf.py menuconfig
```

A blue text UI. Arrow to **Hello WiFi**, `Enter` to open, `Enter` on each field
to edit, `S` to save, `Q` to quit.

Set SSID and password. Two things to get right:

- **2.4GHz network.** The S2 has no 5GHz radio.
- **Exact spelling.** SSIDs are case-sensitive.

Verify what was saved rather than trusting the UI:

```powershell
Select-String -Path sdkconfig -Pattern 'HELLO_WIFI'
```

```text
CONFIG_HELLO_WIFI_SSID="Fios-Martin"
CONFIG_HELLO_WIFI_PASSWORD="..."
```

## Build

```powershell
idf.py build
```

The first build takes several minutes — it compiles the whole SDK. Later builds
are incremental and quick.

Ends with a size report:

```text
hello_wifi.bin binary size 0xb9390 bytes.
Smallest app partition is 0x100000 bytes. 0x46c70 bytes (28%) free.
```

Watch that percentage. Running out of app partition is a normal thing to hit
once WiFi, TLS, and a filesystem are all linked in.

## Flash

Put the board in **bootloader mode** first — hold BOOT, tap RESET, release BOOT
— then find the port:

```powershell
[System.IO.Ports.SerialPort]::GetPortNames()
```

```powershell
idf.py -p COM4 flash monitor
```

## The COM port shuffle

`flash monitor` will often flash successfully and then fail to monitor:

```text
Leaving...
Hard resetting with a watchdog...
...
could not open port '\\.\COM4': FileNotFoundError(2, ...)
--- Connection to \\.\COM4 failed. Available ports:
--- COM3
```

**This is not a failed flash.** `Leaving...` and `Hard resetting` confirm it
worked. The chip reset into your app, re-enumerated as a different USB device,
and COM4 stopped existing. See [The Board](the_board.md#the-com-port-moves).

Recover:

```powershell
# tap RESET on the board
[System.IO.Ports.SerialPort]::GetPortNames()
idf.py -p COM5 monitor
```

`monitor` alone — the firmware is already flashed.

## Reading the monitor

A successful boot, trimmed:

```text
I (29) boot: ESP-IDF GIT-NOTFOUND 2nd stage bootloader
I (31) boot.esp32s2: SPI Flash Size : 4MB
I (37) esp_image: segment 0: paddr=00010020 vaddr=3f000020 size=1adb4h map
I (211) boot: Loaded app from partition at offset 0x10000
I (643) main_task: Calling app_main()
I (673) wifi:wifi firmware version: 4df78f2
I (746) wifi:mode : sta (80:65:99:f0:1c:9c)
I (766) hello_wifi: connecting to SSID "Fios-Martin" ...
I (796) wifi:state: init -> auth (0xb0)
I (826) wifi:state: auth -> assoc (0x0)
I (836) wifi:state: assoc -> run (0x10)
I (896) wifi:connected with Fios-Martin, aid = 24, channel 6, BW20
I (906) wifi:security: WPA2-PSK, phy: bgn, rssi: -60
I (5746) esp_netif_handlers: sta ip: 192.168.1.157, mask: 255.255.255.0
I (5756) hello_wifi: got IP: 192.168.1.157
I (5756) hello_wifi: HTTP server listening on port 80
I (5756) main_task: Returned from app_main()
```

Worth knowing how to read this:

- The number in parentheses is **milliseconds since boot**
- The tag after it says **which component** logged the line — `hello_wifi` is
  your code, `wifi:` and `boot:` are the SDK
- `init -> auth -> assoc -> run` is the association state machine; reaching
  `run` means associated
- `rssi: -60` is signal strength — good
- `Returned from app_main()` is **not** an error; see
  [The Project](the_project.md#entry-point)

Then browse to `192.168.1.157`.

### Monitor keys

| Key | Action |
|-----|--------|
| `Ctrl+]` | Quit |
| `Ctrl+T` `Ctrl+H` | Help |
| `Ctrl+T` `Ctrl+R` | Reset the board |
| `Ctrl+T` `Ctrl+F` | Rebuild and flash |

## Everyday cycle

After the first setup, editing code is:

```powershell
# hold BOOT, tap RESET, release BOOT
idf.py -p COM4 flash monitor
```

`flash` builds first, so a separate `idf.py build` is not needed.
