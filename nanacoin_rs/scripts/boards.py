"""The two NanaCoin banks. Every build, deploy, provision and probe step reads
its board from here, so one board's image, certificate, bundle or hostname can
never be used for the other.

The checks themselves are miniframework's boardsafe (tools/boardsafe); this
file is NanaCoin's registry for them. Replacing a physical board is a
deliberate edit to `mac` below.
"""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
FRAMEWORK_TOOLS = ROOT.parents[1] / 'microcontroller' / 'miniframework' / 'tools' / 'boardsafe' / 'src'
if str(FRAMEWORK_TOOLS) not in sys.path:
    sys.path.insert(0, str(FRAMEWORK_TOOLS))

from boardsafe.boards import Board, pick  # noqa: E402

BOARDS = {
    's3': Board(
        id='s3', name='ESP32-S3-N16R8 (first bank)', app='nanacoin', chip='esp32s3',
        target='xtensa-esp32s3-espidf', flash_size='16MB', bootloader_offset=0x0,
        hostname='nanacoin.local', mac='ac:a7:04:2c:2c:04', target_dir='C:/ncr',
        binary='nanacoin-esp32', sdkconfig='sdkconfig.defaults', partitions='partitions.csv',
        web_dir='.embuild/web', cert='nanacoin-ca-signed', ca='certs/home-ca.crt',
        root=ROOT, before='default_reset'),
    's2': Board(
        id='s2', name='ESP32-S2 Mini (second bank)', app='nanacoin', chip='esp32s2',
        target='xtensa-esp32s2-espidf', flash_size='4MB', bootloader_offset=0x1000,
        hostname='nanacoin-s2.local', mac='80:65:99:f0:1c:9c', target_dir='C:/ncr-s2',
        binary='nanacoin-esp32', sdkconfig='boards/s2/sdkconfig.defaults',
        partitions='boards/s2/partitions.csv', web_dir='.embuild/web-s2',
        cert='nanacoin-s2-ca-signed', ca='certs/home-ca.crt', root=ROOT,
        # The S2 has no bridge chip: an operator puts it in ROM download mode.
        before='no_reset'),
}


def board(name):
    """Look up a board; there is deliberately no default."""
    return pick(BOARDS, name)


if __name__ == '__main__':
    # Shell helper: python scripts/boards.py s2 target
    if len(sys.argv) != 3:
        raise SystemExit('Usage: python scripts/boards.py <s3|s2> <field>')
    value = getattr(board(sys.argv[1]), sys.argv[2])
    print(value() if callable(value) else value)
