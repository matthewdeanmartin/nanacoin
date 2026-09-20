"""Tests for the static file server, runnable on a PC.

static.py is written to MicroPython's subset but is ordinary Python, so the
path handling can be tested here rather than by flashing a board and clicking
around. The two rules worth being sure about are the SPA fallback, which is
what makes a page survive being refreshed, and the traversal check, which is
what keeps config.py and its WiFi password off the network.

    python -m pytest test_static.py -q
"""

import gzip as gziplib

import pytest

import static

# The header/body separator, named so no test has to escape it.
CRLF2 = b"\r\n\r\n"


@pytest.fixture(autouse=True)
def site(tmp_path, monkeypatch):
    """A built site on disk, shaped like Angular's output."""
    root = tmp_path / "www"
    root.mkdir()
    (root / "index.html").write_text("<!doctype html>index")
    (root / "main-ETPGPPCZ.js").write_text("console.log(1)")
    (root / "main-ETPGPPCZ.js.gz").write_bytes(gziplib.compress(b"console.log(1)"))
    (root / "styles-YLJGJUOI.css").write_text("body{}")
    (root / "favicon.ico").write_bytes(b"\x00icon")
    monkeypatch.setattr(static, "ROOT", str(root))
    # A sibling the server must never reach.
    (tmp_path / "config.py").write_text("WIFI_PASSWORD = 'hunter2'")
    return root


class FakeConn:
    def __init__(self):
        self.written = b""

    def write(self, data):
        self.written += data


# --- resolution -------------------------------------------------------------


def test_root_serves_index():
    path, gz, ctype = static.resolve("/", False)
    assert path.endswith("index.html")
    assert not gz
    assert ctype.startswith("text/html")


def test_existing_file_is_served():
    path, gz, ctype = static.resolve("/styles-YLJGJUOI.css", False)
    assert path.endswith("styles-YLJGJUOI.css")
    assert ctype.startswith("text/css")

def test_self_hosted_cursive_font(site):
    (site / "hand.ttf").write_bytes(b"font fixture")
    path, gz, ctype = static.resolve("/hand.ttf", False)
    assert path.endswith("hand.ttf")
    assert not gz
    assert ctype == "font/ttf"


def test_gzip_is_preferred_when_accepted():
    path, gz, ctype = static.resolve("/main-ETPGPPCZ.js", True)
    assert path.endswith(".gz")
    assert gz is True
    # The type is the *file's*, not gzip's - the encoding is a separate header.
    assert ctype.startswith("text/javascript")


def test_plain_copy_is_used_when_gzip_is_refused():
    path, gz, _ = static.resolve("/main-ETPGPPCZ.js", False)
    assert not path.endswith(".gz")
    assert gz is False


def test_unknown_route_falls_back_to_index():
    # This is what makes refreshing on /economy work. Angular owns that route;
    # the board has never heard of it.
    path, _, ctype = static.resolve("/economy", False)
    assert path.endswith("index.html")
    assert ctype.startswith("text/html")


def test_query_string_is_ignored():
    # ?api=... is for the client, not the file system.
    path, _, _ = static.resolve("/?api=192.168.1.158", False)
    assert path.endswith("index.html")


def test_nested_route_falls_back_to_index():
    path, _, _ = static.resolve("/some/deep/route", False)
    assert path.endswith("index.html")


# --- traversal --------------------------------------------------------------


@pytest.mark.parametrize(
    "attack",
    [
        "/../config.py",
        "/../../config.py",
        "/assets/../../config.py",
        "/..",
    ],
)
def test_traversal_is_refused(attack):
    # The board's filesystem holds the WiFi password. None of these may reach
    # it - and none may quietly fall back to index.html either, which would
    # hide the attempt.
    assert static.resolve(attack, False) is None


def test_path_without_leading_slash_is_refused():
    assert static.resolve("config.py", False) is None


# --- types and caching ------------------------------------------------------


def test_content_types():
    assert static.content_type("/a.html").startswith("text/html")
    assert static.content_type("/a.js").startswith("text/javascript")
    assert static.content_type("/a.css").startswith("text/css")
    assert static.content_type("/a.woff2") == "font/woff2"
    assert static.content_type("/a.unknown") == "application/octet-stream"
    assert static.content_type("/noextension") == "application/octet-stream"


def test_hashed_files_are_immutable():
    assert static.is_hashed("/www/main-ETPGPPCZ.js")
    assert static.is_hashed("/www/styles-YLJGJUOI.css")
    # A gzipped asset must still be recognised, or most of the build silently
    # loses its cache headers.
    assert static.is_hashed("/www/main-ETPGPPCZ.js.gz")


def test_index_is_never_immutable():
    # index.html names the hashed files. Cache it and a browser asks for a
    # build that no longer exists.
    assert not static.is_hashed("/www/index.html")
    assert not static.is_hashed("/www/index.html.gz")


def test_unhashed_names_are_not_immutable():
    assert not static.is_hashed("/www/favicon.ico")
    assert not static.is_hashed("/www/main.js")
    assert not static.is_hashed("/www/a-b.js")  # too short to be a hash


# --- sending ----------------------------------------------------------------


def test_send_writes_headers_then_body():
    conn = FakeConn()
    status = static.send(conn, "/styles-YLJGJUOI.css", False)
    assert status == 200
    head, _, body = conn.written.partition(CRLF2)
    assert b"200 OK" in head
    assert b"text/css" in head
    assert b"Content-Length: 6" in head
    assert b"Content-Encoding" not in head
    assert body == b"body{}"


def test_send_marks_gzip_and_keeps_the_files_type():
    conn = FakeConn()
    static.send(conn, "/main-ETPGPPCZ.js", True)
    head = conn.written.partition(CRLF2)[0]
    assert b"Content-Encoding: gzip" in head
    assert b"text/javascript" in head
    # Hashed, so it may be cached forever.
    assert b"immutable" in head


def test_send_tells_browsers_not_to_cache_index():
    conn = FakeConn()
    static.send(conn, "/", False)
    head = conn.written.partition(CRLF2)[0]
    assert b"no-cache" in head
    assert b"immutable" not in head


def test_send_404s_a_refused_path_rather_than_falling_back():
    conn = FakeConn()
    status = static.send(conn, "/../config.py", False)
    assert status == 404
    assert b"hunter2" not in conn.written


def test_send_404s_when_there_is_no_site(tmp_path, monkeypatch):
    monkeypatch.setattr(static, "ROOT", str(tmp_path / "empty"))
    conn = FakeConn()
    assert static.send(conn, "/", False) == 404


# --- the gzip-only case -----------------------------------------------------
#
# deploy ships whichever copy of a file is smaller, which for every real asset
# is the .gz - so most files exist ONLY compressed. Browsers all send
# Accept-Encoding: gzip and get those bytes back with a Content-Encoding
# header, which is ordinary HTTP negotiation and not the browser opening a
# .gz file. But curl without the header, a proxy that strips it, and
# hand-written clients do not send it, and they must still be served rather
# than 404'd - which is what happened before this path existed.


def test_gz_only_file_is_inflated_for_a_client_that_refuses_gzip(tmp_path, monkeypatch):
    root = tmp_path / "gzonly"
    root.mkdir()
    (root / "index.html.gz").write_bytes(gziplib.compress(b"<!doctype html>hello"))
    monkeypatch.setattr(static, "ROOT", str(root))

    path, mode, _ = static.resolve("/index.html", False)
    assert mode == static.GZIP_INFLATE
    assert path.endswith(".gz")

    conn = FakeConn()
    assert static.send(conn, "/", False) == 200
    head, _, body = conn.written.partition(CRLF2)
    # Plain bytes on the wire, so it must NOT claim to be gzipped.
    assert b"Content-Encoding" not in head
    assert body == b"<!doctype html>hello"
    assert b"Content-Length: 20" in head


def test_gz_only_file_is_passed_through_untouched_when_accepted(tmp_path, monkeypatch):
    root = tmp_path / "gzonly2"
    root.mkdir()
    raw = gziplib.compress(b"<!doctype html>hello")
    (root / "index.html.gz").write_bytes(raw)
    monkeypatch.setattr(static, "ROOT", str(root))

    conn = FakeConn()
    static.send(conn, "/", True)
    head, _, body = conn.written.partition(CRLF2)
    assert b"Content-Encoding: gzip" in head
    # Untouched: the compressed bytes exactly as they sit on flash.
    assert body == raw


def test_gz_only_route_falls_back_to_an_inflated_index(tmp_path, monkeypatch):
    # A refresh on /economy, from a client that will not take gzip, on a board
    # where only index.html.gz exists. All three at once is the case that was
    # returning 404.
    root = tmp_path / "gzonly3"
    root.mkdir()
    (root / "index.html.gz").write_bytes(gziplib.compress(b"<!doctype html>spa"))
    monkeypatch.setattr(static, "ROOT", str(root))

    conn = FakeConn()
    assert static.send(conn, "/economy", False) == 200
    head, _, body = conn.written.partition(CRLF2)
    assert body == b"<!doctype html>spa"
    assert b"Content-Encoding" not in head


# --- API paths are not this board's --------------------------------------
#
# The site and the ledger live on different boards. A browser opening this
# site at a fresh origin has nothing remembered in localStorage, so the client
# falls back to same-origin and asks THIS board for /api/v1/status. Answering
# with index.html and a 200 makes that fail as "malformed JSON" with nothing
# in the console, because nothing actually went wrong at the HTTP level.


@pytest.mark.parametrize(
    "api_path",
    [
        "/api/v1/status",
        "/api/v1/me",
        "/api/v1/transactions?limit=50",
    ],
)
def test_api_paths_404_rather_than_returning_the_page(api_path):
    assert static.resolve(api_path, False) is None

    conn = FakeConn()
    assert static.send(conn, api_path, False) == 404
    assert b"<!doctype" not in conn.written.lower()


def test_a_route_merely_starting_with_api_still_reaches_the_app():
    # /apiary is a page, not an API call. The check is on the /api/ segment.
    path, _, _ = static.resolve("/apiary", False)
    assert path.endswith("index.html")
