"""Offline safety tests; no test invokes esptool or opens a serial port."""
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from boards import BOARDS  # noqa: E402

spec = importlib.util.spec_from_file_location('deploy', Path(__file__).with_name('deploy.py'))
deploy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(deploy)
S2, S3, P4 = BOARDS['s2'], BOARDS['s3'], BOARDS['p4']


def table(entries):
    return b''.join(struct.pack('<HBBII16sI', 0x50AA, *values, name.encode(), 0)
                    for name, values in entries.items()) + b'\xff' * 32


def image(b, marker=None, size=4096):
    header = bytearray(b'\xe9' + b'\0' * 23)
    header[12:14] = b.image_chip_id.to_bytes(2, 'little')
    body = bytes(header) + (marker if marker is not None else b.marker)
    return body + b'\0' * (size - len(body))


class PartitionSafety(unittest.TestCase):
    def test_current_layouts(self):
        for b in BOARDS.values():
            deploy.verify_partition_table(b, table(b.layout()))

    def test_s3_layout_is_unchanged(self):
        self.assertEqual(S3.layout(), {
            'nvs': (1, 2, 0x9000, 0x6000),
            'phy_init': (1, 1, 0xF000, 0x1000),
            'factory': (0, 0, 0x10000, 0x400000),
            'ledger': (1, 2, 0x410000, 0x800000),
        })

    def test_s2_layout_fits_4mib(self):
        layout = S2.layout()
        self.assertEqual(max(offset + size for _, _, offset, size in layout.values()), 0x400000)

    def test_p4_layout_and_cross_board_guards(self):
        self.assertEqual(P4.image_chip_id, 18)
        self.assertEqual(P4.bootloader_offset, 0x2000)
        self.assertEqual(P4.app_size(), 8 * 1024 * 1024)
        self.assertLessEqual(max(o + s for _, _, o, s in P4.layout().values()), 32 * 1024 * 1024)
        self.assertEqual(deploy.verify_mac(P4, f'Chip is ESP32-P4\nMAC: {P4.mac}\n'), P4.mac)
        for other in (S2, S3):
            with self.assertRaises(ValueError):
                deploy.verify_partition_table(P4, table(other.layout()))
            with self.assertRaises(ValueError):
                deploy.verify_mac(P4, f'MAC: {other.mac}\n')
            with self.assertRaises(ValueError):
                deploy.verify_mac(other, f'MAC: {P4.mac}\n')
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'app.bin'
            path.write_bytes(image(P4))
            deploy.check_image(P4, path)
            for other in (S2, S3):
                with self.assertRaises(ValueError):
                    deploy.check_image(other, path)
                path.write_bytes(image(P4, marker=other.marker))
                with self.assertRaises(ValueError):
                    deploy.check_image(P4, path)

    def test_other_bank_layout_rejected_by_name(self):
        with self.assertRaisesRegex(ValueError, 'has the s2 layout'):
            deploy.verify_partition_table(S3, table(S2.layout()))
        with self.assertRaisesRegex(ValueError, 'has the s3 layout'):
            deploy.verify_partition_table(S2, table(S3.layout()))

    def test_other_layout_rejected(self):
        for b in BOARDS.values():
            for name in b.layout():
                modified = dict(b.layout())
                del modified[name]
                with self.assertRaises(ValueError):
                    deploy.verify_partition_table(b, table(modified))
        modified = dict(S3.layout())
        modified['ledger'] = (1, 2, 0x210000, 0x800000)
        with self.assertRaises(ValueError):
            deploy.verify_partition_table(S3, table(modified))

    def test_invalid_rejected(self):
        for b in BOARDS.values():
            for bad in (b'', b'x' * 32, b'\xff' * 4096):
                with self.assertRaises(ValueError):
                    deploy.verify_partition_table(b, bad)


class BoardIdentity(unittest.TestCase):
    def test_mac_must_match_board(self):
        self.assertEqual(deploy.verify_mac(S2, f'Chip is ESP32-S2\nMAC: {S2.mac.upper()}\n'), S2.mac)
        with self.assertRaisesRegex(ValueError, 'is the s3 board'):
            deploy.verify_mac(S2, f'MAC: {S3.mac}\n')
        with self.assertRaisesRegex(ValueError, 'is the s2 board'):
            deploy.verify_mac(S3, f'MAC: {S2.mac}\n')
        with self.assertRaisesRegex(ValueError, 'not the recorded'):
            deploy.verify_mac(S3, 'MAC: 00:11:22:33:44:55\n')
        with self.assertRaisesRegex(ValueError, 'did not report'):
            deploy.verify_mac(S3, 'no mac here')

    def test_image_must_be_this_boards_build(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'app.bin'
            for b in BOARDS.values():
                path.write_bytes(image(b))
                deploy.check_image(b, path)
            path.write_bytes(image(S2))
            with self.assertRaisesRegex(ValueError, 'chip ID'):
                deploy.check_image(S3, path)
            # Right chip header, wrong bank marker.
            path.write_bytes(image(S3, marker=S2.marker))
            with self.assertRaisesRegex(ValueError, 'lacks the s3 board marker'):
                deploy.check_image(S3, path)
            path.write_bytes(image(S2, marker=S2.marker + S3.marker))
            with self.assertRaisesRegex(ValueError, 'marked for board s3'):
                deploy.check_image(S2, path)
            path.write_bytes(image(S2, size=S2.app_size() + 1))
            with self.assertRaisesRegex(ValueError, 'app partition'):
                deploy.check_image(S2, path)

    def test_boards_are_distinct(self):
        for field in ('hostname', 'mac', 'target_dir', 'web_dir', 'cert', 'chip', 'target', 'marker'):
            self.assertNotEqual(getattr(S2, field), getattr(S3, field), field)
        self.assertNotIn(S3.marker, S2.marker)
        self.assertNotIn(S2.marker, S3.marker)


if __name__ == '__main__':
    unittest.main()
