"""Capture the board's boot log from the USB serial port.

By default the board is restarted first with esptool's RTC watchdog reset
(esptool's plain USB reset leaves this board stuck in download mode). The
restart does not write flash. Pass --no-reset to only listen.

Prints the log, then a SUMMARY line with the board's address. Exit 0 only when
the firmware reached "Ready at". Run with the ESP-IDF Python (esptool, pyserial):
    /c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe scripts/boot-log.py --port COM9
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
import time

import serial


def restart(port: str) -> None:
    subprocess.run([sys.executable, '-m', 'esptool', '--chip', 'esp32s3', '--port', port,
                    '--after', 'watchdog_reset', 'read_mac'],
                   check=True, stdout=subprocess.DEVNULL)


def open_port(name: str) -> serial.Serial:
    port = serial.Serial()
    port.port = name
    port.baudrate = 115200
    port.timeout = 0.2
    # Both lines low: opening must not reset the chip or select download mode.
    port.dtr = False
    port.rts = False
    for _ in range(50):  # the port reappears a moment after the reset
        try:
            port.open()
            return port
        except serial.SerialException:
            time.sleep(0.1)
    raise SystemExit(f'Could not open {name}; close any serial monitor using it')


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--port', required=True)
    parser.add_argument('--seconds', type=float, default=60)
    parser.add_argument('--no-reset', action='store_true')
    args = parser.parse_args()

    if not args.no_reset:
        restart(args.port)
    port = open_port(args.port)
    deadline = time.monotonic() + args.seconds
    text = ''
    while time.monotonic() < deadline:
        chunk = port.read(4096).decode('utf-8', 'replace')
        if chunk:
            sys.stdout.write(chunk)
            sys.stdout.flush()
            text += chunk
            if 'Ready at' in text or 'Setup network' in text:
                deadline = min(deadline, time.monotonic() + 2)
    port.close()

    if 'Ready at https://nanacoin.local' in text:
        print('SUMMARY: NanaCoin ready')
        return 0
    if 'waiting for download' in text:
        print('\nSUMMARY: the chip is in download mode (firmware not running); see DEPLOY.md')
        return 1
    crashed = re.search(r'panic|abort\(\) was called|Guru Meditation|Error: ', text)
    reason = 'the firmware reported an error' if crashed else 'nothing more within the time limit'
    print(f'\nSUMMARY: boot did not reach "Ready at" ({reason}); see the log above')
    return 1


if __name__ == '__main__':
    sys.exit(main())
