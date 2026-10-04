"""Update only the application on an already-provisioned NanaCoin bank.

Every write is preceded by four independent identity checks, any of which
stops the deployment before the flash is touched:

1. the image header's chip ID and the embedded board marker name this board;
2. esptool connects with this board's --chip (an S2 image never reaches an S3);
3. the connected chip's MAC is the one recorded for this board in boards.py;
4. the partition table on the chip is exactly this board's layout.

No full-chip erase, partition rewrite, bootloader replacement, recovery flash,
or serial monitor. First installation is scripts/provision.py.

The checks and the writer are miniframework's boardsafe (tools/boardsafe);
this script binds them to NanaCoin's registry (scripts/boards.py).
"""
import argparse
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from boards import BOARDS, board  # noqa: E402
from boardsafe import flash, image as image_checks  # noqa: E402


def check_image(b, image):
    return image_checks.check_image(b, image, BOARDS.values())


def parse_partition_table(data):
    return flash.parse_partition_table(data)


def verify_partition_table(b, data):
    flash.verify_partition_table(b, data, BOARDS.values())


def chip_mac(output):
    return flash.chip_mac(output)


def verify_mac(b, output):
    return flash.verify_mac(b, output, BOARDS.values())


# provision.py drives esptool the same way (port waits for the native-USB S2).
esptool = flash.esptool


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
    print('Ledger, configuration, bootloader and partition table will not be written.')
    flash.update(b, args.port, image, BOARDS.values(), dry_run=args.dry_run)


if __name__ == '__main__':
    main()
