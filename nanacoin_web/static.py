"""Serving a built single-page application off the board's flash.

This is the half of NanaCoin that is not the ledger: an ESP32-S2 whose whole
job is to hand the Angular bundle to a browser. The API lives on a different
board entirely - the S3 - and the two never talk to each other. The browser
talks to both.

# Why files are streamed rather than returned

The rest of this project's handlers build a response body in memory and hand
back a string. That is right for a few hundred bytes of JSON and wrong for a
200KB JavaScript file: this board has about 2MB of usable heap, and reading a
whole bundle into it to copy it out again spends twice the file's size for no
reason, on the one device in the house with the least memory to spare.

So `send_file` writes the header, then copies the file to the socket a chunk at
a time. Peak memory is one chunk, whatever the file's size.

# Why gzip matters here more than it usually does

4MB of flash, most of it already spent on the MicroPython firmware. A
pre-compressed copy of each asset is both smaller on flash and faster to serve,
because the board never compresses anything at runtime - it just picks the
`.gz` file when the browser says it accepts it. See `deploy.ps1`, which does
the compressing on the PC where the CPU is free.
"""

try:
    import uos as os
except ImportError:
    import os

# Where the built site lives on the board's filesystem.
ROOT = "/www"

# How much of a file to hold at once. 1KB is a compromise found by arithmetic
# rather than taste: large enough that a 200KB file is 200 writes rather than
# 200,000, small enough that several concurrent requests cannot add up to
# anything this board would notice.
CHUNK = 1024

# Content types by extension. Deliberately a small fixed table: a browser that
# gets `application/octet-stream` for a stylesheet ignores the stylesheet, and
# the failure looks like a CSS bug rather than a server one.
TYPES = {
    "html": "text/html; charset=utf-8",
    "js": "text/javascript; charset=utf-8",
    "css": "text/css; charset=utf-8",
    "json": "application/json",
    "svg": "image/svg+xml",
    "ico": "image/x-icon",
    "png": "image/png",
    "jpg": "image/jpeg",
    "webp": "image/webp",
    "woff2": "font/woff2",
    "ttf": "font/ttf",
    "txt": "text/plain; charset=utf-8",
    "map": "application/json",
}

DEFAULT_TYPE = "application/octet-stream"

# A third state for "is this gzipped", beyond True and False: the file on flash
# is compressed but the client will not accept gzip, so it must be inflated on
# the way out and served without a Content-Encoding header.
GZIP_INFLATE = 2


def content_type(path):
    """The MIME type for a path, by extension."""
    dot = path.rfind(".")
    if dot < 0:
        return DEFAULT_TYPE
    return TYPES.get(path[dot + 1 :].lower(), DEFAULT_TYPE)


def _exists(path):
    """Whether a path exists and is a regular file.

    os.stat raises rather than returning None on MicroPython, and the mode bit
    matters: a directory called `index.html` would otherwise be served as an
    empty file.
    """
    try:
        mode = os.stat(path)[0]
    except OSError:
        return False
    # 0x8000 is S_IFREG. Spelled numerically because MicroPython's `stat`
    # module is not always built in.
    return mode & 0x8000 != 0


def _size(path):
    return os.stat(path)[6]


def resolve(url_path, accepts_gzip):
    """Map a URL to a file on flash.

    Returns (filesystem path, is_gzipped, content type), or None when nothing
    sensible can be served.

    The rules, in order:

      1. `/` means index.html.
      2. A file that exists is served.
      3. Anything else falls back to index.html, because Angular routes like
         /market and /economy exist only in the browser. Without this, opening
         the app at a route or pressing refresh gives a 404 from a server that
         has never heard of that path - which is the single most common way a
         deployed SPA appears broken.

    A `.gz` sibling wins whenever the browser accepts it.
    """
    if not url_path or url_path == "/":
        url_path = "/index.html"

    # Strip a query string: the board serves files, and ?api=... is for the
    # client. Also strip a fragment, which browsers do not send but proxies
    # and hand-written requests sometimes do.
    for sep in ("?", "#"):
        cut = url_path.find(sep)
        if cut >= 0:
            url_path = url_path[:cut]

    if not _safe(url_path):
        return None

    candidate = ROOT + url_path
    ctype = content_type(url_path)

    if accepts_gzip and _exists(candidate + ".gz"):
        return candidate + ".gz", True, ctype
    if _exists(candidate):
        return candidate, False, ctype
    # Only the compressed copy is on flash - deploy ships whichever of the two
    # is smaller, which for anything but a tiny file is the .gz. A client that
    # will not take gzip still has to be served something, so it is inflated on
    # the way out. Marked GZIP_INFLATE rather than True: the bytes on the wire
    # are plain, so no Content-Encoding header may be sent.
    if _exists(candidate + ".gz"):
        return candidate + ".gz", GZIP_INFLATE, ctype

    # An API path is never a route this board can answer. It serves files; the
    # ledger lives on the other board entirely.
    #
    # Without this the SPA fallback returns index.html with a 200, and a client
    # that has not been told where the API is - a browser opening this site at
    # a fresh origin, with nothing remembered in localStorage - asks *here* for
    # /api/v1/status, gets HTML, and fails with "malformed JSON" and nothing in
    # the console, because from HTTP's point of view nothing went wrong. A 404
    # is both true and diagnosable.
    if url_path.startswith("/api/"):
        return None

    # SPA fallback. A request for a missing *asset* falls through to here too
    # and gets index.html, which is wrong but harmless: the browser asked for a
    # file this build does not contain, and either answer is a broken page. The
    # alternative - guessing which paths are routes and which are assets - is
    # more code and more ways to be wrong.
    index = ROOT + "/index.html"
    if accepts_gzip and _exists(index + ".gz"):
        return index + ".gz", True, TYPES["html"]
    if _exists(index):
        return index, False, TYPES["html"]
    if _exists(index + ".gz"):
        return index + ".gz", GZIP_INFLATE, TYPES["html"]

    return None


def _safe(url_path):
    """Reject any path that tries to climb out of ROOT.

    The board's filesystem holds config.py, which holds the WiFi password. A
    request for /../config.py must not reach it. Checking for the segment
    rather than the substring means a legitimate file with two dots in its
    name is still servable.
    """
    if not url_path.startswith("/"):
        return False
    for segment in url_path.split("/"):
        if segment == "..":
            return False
    return True


def headers(status, ctype, length, gzipped, cacheable):
    """The response head.

    HTTP/1.0 with Connection: close, matching the rest of this project - the
    server is single-threaded and closes after every response, so there is no
    keep-alive to negotiate.

    Cache-Control is where most of the performance is. Angular emits hashed
    filenames (main-ETPGPPCZ.js), so those bytes can never change under that
    name and are immutable for a year; index.html names them and must never be
    cached, or a browser holding an old copy asks for files the new build no
    longer has.
    """
    reason = {200: "OK", 404: "Not Found", 500: "Internal Server Error"}.get(status, "OK")
    head = [
        "HTTP/1.0 %d %s" % (status, reason),
        "Content-Type: %s" % ctype,
        "Content-Length: %d" % length,
        "Connection: close",
    ]
    # Only a file passed through untouched is declared gzip. An inflated one
    # goes out as plain bytes and must not claim otherwise.
    if gzipped is True:
        head.append("Content-Encoding: gzip")
    if cacheable:
        head.append("Cache-Control: public, max-age=31536000, immutable")
    else:
        head.append("Cache-Control: no-cache")
    return ("\r\n".join(head) + "\r\n\r\n").encode("utf-8")


def is_hashed(path):
    """Whether a filename carries a content hash, and is therefore immutable.

    Angular's pattern is name-HASH.ext, with an 8+ character base-36-ish hash.
    Getting this wrong is safe in one direction only: a file wrongly treated as
    immutable is cached for a year and cannot be corrected, so the test is
    deliberately conservative and index.html is excluded outright.
    """
    slash = path.rfind("/")
    name = path[slash + 1 :] if slash >= 0 else path

    # A gzipped asset arrives here as main-ETPGPPCZ.js.gz. Without stripping
    # the suffix the hash test reads "ETPGPPCZ.js" as the token, fails on the
    # dot, and every compressed file - which is most of the build - silently
    # loses its cache headers.
    if name.endswith(".gz"):
        name = name[:-3]

    if name.startswith("index."):
        return False
    dash = name.rfind("-")
    dot = name.rfind(".")
    if dash < 0 or dot < dash:
        return False
    token = name[dash + 1 : dot]
    if len(token) < 8:
        return False
    for ch in token:
        if not (ch.isdigit() or ("a" <= ch <= "z") or ("A" <= ch <= "Z") or ch == "_"):
            return False
    return True


def send(conn, url_path, accepts_gzip):
    """Serve one request. Returns the status written, for logging.

    The socket is written directly rather than through a returned body, which
    is what keeps a large file from ever being resident in full.
    """
    found = resolve(url_path, accepts_gzip)
    if found is None:
        body = b"not found"
        conn.write(headers(404, TYPES["txt"], len(body), False, False))
        conn.write(body)
        return 404

    path, gzipped, ctype = found

    if gzipped is GZIP_INFLATE:
        return _send_inflated(conn, path, ctype)

    try:
        length = _size(path)
    except OSError:
        body = b"not found"
        conn.write(headers(404, TYPES["txt"], len(body), False, False))
        conn.write(body)
        return 404

    conn.write(headers(200, ctype, length, gzipped, is_hashed(path)))

    # One chunk resident at a time, whatever the file's size.
    with open(path, "rb") as f:
        while True:
            chunk = f.read(CHUNK)
            if not chunk:
                break
            conn.write(chunk)
    return 200


def _inflate(path):
    """Read a .gz file and return its uncompressed bytes.

    MicroPython has `deflate`; CPython has `gzip`. Both are tried so that the
    test suite exercises this path on a PC rather than only discovering it on
    the board - which is the whole reason static.py avoids MicroPython-only
    spellings elsewhere.
    """
    try:
        import deflate  # MicroPython

        with open(path, "rb") as f:
            with deflate.DeflateIO(f, deflate.GZIP) as d:
                return d.read()
    except ImportError:
        pass

    import gzip  # CPython

    with gzip.open(path, "rb") as f:
        return f.read()


def _send_inflated(conn, path, ctype):
    """Serve a .gz file as plain bytes, for a client that will not take gzip.

    Every browser sends `Accept-Encoding: gzip`, so this path is for curl
    without the header, a proxy that strips it, and anything hand-written. It
    is the uncommon case, which is what makes the cost acceptable: the
    inflated size is not known without inflating, and HTTP/1.0 has no chunked
    encoding to stream an unknown length, so the whole file is decompressed
    into memory to measure it.

    Bounded by the fact that the largest asset here is a few hundred KB
    against ~2MB of heap. A build large enough to make that uncomfortable
    would fail the deploy script's size check first.
    """
    try:
        body = _inflate(path)
    except ImportError:
        # No decompressor at all. Refusing is honest; sending compressed bytes
        # without the header would be silent corruption.
        body = b"gzip required"
        conn.write(headers(406, TYPES["txt"], len(body), False, False))
        conn.write(body)
        return 406
    except (OSError, ValueError):
        body = b"not found"
        conn.write(headers(404, TYPES["txt"], len(body), False, False))
        conn.write(body)
        return 404

    conn.write(headers(200, ctype, len(body), False, is_hashed(path)))
    # Written in chunks rather than one call: a large single write can fail on
    # a socket whose send buffer is smaller than the body.
    for i in range(0, len(body), CHUNK):
        conn.write(body[i : i + CHUNK])
    return 200
