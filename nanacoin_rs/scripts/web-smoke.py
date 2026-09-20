"""Real local HTTP checks for the bundled application; disposable ledger only."""
import gzip
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
TARGET = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
EXE = TARGET / 'debug' / ('nanacoin.exe' if os.name == 'nt' else 'nanacoin')


def main():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    base = f'http://127.0.0.1:{port}'

    def get(path, headers=None, method='GET', data=None):
        req = urllib.request.Request(base + path, headers=headers or {}, method=method, data=data)
        try:
            response = urllib.request.urlopen(req, timeout=5)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            return response.status, response.headers, response.read()

    with tempfile.TemporaryDirectory(prefix='nanacoin-web-smoke-') as temp:
        env = {**os.environ, 'NANACOIN_PORT': str(port), 'NANACOIN_JOURNAL': str(Path(temp) / 'test.journal')}
        process = subprocess.Popen([str(EXE.resolve())], env=env, stdout=subprocess.DEVNULL)
        try:
            for _ in range(100):
                if process.poll() is not None:
                    raise RuntimeError('Server exited before ready')
                try:
                    status, _, html = get('/')
                    break
                except urllib.error.URLError:
                    time.sleep(.05)
            else:
                raise RuntimeError('Server did not start')
            assert status == 200 and b'<app-root' in html
            status, headers, certificate = get('/ca')
            assert status == 200 and certificate == (ROOT / 'certs/home-ca.der').read_bytes()
            assert headers['Content-Type'] == 'application/x-x509-ca-cert'
            fingerprint = ':'.join(f'{b:02X}' for b in hashlib.sha256(certificate).digest()).encode()
            assert fingerprint in get('/trust')[2]
            for secret in ['/rootCA-key.pem', '/certs/nanacoin-ca-signed.key', '/.local/ca/rootCA-key.pem']:
                assert get(secret)[0] == 404
            assert b'name="nanacoin-api" content=""' in html
            assets = ROOT.parent / 'nanacoin_go/angular/dist/nanacoin-web/browser'
            for path in assets.rglob('*'):
                if not path.is_file():
                    continue
                uri = '/' + path.relative_to(assets).as_posix()
                status, headers, raw = get(uri)
                assert status == 200, uri
                if path.suffix in ('.js', '.css') and '-' in path.name:
                    assert 'immutable' in headers['Cache-Control'], uri
                status, zipped_headers, zipped = get(uri, {'Accept-Encoding': 'gzip'})
                assert status == 200 and zipped_headers['Content-Encoding'] == 'gzip', uri
                assert gzip.decompress(zipped) == raw, uri
                status, _, unchanged = get(uri, {'If-None-Match': headers['ETag']})
                assert status == 304 and not unchanged, uri
                status, _, body = get(uri, method='HEAD')
                assert status == 405 and not body, uri
            assert get('/nana?test=1')[2] == html
            for missing in ['/missing.js', '/%2e%2e/config.py', '/config.py']:
                assert get(missing)[0] == 404
            assert get('/', method='POST', data=b'')[0] == 405
            assert get('/', {'Accept-Encoding': '*;q=0'})[0] == 406
            status, headers, body = get('/api/v1/not-a-route')
            assert status in (401, 404) and headers.get_content_type() == 'application/json' and b'<app-root' not in body
            assert get('/api/v1/status', {'Origin': base})[0] == 200
            status, _, _ = get('/api/v1/provision', {'Origin': base, 'Content-Type': 'application/json'},
                               method='POST', data=json.dumps(dict(household_name='Local UI',
                               username='nana', display_name='Nana', password='1234')).encode())
            assert status == 201, 'same-origin provisioning failed'
            # No auth: same-origin mutations reach the API rather than CORS rejection.
            status, _, _ = get('/api/v1/auth/logout', {'Origin': base}, method='POST', data=b'{}')
            assert status != 403
            print('Bundled HTTP smoke passed: all assets, gzip, method rejection, ETags, SPA routes, API isolation and same-origin requests.')
        finally:
            process.terminate()
            process.wait(timeout=10)


if __name__ == '__main__':
    main()
