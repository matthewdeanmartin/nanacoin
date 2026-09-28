"""Verify the complete embedded site, including compressed and cached replies."""
from concurrent.futures import ThreadPoolExecutor
from contextlib import closing
import gzip
import http.client
from pathlib import Path
import re
import socket
import time

ROOT = Path(__file__).resolve().parents[1]


def bundled_assets(web_dir=".embuild/web"):
    """(uri, identity, gzip, gzip_only) for each asset of one board's bundle.

    A gzip-only bundle (the S2) embeds an empty identity copy; its identity
    bytes are recovered from the embedded gzip for comparison.
    """
    generated = (ROOT / web_dir / "assets.rs").read_text(encoding="utf-8")
    assets = []
    for line in generated.splitlines():
        match = re.search(r'path: "([^"]+)".*?raw: include_bytes!\("([^"]+)"\).*?gzip: include_bytes!\("([^"]+)"\)', line)
        if match:
            uri, raw, zipped = match.groups()
            raw, zipped = Path(raw).read_bytes(), Path(zipped).read_bytes()
            gzip_only = not raw
            assets.append((uri, gzip.decompress(zipped) if gzip_only else raw, zipped, gzip_only))
    if not assets or not any(asset[0] == "/index.html" for asset in assets):
        raise RuntimeError("Could not identify the locally bundled assets")
    return assets


class BoardConnection:
    """Strict TLS over a specified address, with the certificate hostname/SNI.

    Keep requests on one connection to exercise the board's keep-alive path.
    Reconnect only when the server explicitly asks to close, never on truncated
    responses: a deployment check must fail instead of hiding lost transfers.
    """
    def __init__(self, address, hostname, context, slow=False):
        self.address = address
        self.hostname = hostname
        self.context = context
        self.tls = None
        self.slow = slow

    def close(self):
        if self.tls is not None:
            self.tls.close()
            self.tls = None

    def get(self, uri, headers):
        if self.tls is None:
            raw = socket.create_connection((self.address, 443), timeout=20)
            if self.slow:
                raw.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 4096)
            try:
                self.tls = self.context.wrap_socket(raw, server_hostname=self.hostname)
            except BaseException:
                raw.close()
                raise
        request = [f"GET {uri} HTTP/1.1", f"Host: {self.hostname}", "Cache-Control: no-cache"]
        request.extend(f"{name}: {value}" for name, value in headers.items())
        self.tls.sendall(("\r\n".join(request) + "\r\n\r\n").encode("ascii"))
        with http.client.HTTPResponse(self.tls) as response:
            response.begin()
            status = response.status
            fields = {key.lower(): value for key, value in response.getheaders()}
            close = response.will_close
            if self.slow:
                chunks = []
                while chunk := response.read(1024):
                    chunks.append(chunk)
                    time.sleep(0.03)
                body = b"".join(chunks)  # exact byte comparison also detects EOF
            else:
                body = response.read()  # raises IncompleteRead on premature EOF
        if close:
            self.close()
        return status, fields, body


def verify_asset(get, asset, require_length=True):
    uri, raw, zipped, gzip_only = asset
    etag = None
    if gzip_only:
        # No identity copy on the board: an identity-only client gets 406.
        status, headers, body = get(uri, {"Accept-Encoding": "identity"})
        if status != 406:
            raise RuntimeError(f"{uri} (identity): gzip-only board returned HTTP {status}, expected 406")
    for encoding, expected in ((("gzip", zipped),) if gzip_only else (("identity", raw), ("gzip", zipped))):
        status, headers, body = get(uri, {"Accept-Encoding": encoding})
        label = f"{uri} ({encoding})"
        if status != 200 or body != expected:
            raise RuntimeError(f"{label}: HTTP {status}, {len(body)} bytes; expected {len(expected)} exact bytes")
        length = headers.get("content-length")
        if length != str(len(expected)) and not (
            not require_length and length is None and headers.get("transfer-encoding") == "chunked"
        ):
            raise RuntimeError(f"{label}: incorrect Content-Length")
        if encoding == "gzip":
            if headers.get("content-encoding") != "gzip" or gzip.decompress(body) != raw:
                raise RuntimeError(f"{label}: invalid gzip representation")
        elif headers.get("content-encoding", "identity") != "identity":
            raise RuntimeError(f"{label}: unexpected content encoding")
        if "accept-encoding" not in headers.get("vary", "").lower():
            raise RuntimeError(f"{label}: missing Vary: Accept-Encoding")
        current = headers.get("etag")
        if not current or (etag is not None and current != etag):
            raise RuntimeError(f"{label}: missing or inconsistent representation ETag")
        etag = current
        status, headers, body = get(uri, {"Accept-Encoding": encoding, "If-None-Match": etag})
        if status != 304 or body or headers.get("etag") != etag:
            raise RuntimeError(f"{label}: invalid conditional response")


def verify_board_assets(address, hostname, context, workers=3, web_dir=".embuild/web"):
    assets = bundled_assets(web_dir)

    def check_group(group):
        with closing(BoardConnection(address, hostname, context)) as connection:
            for asset in group:
                try:
                    verify_asset(connection.get, asset)
                except Exception as error:
                    raise RuntimeError(f"Asset verification failed for {asset[0]}: {error}") from error

    # A bounded browser-like set of concurrent keep-alive connections.
    with ThreadPoolExecutor(max_workers=workers) as pool:
        list(pool.map(check_group, (assets[i::workers] for i in range(workers))))
    # Exercise backpressure and a transfer longer than the old five-second
    # total deadline, not just fast downloads into the PC's large TCP buffers.
    with closing(BoardConnection(address, hostname, context, slow=True)) as connection:
        verify_asset(connection.get, max(assets, key=lambda asset: len(asset[1])))
    return len(assets)
