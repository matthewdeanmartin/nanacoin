"""Device entry point. MicroPython runs this automatically at boot.

Joins WiFi, then serves the built NanaCoin site out of /www forever.

This board serves files and nothing else. The ledger, the API and every
decision about money live on the S3 running NanaCoin; this one has no idea any
of that exists. The browser loads the page from here and then talks to the
other board directly, which is why the API's CORS policy is what makes the
pair work - see the README.
"""

import gc
import socket
import sys
import time

import network

import static

try:
    from config import WIFI_SSID, WIFI_PASSWORD
except ImportError:
    print("!! config.py missing - copy config_example.py to config.py")
    print("!! and put your WiFi credentials in it.")
    sys.exit(1)

PORT = 80

# Advertised over mDNS, so the site is at http://nanacoin.local regardless of
# what IP DHCP hands out.
#
# The TinyGo S3 API uses nanacoin-api.local; keep these names distinct when
# both boards are on the same LAN.
HOSTNAME = "nanacoin"

CONNECT_TIMEOUT_S = 30

# Close a client that opens a connection and then says nothing, rather than
# letting it hold the single-threaded server forever. One stalled peer would
# otherwise take the whole site down for the household.
CLIENT_TIMEOUT_S = 10


def connect_wifi():
    """Join the network. Returns the IP address, or None on failure."""
    wlan = network.WLAN(network.STA_IF)
    wlan.active(True)

    # Set the hostname BEFORE connecting: the announcement goes out during
    # association, so setting it afterwards is too late to take effect.
    try:
        wlan.config(hostname=HOSTNAME)
    except (OSError, ValueError) as e:
        print("could not set hostname:", e)

    if wlan.isconnected():
        return wlan.ifconfig()[0]

    print("connecting to SSID %r ..." % WIFI_SSID)
    wlan.connect(WIFI_SSID, WIFI_PASSWORD)

    deadline = time.time() + CONNECT_TIMEOUT_S
    while not wlan.isconnected():
        if time.time() > deadline:
            print("!! could not join %r in %ds" % (WIFI_SSID, CONNECT_TIMEOUT_S))
            print("!! the S2 has no 5GHz radio - check the band, not just the password")
            return None
        time.sleep(0.5)

    return wlan.ifconfig()[0]


def parse_request(conn):
    """Read the request line and the one header that matters.

    Returns (path, accepts_gzip).

    Only Accept-Encoding is inspected. Everything else is drained rather than
    parsed: the client may not send a body, but it will send headers, and
    leaving them unread makes some browsers report a connection reset instead
    of reading the response that is already on its way.
    """
    request_line = conn.readline()
    if not request_line:
        return None, False

    parts = request_line.split()
    path = parts[1].decode() if len(parts) > 1 else "/"

    accepts_gzip = False
    while True:
        line = conn.readline()
        if not line or line == b"\r\n":
            break
        lowered = line.lower()
        if lowered.startswith(b"accept-encoding:") and b"gzip" in lowered:
            accepts_gzip = True

    return path, accepts_gzip


def serve(ip):
    addr = socket.getaddrinfo("0.0.0.0", PORT)[0][-1]
    s = socket.socket()
    # Without SO_REUSEADDR a restart fails to bind for a minute or so while
    # the old socket lingers in TIME_WAIT.
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind(addr)
    s.listen(4)

    print("")
    print("NanaCoin site at http://%s/" % ip)
    print("              or http://%s.local/" % HOSTNAME)
    print("")
    print("Point it at the API board once, and it is remembered:")
    print("  http://%s.local/?api=<nanacoin-board-ip>" % HOSTNAME)
    print("")

    while True:
        conn = None
        try:
            conn, remote = s.accept()
            conn.settimeout(CLIENT_TIMEOUT_S)

            path, accepts_gzip = parse_request(conn)
            if path is None:
                continue

            status = static.send(conn, path, accepts_gzip)
            print("%d %s" % (status, path))

        except OSError as e:
            # One bad client must never take the server down. A timeout here
            # is an ordinary event, not a fault: browsers open speculative
            # connections and abandon them.
            print("connection error:", e)

        finally:
            if conn:
                conn.close()
            # 2MB of heap, and serving files fragments it. Collecting per
            # connection costs a few milliseconds and buys uptime measured in
            # weeks rather than hours.
            gc.collect()


def main():
    ip = connect_wifi()
    if ip is None:
        return
    serve(ip)


main()
