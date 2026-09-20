# Setup

## Hardware and software

This guide targets the ESP32-S3-N16R8 devkit. The S2 projects elsewhere in the
repository are different firmware targets; their flashing offsets and runtime
instructions are not interchangeable.

The current development combination is TinyGo 0.42.0 and Go 1.26.5. The latter
is declared in `nanacoin_go/go.mod`. Treat these as the tested combination, not a
promise that arbitrary newer or older versions work together. Start with the
[TinyGo installation instructions](https://tinygo.org/getting-started/install/)
when installing the compiler.

You also need Git, Node/npm for the Angular application, and Python with
`esptool` and `pyserial` for flashing and serial tools. Locust uses its own
Python 3.14 environment through `uv` in the sibling load-test project.

```powershell
go version
tinygo version
node --version
python -m pip install esptool pyserial
python -m serial.tools.list_ports -v
```

## Local networking dependencies

From `nanacoin_go/`, bootstrap the patched dependencies:

```powershell
.\patches\apply.ps1
```

The Go module uses local replacements under `third_party`. The patches cover
radio/network behavior needed by this application; downloading an unmodified
upstream package is not an equivalent setup. Read the
[patch notes](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_go/patches/README.md)
when updating dependencies.

The bootstrap script skips existing dependency directories. Running it again
does not prove that an old checkout has acquired every new change. Inspect
the checkout and patch state rather than deleting local work to force a reset.
Record dependency and toolchain versions with hardware measurements.

## Which USB port?

The devkit exposes two different connections:

| Connection | Identification | Use with this firmware |
|---|---|---|
| CH343 USB/UART bridge | USB vendor `1A86`; often COM8 on the development machine | Stable programming port |
| Native ESP USB | USB vendor `303A`; COM number may change after reset | TinyGo console output |

COM numbers are examples, not board properties. Use the port-list command
above each time the device has moved or re-enumerated. The MicroPython guide's
advice to use the bridge for its REPL does not describe TinyGo's console.

Only one program can normally own a serial port at a time. Close an existing
monitor before flashing or starting a recorder on that port. A charge-only
USB cable supplies power but cannot carry either programming traffic or logs.

## WiFi and addresses

The board needs a reachable 2.4 GHz network. The deploy script accepts the SSID
and password and embeds them in the firmware:

```powershell
.\deploy.ps1 -Port COM8 -Ssid "YourWiFi" -Password "YourPassword"
```

`-SaveCredentials` saves local credentials for later builds; the script also
reads `wifi.local.json`. Keep that file and credential-bearing firmware out of
published artifacts. Command-line passwords can also remain in shell history.

Use `http://nanacoin-api.local` on the same LAN, or the DHCP address printed by
the board if your client/network does not support mDNS. The separate MicroPython
web board keeps `nanacoin.local`. Do not assume the example address
`192.168.1.158` is yours.

For the first bring-up, place the board near the router. Once it works there,
move it to a difficult location deliberately and measure network resilience
as a separate experiment.
