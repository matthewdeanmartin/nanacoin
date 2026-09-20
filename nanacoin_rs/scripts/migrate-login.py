"""One-time local credential migration; never deletes or replaces the journal."""
import getpass
import json
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
os.chdir(root)
saved = root / '.local/admin-token'
token = saved.read_text().strip() if saved.exists() else getpass.getpass('Original account access token (migration only): ')
username = input('Choose your username: ').strip()
password = getpass.getpass('Choose your PIN or password (at least 4 characters): ')
if password != getpass.getpass('Confirm PIN or password: '):
    raise SystemExit('Passwords do not match; nothing changed.')
exe = Path(os.environ.get('CARGO_TARGET_DIR', str(root / 'target'))) / 'debug' / ('nanacoin.exe' if os.name == 'nt' else 'nanacoin')
result = subprocess.run([str(exe), '--migrate-login'], input=json.dumps(dict(token=token, username=username, password=password), ensure_ascii=False).encode(), creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
raise SystemExit(result.returncode)
