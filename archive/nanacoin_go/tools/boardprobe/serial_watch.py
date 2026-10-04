#!/usr/bin/env python3
"""Watch the board's serial console with a deadline.

The native USB port re-enumerates on every reboot and disappears while the
board is hung, so a watcher has to find the port each time and give up rather
than block. PowerShell's SerialPort kept leaving handles open, which made the
next read return nothing and look like a silent board - a misleading answer to
the most important question here.

    python serial_watch.py [seconds] [--reset]

--reset pulses RTS via esptool first, so the boot banner is captured from the
start rather than joined mid-stream.
"""

import subprocess
import sys
import time

try:
    import serial
    from serial.tools import list_ports
except ImportError:
    print("needs pyserial:  pip install pyserial")
    sys.exit(2)

# The native USB CDC port on the S3 (Espressif VID). The CH343 bridge is a
# different VID and is used for flashing, not for reading the console.
NATIVE_VID = 0x303A


def find_native():
    for p in list_ports.comports():
        if p.vid == NATIVE_VID:
            return p.device
    return None


def main():
    seconds = 30.0
    do_reset = "--reset" in sys.argv
    for a in sys.argv[1:]:
        if not a.startswith("-"):
            seconds = float(a)

    if do_reset:
        print("resetting via RTS...", flush=True)
        subprocess.run(
            ["python", "-m", "esptool", "--chip", "esp32s3", "--port", "COM8",
             "--after", "hard-reset", "chip-id"],
            capture_output=True,
        )
        time.sleep(2)

    port = find_native()
    if not port:
        print("no native USB port - board is not enumerating "
              "(unpowered, or hung hard enough to drop USB)")
        return 1

    print(f"reading {port} for {seconds}s", flush=True)
    try:
        sp = serial.Serial(port, 115200, timeout=0.2, dsrdtr=True)
    except Exception as e:
        print(f"cannot open {port}: {e}")
        return 1

    got_any = False
    deadline = time.monotonic() + seconds
    try:
        while time.monotonic() < deadline:
            try:
                data = sp.read(4096)
            except Exception as e:
                print(f"\n[read error: {e}]")
                break
            if data:
                got_any = True
                sys.stdout.write(data.decode("utf-8", "replace"))
                sys.stdout.flush()
    finally:
        sp.close()

    if not got_any:
        print("\n(silence - port enumerates but the board is printing nothing: "
              "hung, not reset-looping, since a reboot would print a banner)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
