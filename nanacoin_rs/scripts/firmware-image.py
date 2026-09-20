"""Offline conversion and size gate. Run with a Python that has esptool 4.x."""
import pathlib
import subprocess
import sys


def main():
    elf = pathlib.Path(sys.argv[1]).resolve()
    image = elf.with_suffix('.bin')
    subprocess.run([sys.executable, '-m', 'esptool', '--chip', 'esp32s3',
                    'elf2image', '--flash_mode', 'dio', '--flash_size', '16MB',
                    '--output', str(image), str(elf)], check=True)
    size = image.stat().st_size
    if not 0 < size <= 0x400000:
        raise SystemExit(f'Firmware is {size} bytes; factory partition is 4194304 bytes. Refusing deployment.')
    print(f'Combined firmware: {image} ({size} / 4194304 bytes). No board accessed.')


if __name__ == '__main__':
    main()
