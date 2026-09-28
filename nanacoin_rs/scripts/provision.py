"""First installation of a NanaCoin bank: erase the chip, then write the
bootloader, this board's partition table and the application.

This destroys everything on the chip. It is only for a board that is not yet a
NanaCoin bank (for example the S2 Mini arriving with MicroPython). It refuses:

- an image, bootloader or partition table that is not this board's build;
- a chip whose type or MAC is not the recorded board in boards.py;
- a chip that already carries ANY NanaCoin bank layout, because that chip has
  a ledger. Replacing a bank's ledger needs --replace-existing-bank as well
  as the owner's explicit decision; ordinary upgrades use scripts/deploy.py.
"""
import argparse
import pathlib
import subprocess
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from boards import BOARDS, board  # noqa: E402
from deploy import check_image, esptool, parse_partition_table, verify_mac  # noqa: E402


def check_bootloader(b, path):
    data = path.read_bytes()
    if not data or data[0] != 0xE9 or int.from_bytes(data[12:14], 'little') != b.image_chip_id:
        raise ValueError(f'{path} is not a {b.chip} bootloader image')


def check_table(b, path):
    found = parse_partition_table(path.read_bytes())
    if found != b.layout():
        raise ValueError(f'{path} is not the {b.id} partition table from {b.partitions}')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--board', required=True, choices=sorted(BOARDS))
    parser.add_argument('--port', required=True)
    parser.add_argument('--image', type=pathlib.Path, required=True)
    parser.add_argument('--bootloader', type=pathlib.Path, required=True)
    parser.add_argument('--partition-table', type=pathlib.Path, required=True)
    parser.add_argument('--replace-existing-bank', action='store_true',
                        help='Allow erasing a chip that already has a NanaCoin ledger (owner decision only)')
    parser.add_argument('--dry-run', action='store_true', help='Check inputs and print the plan; no serial port')
    args = parser.parse_args()
    b = board(args.board)
    try:
        size = check_image(b, args.image)
        check_bootloader(b, args.bootloader)
        check_table(b, args.partition_table)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(f'Board: {b.id} = {b.name}, {b.hostname}, chip {b.chip}, MAC {b.mac}')
    print(f'Plan: verify chip and MAC on {args.port}, refuse any existing NanaCoin ledger, ERASE THE WHOLE CHIP, then write')
    print(f'  bootloader {hex(b.bootloader_offset)}, partition table 0x8000, application 0x10000 ({size} bytes).')
    if b.manual_download:
        print('The board must already be in download mode: hold BOOT, tap RST, release BOOT.')
    if args.dry_run:
        return
    with tempfile.TemporaryDirectory(prefix='nanacoin-provision-') as temp:
        table = pathlib.Path(temp) / 'partitions.bin'
        output = esptool(b, args.port, '--before', b.before, '--after', 'no_reset',
                         'read_flash', '0x8000', '0x1000', str(table), capture=True)
        try:
            mac = verify_mac(b, output)
            try:
                found = parse_partition_table(table.read_bytes())
            except ValueError:
                found = None  # blank or foreign firmware: nothing of ours to protect
        except ValueError as error:
            raise SystemExit(str(error)) from None
    banks = [other.id for other in BOARDS.values() if found == other.layout()]
    if banks and not args.replace_existing_bank:
        raise SystemExit(f'Chip {mac} already holds a NanaCoin {banks[0]} bank and its ledger; refusing to erase. '
                         'Use scripts/deploy.py to upgrade it.')
    print(f'Verified {b.id} board MAC {mac}; existing firmware is {"NanaCoin " + banks[0] if banks else "not a NanaCoin bank"}.')
    # Erase and write in one esptool session (the S2 port re-enumerates between sessions).
    esptool(b, args.port, '--before', 'no_reset', '--after', 'hard_reset', 'write_flash', '--erase-all',
            '--flash_mode', 'dio', '--flash_size', b.flash_size,
            hex(b.bootloader_offset), str(args.bootloader),
            '0x8000', str(args.partition_table),
            '0x10000', str(args.image))
    print(f'Provisioned the {b.id} bank. After restart open http://{b.hostname}/trust, then https://{b.hostname}/ to create its household.')


if __name__ == '__main__':
    try:
        main()
    except subprocess.CalledProcessError as error:
        raise SystemExit(f'esptool failed: {error}') from None
