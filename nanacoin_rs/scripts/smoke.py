"""Exercise the JSON API using the existing Angular client's contract."""
import base64
import hashlib
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import zlib

ROOT = Path(__file__).resolve().parents[1]
TARGET = Path(os.environ.get('CARGO_TARGET_DIR', str(ROOT / 'target')))
EXE = (TARGET / 'debug' / ('nanacoin.exe' if os.name == 'nt' else 'nanacoin')).resolve()


def main():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    base = f'http://127.0.0.1:{port}'

    def request(path, payload=None, token='', key='', method=None, origin='http://localhost:4200'):
        headers = {'Origin': origin}
        if token:
            headers['Authorization'] = 'Bearer ' + token
        if key:
            headers['Idempotency-Key'] = key
        data = None if payload is None else json.dumps(payload).encode()
        if data is not None:
            headers['Content-Type'] = 'application/json'
        req = urllib.request.Request(base + '/api/v1' + path, data=data, headers=headers, method=method)
        try:
            response = urllib.request.urlopen(req, timeout=10)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            assert 'application/json' in response.headers['Content-Type']
            if origin == 'http://localhost:4200':
                assert response.headers['Access-Control-Allow-Origin'] == origin
            data = response.read()
            return response.status, json.loads(data) if data else None

    def login(username, password):
        verifier = secrets.token_urlsafe(32)
        challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).decode().rstrip('=')
        status, auth = request('/auth/authorize', dict(username=username, password=password, code_challenge=challenge, code_challenge_method='S256', redirect_uri='http://localhost:4200/'))
        assert status == 200, auth
        status, grant = request('/auth/token', dict(code=auth['code'], code_verifier=verifier, redirect_uri='http://localhost:4200/'))
        assert status == 200, grant
        assert grant['user']['username'] == username
        assert grant['expires_in'] == 28800
        return grant['access_token']

    with tempfile.TemporaryDirectory(prefix='nanacoin-rs-smoke-') as temp:
        env = dict(os.environ, NANACOIN_PORT=str(port), NANACOIN_JOURNAL=str(Path(temp) / 'ledger'))

        def start():
            process = subprocess.Popen([str(EXE)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
            for _ in range(100):
                if process.poll() is not None:
                    raise RuntimeError(process.stderr.read().decode())
                try:
                    if request('/status')[0] == 200:
                        return process
                except (OSError, urllib.error.URLError):
                    time.sleep(.05)
            process.terminate()
            process.wait(timeout=5)
            raise RuntimeError('server did not start')

        def stop(process):
            process.terminate()
            process.wait(timeout=5)
            process.stderr.close()

        process = start()
        try:
            try:
                with urllib.request.urlopen(base + '/', timeout=5) as response:
                    assert response.headers.get_content_type() == 'text/html'
                    assert b'<app-root' in response.read()
            except urllib.error.HTTPError as error:
                assert error.code == 404
                assert json.loads(error.read())['error'] == 'not_found'
            assert request('/status')[1]['provisioned'] is False
            assert request('/me', method='OPTIONS')[0] == 200
            assert request('/status', origin='https://not-allowed.example')[0] == 403
            provision = dict(household_name='Home', username='nana', display_name='Nana', password='1234')
            assert request('/provision', provision)[0] == 201
            assert request('/provision', provision)[0] == 403
            nana = login('nana', '1234')
            assert request('/users', dict(username='alice', display_name='Alice', password='5678', grant=False), nana)[0] == 201
            alice = login('alice', '5678')
            issue = dict(to='account-2', amount=25, reason='Cookies 🍪')
            first = request('/admin/issue', issue, nana, 'issue-one')
            assert first[0] == 201, first
            assert request('/admin/issue', issue, nana, 'issue-one') == first
            assert request('/admin/issue', issue, alice, 'forbidden')[0] == 403
            assert request('/transfers', dict(to='account-1', amount=5, memo='Thanks'), alice, 'transfer-one')[0] == 201
            assert request('/me', token=alice)[1]['balance'] == 20
            assert len(request('/users', token=nana)[1]['users']) == 2
            assert request('/listings', token=nana)[1]['listings'] == []
            result = request('/listings', dict(title='Chore', description='Wash dishes', price=3, side='SELL'), nana)
            assert result[0] == 201, result
            listing = result[1]['id']
            result = request('/listings/' + listing, {'description':'Dry dishes', 'price':2}, nana, method='PATCH')
            assert result[0] == 200 and result[1]['description'] == 'Dry dishes', result
            assert request('/listings/' + listing + '/cancel', {}, nana)[0] == 200
            result = request('/listings', dict(title='Cookies', description='Saturday', price=12, side='SELL'), nana)
            offer_listing = result[1]['id']
            assert request('/offers', token=alice) == (200, {'offers': []})
            result = request('/listings/' + offer_listing + '/offers', dict(amount=7, message='Would you take seven?'), alice)
            assert result[0] == 201, result
            offer_path = '/offers/' + result[1]['id']
            accepted = request(offer_path + '/accept', {}, nana, 'accept-offer')
            assert accepted[0] == 201 and accepted[1]['offer']['reversible'], accepted
            assert request('/me', token=alice)[1]['balance'] == 13
            undone = request(offer_path + '/unaccept', {'reason': 'Not delivered'}, alice, 'undo-offer')
            assert undone[0] == 200 and undone[1]['offer']['status'] == 'REVERSED', undone
            assert request('/me', token=alice)[1]['balance'] == 20
            for _ in range(1000):
                page = request('/offers', token=alice)
                assert page[0] == 200 and page[1]['offers'][0]['status'] == 'REVERSED'
            assert request('/admin/config', token=nana)[1]['initial_grant'] == 100
            assert request('/accounts/account-2/transactions?limit=50', token=alice)[1]['balance'] == 20
            assert request('/users/user-2', {'status':'DISABLED'}, nana, method='PATCH')[0] == 200
            assert request('/me', token=alice)[0] == 401
            assert request('/users/user-2', {'status':'ACTIVE'}, nana, method='PATCH')[0] == 200
            alice = login('alice', '5678')
            assert request('/accounts/account-1', token=alice)[0] == 403
            assert request('/transactions', token=alice)[0] == 403
            assert request('/state', token=alice)[0] == 403
            assert request('/admin/issue-usd', dict(to='account-1', cents=500, reason='Cash reserve'), nana, 'usd-one')[0] == 201
            quote = request('/quotes', dict(side='ASK', cents_per_coin=25, coins=1), alice)
            assert quote[0] == 201, quote
            quote_path = '/quotes/' + quote[1]['id']
            trade = request(quote_path + '/take', {}, nana, 'trade-one')
            assert trade[0] == 201 and trade[1]['quote']['status'] == 'FILLED', trade
            assert request('/accounts/account-2-usd', token=alice)[1]['balance'] == 25
            assert request('/me', token=alice)[1]['balance'] == 19
            assert request(quote_path, token=alice)[1]['cash_tx'] == trade[1]['cash_transaction']['id']
            assert request('/commands', {'junk':'x' * 1100}, nana)[0] == 413
            assert request('/auth/logout', {}, nana)[0] == 204
            assert request('/me', token=nana)[0] == 401
        finally:
            stop(process)
        process = start()
        try:
            assert request('/me', token=alice)[0] == 401
            nana = login('nana', '1234')
            alice = login('alice', '5678')
            assert request('/me', token=alice)[1]['balance'] == 19
            assert request('/admin/issue', issue, nana, 'issue-one') == first
            assert request(quote_path + '/take', {}, nana, 'trade-one') == trade
            status, ledger = request('/transactions', token=nana)
            assert status == 200 and len(ledger['transactions']) == 7
            assert request(offer_path + '/accept', {}, nana, 'accept-offer') == accepted
            assert request(offer_path + '/unaccept', {'reason': 'Not delivered'}, alice, 'undo-offer') == undone
            assert all('password' not in json.dumps(user) for user in request('/users', token=nana)[1]['users'])
        finally:
            stop(process)
        # A disposable token-era Rust journal verifies the actual migration CLI.
        legacy_token = 'ab' * 32
        events = [
            {'version':1, 'sequence':1, 'actor':1, 'request_id':1,
             'command':{'add_member':{'name':'Nana', 'token_hash':list(hashlib.sha256(legacy_token.encode()).digest())}}},
            {'version':1, 'sequence':2, 'actor':1, 'request_id':2,
             'command':{'issue':{'to':1, 'amount':25, 'memo':'Preserve balance'}}},
        ]
        legacy_path = Path(temp) / 'legacy-ledger'
        with legacy_path.open('wb') as journal:
            for event in events:
                data = json.dumps(event, separators=(',', ':')).encode()
                frame = b'NCR1' + len(data).to_bytes(4, 'little') + zlib.crc32(data).to_bytes(4, 'little') + data
                journal.write(frame.ljust(1024, b'\0'))
        env['NANACOIN_JOURNAL'] = str(legacy_path)
        migration = subprocess.run([str(EXE), '--migrate-login'], env=env,
            input=json.dumps(dict(token=legacy_token, username='nana', password='4321')).encode(),
            capture_output=True, creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        assert migration.returncode == 0, migration.stderr.decode()
        process = start()
        try:
            assert request('/me', token=legacy_token)[0] == 401
            assert request('/me', token=login('nana', '4321'))[1]['balance'] == 25
        finally:
            stop(process)
    print('HTTP smoke passed: JSON API, Angular auth/views/money/offers/forex/privacy, 1000 offer reads, CORS, revocation, restart, durable retries')


if __name__ == '__main__':
    main()
