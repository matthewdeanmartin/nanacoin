"""Update only the application on an already-provisioned NanaCoin bank.

Every write is preceded by four independent identity checks, any of which
stops the deployment before the flash is touched:

1. the image header's chip ID and the embedded board marker name this board;
2. esptool connects with this board's --chip (an S2 image never reaches an S3);
3. the connected chip's MAC is the one recorded for this board in boards.py;
4. the partition table on the chip is exactly this board's layout.

No full-chip erase, partition rewrite, bootloader replacement, recovery flash,
or serial monitor. First installation is scripts/provision.py.
"""
import argparse
import pathlib
import re
import struct
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from boards import BOARDS, board  # noqa: E402


def check_image(b, image):
    # firmware-image.py has a hyphenated name; load its checker by path.
    import importlib.util
    spec = importlib.util.spec_from_file_location('firmware_image', pathlib.Path(__file__).with_name('firmware-image.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.check_image(b, image)


def parse_partition_table(data):
    found = {}
    for at in range(0, min(len(data), 0xC00), 32):
        row = data[at:at + 32]
        if len(row) != 32:
            raise ValueError('Short partition table')
        magic, = struct.unpack_from('<H', row)
        if magic in (0xFFFF, 0xEBEB):  # end / IDF MD5 trailer
            break
        if magic != 0x50AA:
            raise ValueError('Invalid partition table')
        _, kind, subtype, offset, size, label, flags = struct.unpack('<HBBII16sI', row)
        name = label.rstrip(b'\0').decode('ascii')
        if flags or name in found:
            raise ValueError('Encrypted/duplicate partition requires separate deployment review')
        found[name] = (kind, subtype, offset, size)
    return found


def verify_partition_table(b, data):
    found = parse_partition_table(data)
    if found == b.layout():
        return
    for other in BOARDS.values():
        if other.id != b.id and found == other.layout():
            raise ValueError(f'This chip has the {other.id} bank layout ({other.hostname}), not {b.id}; refusing to write.')
    raise ValueError(f'Board is not using the {b.id} NanaCoin partition layout; refusing to write. No automatic migration.')


def chip_mac(output):
    match = re.search(r'MAC:\s*([0-9a-fA-F]{2}(?::[0-9a-fA-F]{2}){5})', output)
    if not match:
        raise ValueError('esptool did not report the chip MAC; refusing to write.')
    return match.group(1).lower()


def verify_mac(b, output):
    mac = chip_mac(output)
    if mac == b.mac:
        return mac
    for other in BOARDS.values():
        if other.mac == mac:
            raise ValueError(f'Port is the {other.id} bank ({other.hostname}, MAC {mac}), not {b.id}; refusing to write.')
    raise ValueError(f'Chip MAC {mac} is not the recorded {b.id} board ({b.mac}); refusing to write. '
                     'If the board was deliberately replaced, update boards.py first.')


def wait_for_port(port, seconds=20):
    """The S2's native USB re-enumerates when esptool closes it, even while the
    flasher stays in download mode. Wait for the same port to return."""
    from serial.tools import list_ports
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if any(p.device.upper() == port.upper() for p in list_ports.comports()):
            time.sleep(0.5)  # let Windows finish attaching the driver
            return
        time.sleep(0.25)
    raise SystemExit(f'{port} did not return within {seconds} s; nothing further was written. '
                     'Put the board back in download mode and check the port.')


def esptool(b, port, *args, capture=False):
    if b.manual_download:
        wait_for_port(port)
    command = [sys.executable, '-m', 'esptool', '--chip', b.chip, '--port', port, *args]
    if not capture:
        subprocess.run(command, check=True)
        return ''
    result = subprocess.run(command, check=False, capture_output=True, text=True)
    sys.stdout.write(result.stdout)
    sys.stderr.write(result.stderr)
    if result.returncode:
        raise SystemExit(f'esptool failed (exit {result.returncode}); nothing was written.')
    return result.stdout


def read_identity(b, port, temp, after):
    """One esptool session: verify chip type, MAC and partition table."""
    table = pathlib.Path(temp) / 'partitions.bin'
    output = esptool(b, port, '--before', b.before, '--after', after,
                     'read_flash', '0x8000', '0x1000', str(table), capture=True)
    return verify_mac(b, output), table.read_bytes()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--board', required=True, choices=sorted(BOARDS))
    parser.add_argument('--port', required=True)
    parser.add_argument('--image', type=pathlib.Path, required=True)
    parser.add_argument('--dry-run', action='store_true', help='Print plan without opening a serial port')
    args = parser.parse_args()
    b = board(args.board)
    image = args.image.resolve()
    if not image.is_file():
        parser.error('Missing application image; build firmware first')
    try:
        size = check_image(b, image)
    except ValueError as error:
        parser.error(str(error))
    print(f'Board: {b.id} = {b.name}, {b.hostname}, chip {b.chip}, MAC {b.mac}')
    print(f'Plan: verify chip, MAC and {b.id} partition table on {args.port}, then write {image} ({size} bytes) at 0x10000 only.')
    print('Ledger, configuration, bootloader and partition table will not be written.')
    if b.manual_download:
        print('The board must already be in download mode: hold BOOT, tap RST, release BOOT.')
    if args.dry_run:
        return
    with tempfile.TemporaryDirectory(prefix='nanacoin-partitions-') as temp:
        try:
            mac, table = read_identity(b, args.port, temp, after='no_reset')
            verify_partition_table(b, table)
        except ValueError as error:
            raise SystemExit(str(error)) from None
    print(f'Verified {b.id} bank: MAC {mac}, {b.id} partition layout.')
    esptool(b, args.port, '--before', 'no_reset', '--after', 'hard_reset', 'write_flash', '0x10000', str(image))
    print(f'Application updated on the {b.id} bank. Open https://{b.hostname}/ after restart.')


if __name__ == '__main__':
    main()
