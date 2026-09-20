"""Write the API board's address into a built index.html.

    python stamp_api.py <index.html> <address>

This board serves files and has no API of its own. Without an address baked
in, the client falls back to its own origin, asks this board for
/api/v1/status, and gets index.html back from the single-page fallback - which
fails as a JSON parse error rather than as anything a person can act on.

index.html carries `<meta name="nanacoin-api">` for exactly this and ships
empty. Run against the staged copy, so the Angular build output is never
modified.
"""

import io
import re
import sys


def main(argv):
    if len(argv) != 3:
        sys.stderr.write("usage: stamp_api.py <index.html> <address>\n")
        return 2

    path, api = argv[1], argv[2]

    # Accept what someone would actually type: an address, or a full URL.
    if not api.startswith("http://") and not api.startswith("https://"):
        api = "http://" + api
    api = api.rstrip("/")

    html = io.open(path, encoding="utf-8").read()
    stamped, count = re.subn(
        r'<meta name="nanacoin-api"[^>]*>',
        '<meta name="nanacoin-api" content="%s">' % api,
        html,
    )
    if count == 0:
        sys.stderr.write("no <meta name=\"nanacoin-api\"> tag in %s\n" % path)
        return 1

    io.open(path, "w", encoding="utf-8").write(stamped)
    print("  API baked in: %s" % api)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
