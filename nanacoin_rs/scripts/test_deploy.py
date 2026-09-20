"""Offline safety tests; neither test invokes esptool or opens a serial port."""
import importlib.util
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location('deploy', Path(__file__).with_name('deploy.py'))
deploy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(deploy)


def table(entries):
    return b''.join(struct.pack('<HBBII16sI', 0x50AA, *values, name.encode(), 0)
                    for name, values in entries.items()) + b'\xff' * 32


class PartitionSafety(unittest.TestCase):
    def test_current_layout(self):
        deploy.verify_partition_table(table(deploy.EXPECTED))

    def test_other_layout_rejected(self):
        for name in deploy.EXPECTED:
            modified = dict(deploy.EXPECTED)
            del modified[name]
            with self.assertRaises(ValueError):
                deploy.verify_partition_table(table(modified))
        modified = dict(deploy.EXPECTED)
        modified['ledger'] = (1, 2, 0x210000, 0x800000)
        with self.assertRaises(ValueError):
            deploy.verify_partition_table(table(modified))

    def test_invalid_rejected(self):
        for bad in (b'', b'x' * 32, b'\xff' * 4096):
            with self.assertRaises(ValueError):
                deploy.verify_partition_table(bad)


if __name__ == '__main__':
    unittest.main()
