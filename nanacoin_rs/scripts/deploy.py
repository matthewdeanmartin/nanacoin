"""Update only the application on an already-provisioned Rust partition layout.

No full-chip erase, partition rewrite, bootloader replacement, recovery flash,
or serial monitor. A different layout requires a separately reviewed migration.
"""
import argparse
import pathlib
import struct
import subprocess
import sys
import tempfile

EXPECTED = {
    'nvs': (1, 2, 0x9000, 0x6000),
    'phy_init': (1, 1, 0xF000, 0x1000),
    'factory': (0, 0, 0x10000, 0x400000),
    'ledger': (1, 2, 0x410000, 0x800000),
}


def verify_partition_table(data):
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
    if found != EXPECTED:
        raise ValueError('Board is not using the expected Rust partition layout; refusing to write. No automatic migration.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', required=True)
    parser.add_argument('--image', type=pathlib.Path, required=True)
    parser.add_argument('--dry-run', action='store_true', help='Print plan without opening a serial port')
    args = parser.parse_args()
    image = args.image.resolve()
    if not image.is_file() or not 0 < image.stat().st_size <= 0x400000:
        parser.error('Missing/oversized application image; build firmware first')
    with image.open('rb') as stream:
        if stream.read(1) != b'\xe9':
            parser.error('Not an Espressif application image')
    print(f'Plan: verify partition table on {args.port}, then write {image} at 0x10000 only.')
    print('Ledger, configuration, bootloader and partition table will not be written.')
    if args.dry_run:
        return
    tool = [sys.executable, '-m', 'esptool', '--chip', 'esp32s3', '--port', args.port]
    with tempfile.TemporaryDirectory(prefix='nanacoin-partitions-') as temp:
        table = pathlib.Path(temp) / 'partitions.bin'
        subprocess.run([*tool, 'read_flash', '0x8000', '0x1000', str(table)], check=True)
        verify_partition_table(table.read_bytes())
    subprocess.run([*tool, 'write_flash', '0x10000', str(image)], check=True)
    print('Application updated. Open https://nanacoin.local/ after restart.')


if __name__ == '__main__':
    main()
