"""Offline conversion and board/size gate. Run with a Python that has esptool 4.x.

Usage: firmware-image.py s3|s2 <elf>
"""
import pathlib
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from boards import BOARDS, board  # noqa: E402


def check_image(b, image: pathlib.Path):
    """Refuse an application image that is not this board's NanaCoin build."""
    data = image.read_bytes()
    if not data or data[0] != 0xE9 or len(data) < 24:
        raise ValueError(f'{image} is not an Espressif application image')
    chip_id = int.from_bytes(data[12:14], 'little')
    if chip_id != b.image_chip_id:
        raise ValueError(f'{image} is for chip ID {chip_id}, not {b.chip} ({b.image_chip_id}); refusing.')
    if b.marker not in data:
        raise ValueError(f'{image} lacks the {b.id} board marker ({b.hostname}); refusing.')
    for other in BOARDS.values():
        if other.id != b.id and other.marker in data:
            raise ValueError(f'{image} is marked for board {other.id} ({other.hostname}); refusing.')
    if not 0 < len(data) <= b.app_size():
        raise ValueError(f'Firmware is {len(data)} bytes; the {b.id} factory partition is {b.app_size()} bytes. Refusing deployment.')
    return len(data)


def main():
    if len(sys.argv) != 3:
        raise SystemExit('Usage: firmware-image.py s3|s2 <elf>')
    b = board(sys.argv[1])
    elf = pathlib.Path(sys.argv[2]).resolve()
    image = elf.with_suffix('.bin')
    subprocess.run([sys.executable, '-m', 'esptool', '--chip', b.chip,
                    'elf2image', '--flash_mode', 'dio', '--flash_size', b.flash_size,
                    '--output', str(image), str(elf)], check=True)
    try:
        size = check_image(b, image)
    except ValueError as error:
        raise SystemExit(str(error)) from None
    print(f'Board {b.id} firmware: {image} ({size} / {b.app_size()} bytes, {b.hostname}). No board accessed.')


if __name__ == '__main__':
    main()
