"""Run bundled NanaCoin and Minicloud on loopback with separate test data.

Build NanaCoin with make bundle and Minicloud with make web and cargo build.
MINICLOUD_EXE selects another desktop build. Ctrl+C stops only our children.
"""
import importlib
import os
from pathlib import Path
import socket
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
relay = importlib.import_module("minicloud-smoke")


def main():
    suffix = ".exe" if os.name == "nt" else ""
    bank = ROOT / "target/debug" / ("nanacoin" + suffix)
    cloud = Path(os.environ.get("MINICLOUD_EXE", str(
        ROOT.parents[1] / "mastomini/minicloud_rs/target/debug" / ("minicloud" + suffix))))
    for binary in (bank, cloud):
        if not binary.is_file():
            raise SystemExit(f"Build the desktop executable first: {binary}")
    for port in (8086, 8096, 1886):
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", port))
    data = ROOT / ".local/kitchen-preview"
    data.mkdir(parents=True, exist_ok=True)
    children = []
    with (data / "process.log").open("a", encoding="utf-8") as log:
        def start(binary, settings):
            process = subprocess.Popen([str(binary.resolve())],
                env=dict(os.environ, **settings), stdout=log, stderr=log,
                creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
            children.append(process)
            return process

        def ready(base, path):
            def check():
                if any(child.poll() is not None for child in children):
                    raise RuntimeError(f"A preview server stopped; see {data / 'process.log'}")
                return relay.http(base, path)[0] == 200
            relay.wait(check)

        try:
            start(cloud, dict(MINICLOUD_BIND="127.0.0.1", MINICLOUD_PORT="8096",
                MINICLOUD_MQTT_PORT="1886", MINICLOUD_DATA=str(data / "cloud"),
                MINICLOUD_ADMIN_TOKEN="kitchen-preview-admin-token"))
            ready("http://127.0.0.1:8096", "/api/status")
            start(bank, dict(NANACOIN_PORT="8086", NANACOIN_JOURNAL=str(data / "bank"),
                NANACOIN_MINICLOUD_URL="http://127.0.0.1:8096"))
            ready("http://127.0.0.1:8086", "/api/v1/status")
            print("NanaCoin: http://127.0.0.1:8086/")
            print("Minicloud: http://127.0.0.1:8096/")
            print("Minicloud management token: kitchen-preview-admin-token")
            print("First NanaCoin visit: provision a household, then add two test accounts.")
            print(f"Separate persistent PC data: {data}")
            print("Ctrl+C stops these two preview servers. No physical board is contacted.", flush=True)
            while all(child.poll() is None for child in children):
                time.sleep(1)
        except KeyboardInterrupt:
            pass
        finally:
            for child in reversed(children):
                if child.poll() is None:
                    child.terminate()
                child.wait(timeout=10)


if __name__ == "__main__":
    main()
