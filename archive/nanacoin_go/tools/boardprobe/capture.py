#!/usr/bin/env python3
"""Capture the board's serial console across a crash.

# Why this exists

The board prints a heap line per connection. Every earlier investigation read
that console *after* resetting the board, which is precisely too late: the
interesting lines are the last few before it goes quiet, and a reset discards
them. `/diag` has the same problem from the other direction - its history
lives in RAM and is empty again by the time anyone can ask.

So this holds the console open for the whole run, writes every line to a file,
and reports the moment output stops. The last lines before silence are the
board's own account of its final connection.

# What the output distinguishes

The three candidate failure modes look different here, which is the point:

  - `fatal error: out of memory` then silence
        A genuine heap exhaustion. TinyGo's runtimeFatal prints this before
        abort(), so its absence is evidence *against* the memory theory that
        has driven most of this project.

  - falling `free`, then silence with no message
        Ran out without the allocator noticing - or died somewhere that does
        not allocate. Check `blk`: if it collapsed while `free` held steady,
        that is fragmentation rather than exhaustion.

  - healthy numbers, then silence
        Not the heap at all. A stack overflow, a deadlock in the network
        stack, or a radio fault. Different problem, different fix.

  - a boot banner
        It rebooted rather than hanging, which the black box could not tell us
        because nothing survives the reboot to say so.

# Usage

    python capture.py --seconds 600 --out crash.log

Run it in one terminal and the torture suite in another. It prints a line
whenever the board has been quiet for a while, so a hang is visible as it
happens rather than at the end.
"""

import argparse
import sys
import time
from collections import deque

try:
    import serial
    from serial.tools import list_ports
except ImportError:
    print("needs pyserial:  pip install pyserial")
    sys.exit(2)

# The native USB CDC port on the S3. The CH343 bridge is a different VID and
# is used for flashing, not for reading the console.
NATIVE_VID = 0x303A


def find_native():
    for p in list_ports.comports():
        if p.vid == NATIVE_VID:
            return p.device
    return None


# Lines that mean the board just started, so the run can tell a reboot from a
# hang. A hang produces silence; a reboot produces these.
BOOT_MARKERS = (
    "NanaCoin starting",
    "ESP-ROM:",
    "rst:0x",
)

# Lines that mean the runtime gave up. TinyGo's runtimeFatal prints the first
# of these immediately before abort().
FATAL_MARKERS = (
    "fatal error:",
    "panic:",
    "out of memory",
    "FATAL:",
)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--seconds", type=float, default=600.0,
                    help="how long to watch")
    ap.add_argument("--out", default="crash.log",
                    help="write every line here")
    ap.add_argument("--quiet-after", type=float, default=15.0,
                    help="seconds of silence before reporting a stall")
    ap.add_argument("--tail", type=int, default=25,
                    help="lines of context to show around an event")
    args = ap.parse_args()

    port = find_native()
    if not port:
        print("no native USB port - board is not enumerating")
        return 1

    print(f"capturing {port} for {args.seconds:.0f}s -> {args.out}")
    print("(run the torture suite in another terminal now)\n", flush=True)

    try:
        sp = serial.Serial(port, 115200, timeout=0.2, dsrdtr=True)
    except Exception as e:
        print(f"cannot open {port}: {e}")
        return 1

    recent = deque(maxlen=args.tail)
    deadline = time.monotonic() + args.seconds
    last_line_at = time.monotonic()
    started = time.monotonic()

    stalls = []
    reboots = []
    fatals = []
    lines = 0
    reported_quiet = False
    buf = b""

    with open(args.out, "w", encoding="utf-8") as log:
        try:
            while time.monotonic() < deadline:
                try:
                    chunk = sp.read(4096)
                except Exception as e:
                    print(f"\n[serial read error: {e}]")
                    break

                now = time.monotonic()

                if chunk:
                    buf += chunk
                    while b"\n" in buf:
                        raw, buf = buf.split(b"\n", 1)
                        line = raw.decode("utf-8", "replace").rstrip("\r")
                        if not line:
                            continue

                        lines += 1
                        t = now - started
                        stamped = f"[{t:7.1f}s] {line}"
                        log.write(stamped + "\n")
                        log.flush()
                        recent.append(stamped)

                        if any(m in line for m in FATAL_MARKERS):
                            fatals.append((t, line))
                            print(f"\n*** FATAL at {t:.1f}s ***")
                            for r in recent:
                                print("  " + r)
                            print(flush=True)
                        elif any(m in line for m in BOOT_MARKERS):
                            # Only count a reboot after the first boot.
                            if t > 5.0:
                                reboots.append(t)
                                print(f"\n*** REBOOTED at {t:.1f}s "
                                      f"(it reset rather than hanging) ***")
                                for r in recent:
                                    print("  " + r)
                                print(flush=True)

                    last_line_at = now
                    reported_quiet = False
                    continue

                # No data this pass.
                quiet = now - last_line_at
                if quiet >= args.quiet_after and not reported_quiet:
                    reported_quiet = True
                    t = now - started
                    stalls.append(t)
                    print(f"\n*** SILENT for {quiet:.0f}s at {t:.1f}s ***")
                    print("    last lines before silence:")
                    for r in recent:
                        print("  " + r)
                    print(flush=True)
        finally:
            sp.close()

    print("\n" + "=" * 62)
    print(f"captured {lines} lines over {time.monotonic()-started:.0f}s "
          f"-> {args.out}")
    print(f"fatal messages: {len(fatals)}")
    print(f"reboots:        {len(reboots)}")
    print(f"stalls:         {len(stalls)}")

    if fatals:
        print("\nThe runtime reported a fatal error, so this was a real "
              "abort:")
        for t, line in fatals:
            print(f"  {t:7.1f}s  {line}")
    elif reboots:
        print("\nThe board reset without a fatal message. That is a watchdog "
              "or a hardware fault, not a Go-level panic.")
    elif stalls:
        print("\nThe board went quiet with no fatal message and no reboot, "
              "so it hung rather than crashed.")
        print("Check the last heap line above: 'free' collapsing means "
              "exhaustion, 'blk' collapsing while free holds means "
              "fragmentation, and healthy numbers mean the failure is not "
              "the heap at all.")
    else:
        print("\nNo stall, no reboot, no fatal error - the board stayed up "
              "for the whole capture.")

    return 0


if __name__ == "__main__":
    sys.exit(main())
