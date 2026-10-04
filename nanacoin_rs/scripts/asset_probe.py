"""NanaCoin's bundles for miniframework's asset checks (boardsafe.assets):
the complete embedded site, compressed and cached replies, byte for byte."""
from pathlib import Path

import boards  # noqa: F401  (puts boardsafe on the import path)
from boardsafe import assets as _assets

ROOT = Path(__file__).resolve().parents[1]

BoardConnection = _assets.BoardConnection
verify_asset = _assets.verify_asset


def bundled_assets(web_dir=".embuild/web"):
    """(uri, identity, gzip, gzip_only) for each asset of one board's bundle."""
    return _assets.bundled_assets(ROOT / web_dir)


def verify_board_assets(address, hostname, context, workers=3, web_dir=".embuild/web"):
    return _assets.verify_board_assets(address, hostname, context, ROOT / web_dir, workers)
