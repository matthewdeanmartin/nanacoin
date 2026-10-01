"""Exercise NanaCoin's actual background relay against a disposable Minicloud.

Build both desktop binaries first. Never contacts or changes a real board.
"""
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
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
SUFFIX = '.exe' if os.name == 'nt' else ''
NANA = Path(os.environ.get('CARGO_TARGET_DIR', str(ROOT / 'target'))) / 'debug' / ('nanacoin' + SUFFIX)
CLOUD = Path(os.environ.get('MINICLOUD_EXE', str(ROOT.parents[1] / 'mastomini/minicloud_rs/target/debug' / ('minicloud' + SUFFIX))))


def port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def http(base, path, value=None, token='', key=''):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    if key:
        headers['Idempotency-Key'] = key
    req = Request(base + path, data=None if value is None else json.dumps(value).encode(), headers=headers)
    try:
        response = urlopen(req, timeout=5)
    except HTTPError as error:
        response = error
    with response:
        return response.status, json.loads(response.read())


def wait(check, timeout=40):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        try:
            result = check()
            if result:
                return result
        except OSError:
            pass
        time.sleep(.1)
    raise AssertionError('Relay did not reach expected state')


def main():
    bank_port, cloud_port, mqtt_port = port(), port(), port()
    bank = f'http://127.0.0.1:{bank_port}/api/v1'
    cloud = f'http://127.0.0.1:{cloud_port}'
    processes = []
    with tempfile.TemporaryDirectory(prefix='nanacoin-minicloud-') as directory:
        root = Path(directory)
        with (root / 'process.log').open('w') as log:
            def start(binary, env):
                process = subprocess.Popen([str(binary.resolve())], env=dict(os.environ, **env), stdout=log, stderr=log, creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
                processes.append(process)
                return process

            def start_cloud():
                process = start(CLOUD, dict(MINICLOUD_PORT=str(cloud_port), MINICLOUD_MQTT_PORT=str(mqtt_port), MINICLOUD_BIND='127.0.0.1', MINICLOUD_DATA=str(root / 'cloud'), MINICLOUD_ADMIN_TOKEN='integration-minicloud-token'))
                wait(lambda: http(cloud, '/api/status')[0] == 200)
                return process

            def login(username):
                verifier = secrets.token_urlsafe(32)
                challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).decode().rstrip('=')
                status, auth = http(bank, '/auth/authorize', dict(username=username, password='1234', code_challenge=challenge, code_challenge_method='S256', redirect_uri='http://localhost/'))
                assert status == 200, auth
                status, result = http(bank, '/auth/token', dict(code=auth['code'], code_verifier=verifier, redirect_uri='http://localhost/'))
                assert status == 200, result
                return result['access_token']

            def notices():
                return [n for n in http(cloud, '/api/screen')[1]['notices'] if not n['id'].startswith('stats-')]

            try:
                # Queue while the display is offline, then restart NanaCoin.
                settings = dict(NANACOIN_PORT=str(bank_port), NANACOIN_JOURNAL=str(root / 'bank'), NANACOIN_MINICLOUD_URL=cloud)
                process = start(NANA, settings)
                wait(lambda: http(bank, '/status')[0] == 200)
                assert http(bank, '/provision', dict(household_name='Test house', username='nana', display_name='Nana', password='1234'))[0] == 201
                nana = login('nana')
                assert http(bank, '/users', dict(username='alice', display_name='Alice', password='1234', grant=False), nana)[0] == 201
                terms = dict(kind='SIMPLE', title='Kitchen draw', ticket_price=1, closes_at=int(time.time())+3600, rate_bps=0)
                status, lotto = http(bank, '/lottos', terms, nana, 'lotto-once')
                assert status == 200, lotto
                assert http(bank, '/lottos', terms, nana, 'lotto-once') == (status, lotto)
                process.terminate(); process.wait(timeout=5)
                start_cloud()
                start(NANA, settings)
                wait(lambda: http(bank, '/status')[0] == 200)
                received = wait(lambda: notices())
                assert len(received) == 1, received
                assert received[0]['text'] == 'New Lotto created: Kitchen draw', received
                assert 86300 < received[0]['expires_at'] - int(time.time()) <= 86400, received
                nana = login('nana'); alice = login('alice')
                status, message = http(bank, '/transfers', dict(to='account-2', amount=0, memo='Dinner is ready'), nana, 'mail-once')
                assert status == 201, message
                assert len(notices()) == 1  # unchecked mail stays in NanaCoin
                path = '/transactions/' + message['id'] + '/screen'
                assert http(bank, path, {}, alice)[0] == 403
                assert http(bank, path, {}, nana) == (200, {'queued': True})
                assert http(bank, path, {}, nana)[0] == 200
                wait(lambda: len(notices()) == 2)
                mail = next(n for n in notices() if n['text'] == 'Nana: Dinner is ready')
                assert mail['recipient'] == 'Alice'
                assert mail['expires_at'] == message['created_at'] + 86400
                assert http(bank, path + '/read', {}, nana)[0] == 403
                assert http(bank, path + '/read', {}, alice)[0] == 200
                wait(lambda: len(notices()) == 1)
                # Minicloud itself accepts an unauthenticated producer.
                status, result = http(cloud, '/api/screen/notify', dict(event_id='public-notify', source='household', id='public', recipient='All', text='Public posting', size='small'))
                assert status == 202, result
                wait(lambda: len(notices()) == 2)
                assert http(cloud, '/api/screen/read', dict(event_id='public-read', source='household', id='public'))[0] == 202
                wait(lambda: len(notices()) == 1)
                all_notices=lambda: http(cloud, '/api/screen')[1]['notices']
                wait(lambda: len([n for n in all_notices() if n['id'].startswith('stats-')]) == 2)
                screen=http(cloud,'/api/screen')[1]
                assert (screen['width'],screen['height']) == (320,172)
                assert screen['pages'] >= 1 and screen['page_text']
                # Borrower application -> lender proposal -> borrower consent.
                assert http(bank,'/admin/issue',dict(to='account-1',amount=10000,reason='Disposable lending funds'),nana,'fund-lender')[0] == 201
                application=dict(borrower='account-2',amount=100,installment=20,rate_bps=500,rate_days=365,payment_days=7,credit=False,memo='Kitchen project')
                status,wanted=http(bank,'/loans/request',application,alice,'application-once')
                assert status==200 and wanted['status']=='REQUESTED',(status,wanted)
                assert http(bank,'/loans/request',application,alice,'application-once')==(status,wanted)
                id=wanted['id']; endpoint='/loans/'+str(id)
                assert http(bank,endpoint+'/accept',{},alice,'too-early')[0] != 200
                book=http(bank,'/loans',token=nana)[1]
                assert any(l['id']==id and l['status']=='REQUESTED' for l in book['loans'])
                status,proposal=http(bank,endpoint+'/offer',application,nana,'propose-once')
                assert status==200 and proposal['status']=='OFFERED',(status,proposal)
                assert http(bank,endpoint+'/accept',{},nana,'wrong-actor')[0]==403
                status,funded=http(bank,endpoint+'/accept',{},alice,'accept-loan-once')
                assert status==200 and funded['status']=='ACTIVE',(status,funded)
                assert http(bank,endpoint+'/accept',{},alice,'accept-loan-once')==(status,funded)
                wait(lambda:any(n['text']=='Loan accepted' for n in all_notices()))
                assert http(bank,'/status')[1]['ledger_balanced']
                print('PASS: offline outbox/restart, lotto event/idempotency, optional mail copy, sender/recipient authorization, public posting, read dismissal, original 24-hour expiry, two stats cards, landscape pagination, keyed loan applications/proposals/borrower consent')
            finally:
                for process in reversed(processes):
                    if process.poll() is None:
                        process.terminate()
                    process.wait(timeout=5)


if __name__ == '__main__':
    main()
