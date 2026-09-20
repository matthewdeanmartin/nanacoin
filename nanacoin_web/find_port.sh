#!/usr/bin/env bash
# Print the board's serial port, or nothing if it is not attached.
#
#   ./find_port.sh              # e.g. COM4
#   PORT=$(./find_port.sh)
#
# Sourced by the other scripts so none of them has to hard-code a port. The S2
# has no USB-to-serial bridge chip, so the port is presented by whatever
# firmware is running and moves on every reset, every flash and every crash.
# Hard-coding it guarantees a wrong answer sooner or later.
#
# Identification is by USB vendor ID rather than by "whichever port is not
# COM3", which is what the PowerShell version had to do. 303A is Espressif and
# 1A86 is the CH343 bridge on the S3 board.

set -euo pipefail

python - "$@" <<'PY'
import sys

try:
    import serial.tools.list_ports as list_ports
except ImportError:
    sys.stderr.write(
        "pyserial is not installed.\n"
        "  python -m pip install pyserial esptool mpremote\n"
    )
    sys.exit(2)

WANTED = ("303A", "1A86")  # Espressif native USB, CH343 bridge

found = []
for port in list_ports.comports():
    hwid = (port.hwid or "").upper()
    if any(vid in hwid for vid in WANTED):
        found.append(port)

if not found:
    sys.exit(1)

if len(found) > 1:
    sys.stderr.write("several boards attached:\n")
    for p in found:
        sys.stderr.write("  %s  %s\n" % (p.device, p.description))
    sys.stderr.write("using %s; pass -p to choose\n" % found[0].device)

print(found[0].device)
PY
