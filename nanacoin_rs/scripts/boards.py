"""The two NanaCoin banks. Every build, deploy, provision and probe step reads
its board from here, so one board's image, certificate, bundle or hostname can
never be used for the other.

Replacing a physical board is a deliberate edit to `mac` below.
"""
import csv
from dataclasses import dataclass
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class Board:
    id: str
    name: str
    chip: str               # esptool --chip
    image_chip_id: int      # Espressif image header chip ID
    target: str             # Rust target triple
    flash_size: str
    bootloader_offset: int
    hostname: str
    mac: str                # this household's physical board
    target_dir: str         # CARGO_TARGET_DIR on the Windows build PC
    sdkconfig: str
    partitions: str
    web_dir: str
    cert: str               # certs/<cert>.crt and .key
    # esptool --before for the first command of a flash session. The S2 has no
    # bridge chip: an operator puts it in ROM download mode (BOOT+RST) first.
    before: str

    @property
    def manual_download(self) -> bool:
        return self.before == 'no_reset'

    @property
    def marker(self) -> bytes:
        return f'NANACOIN-BOARD:{self.id}:{self.hostname};'.encode('ascii')

    def layout(self):
        """Partition rows as (type, subtype, offset, size), from the CSV."""
        kinds = {'app': 0, 'data': 1}
        subtypes = {'factory': 0, 'phy': 1, 'nvs': 2}
        rows = {}
        with (ROOT / self.partitions).open(newline='') as stream:
            for row in csv.reader(line for line in stream if line.strip() and not line.startswith('#')):
                name, kind, subtype, offset, size = (field.strip() for field in row[:5])
                rows[name] = (kinds[kind], subtypes[subtype], int(offset, 0), int(size, 0))
        return rows

    def app_size(self):
        return self.layout()['factory'][3]

    def image(self, target_dir=None):
        return Path(target_dir or self.target_dir) / self.target / 'release' / 'nanacoin-esp32.bin'


BOARDS = {
    's3': Board(
        id='s3', name='ESP32-S3-N16R8 (first bank)', chip='esp32s3', image_chip_id=9,
        target='xtensa-esp32s3-espidf', flash_size='16MB', bootloader_offset=0x0,
        hostname='nanacoin.local', mac='ac:a7:04:2c:2c:04', target_dir='C:/ncr',
        sdkconfig='sdkconfig.defaults', partitions='partitions.csv',
        web_dir='.embuild/web', cert='nanacoin-ca-signed', before='default_reset'),
    's2': Board(
        id='s2', name='ESP32-S2 Mini (second bank)', chip='esp32s2', image_chip_id=2,
        target='xtensa-esp32s2-espidf', flash_size='4MB', bootloader_offset=0x1000,
        hostname='nanacoin-s2.local', mac='80:65:99:f0:1c:9c', target_dir='C:/ncr-s2',
        sdkconfig='boards/s2/sdkconfig.defaults', partitions='boards/s2/partitions.csv',
        web_dir='.embuild/web-s2', cert='nanacoin-s2-ca-signed', before='no_reset'),
}


def board(name):
    """Look up a board; there is deliberately no default."""
    try:
        return BOARDS[name]
    except KeyError:
        raise SystemExit(f'Unknown board {name!r}; choose one of: {", ".join(BOARDS)}') from None


if __name__ == '__main__':
    # Shell helper: python scripts/boards.py s2 target
    if len(sys.argv) != 3:
        raise SystemExit('Usage: python scripts/boards.py <s3|s2> <field>')
    value = getattr(board(sys.argv[1]), sys.argv[2])
    print(value() if callable(value) else value)
