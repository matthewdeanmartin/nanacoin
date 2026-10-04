"""Offline conversion and board/size gate. Run with a Python that has esptool 4.x.

Usage: firmware-image.py s3|s2 <elf>
"""
import pathlib
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from boards import BOARDS, board  # noqa: E402
from boardsafe import image as boardsafe_image  # noqa: E402


def check_image(b, image: pathlib.Path):
    """Refuse an application image that is not this board's NanaCoin build
    (chip ID, board marker, no other bank's marker, fits the app partition)."""
    return boardsafe_image.check_image(b, image, BOARDS.values())


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
