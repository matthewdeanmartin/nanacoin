"""Boot the complete server in a local emulator and exercise public banking routes.

Requires build_firmware.py --emulator-network. No board, real Wi-Fi credentials,
flash persistence, or external network access is used. The bounded workload
checks banking and TLS, but does not certify every capacity or power-cut recovery.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
from http.client import HTTPException
import importlib.util
import json
import os
import re
from pathlib import Path
import subprocess
import ssl
import sys
import time
import urllib.request

from probe_esp_emu import linux_path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "nanacoin_rs/scripts"))
from boards import board  # noqa: E402


def exercise_families(client, report, unix_seconds=1799996400):
    """Representative public writes/reads through every banking family."""
    listing = client.listing(client.bob)
    client.call("/listings/" + listing["id"] + "/purchase", {}, client.alice, "family-buy", expected=201)
    loan = client.call("/loans", dict(borrower=client.accounts["bob"], amount=10,
        rate_bps=0, rate_days=365, payment_days=30, installment=10, credit=False,
        memo="Emulator loan"), client.alice, "family-loan")
    loan_path = "/loans/" + str(loan["id"])
    client.assertEqual(client.call(loan_path + "/accept", {}, client.bob, "family-accept")["status"], "ACTIVE")
    client.assertEqual(client.call(loan_path + "/repay", {"amount": 10}, client.bob, "family-repay")["status"], "PAID")
    client.issue("alice", 10000, "family-fx-funds")
    client.call("/admin/issue-usd", dict(to=client.accounts["bob"], cents=500,
        reason="Emulator cash"), client.nana, "family-usd", expected=201)
    quote = client.call("/quotes", dict(side="ASK", cents_per_coin=250, coins=10000), client.alice, expected=201)
    client.call("/quotes/" + quote["id"] + "/take", {}, client.bob, "family-trade", expected=201)
    gift = client.commerce({"create_request": dict(title="Emulator gift", description="Thanks",
        target=20, deadline=None)}, client.bob, "family-gift-request")["sequence"]
    client.commerce({"contribute": dict(request=gift, amount=10, memo="Gift")}, client.alice, "family-gift")
    client.assertEqual(client.request_view(gift)["received"], 10)
    art = client.commerce({"mint_art": dict(title="Emulator art", license="Display",
        sha256="ab" * 32, locator="https://example.invalid/art.png")}, client.bob, "family-mint")["sequence"]
    client.commerce({"list_art": dict(art=art, price=10)}, client.bob, "family-art-list")
    view = client.artwork(art)
    client.commerce({"buy_art": dict(art=art, expected_owner=view["owner"],
        expected_revision=view["revision"], expected_price=10)}, client.alice, "family-art-buy")
    client.assertEqual(client.artwork(art)["owner"], int(client.accounts["alice"].removeprefix("account-")))
    lotto = client.call("/lottos", dict(kind="SIMPLE", title="Emulator draw", ticket_price=10,
        closes_at=unix_seconds + 3600, rate_bps=0), client.nana, "family-lotto")
    bought = client.call("/lottos/" + str(lotto["id"]) + "/tickets", {"count": 1}, client.alice, "family-ticket")
    client.assertEqual(bought["pool"], 10)
    meta = client.get("/loans", client.nana)
    before = client.balance("alice")
    reform = dict(decimals=5, power=0, expected_epoch=meta["money_epoch"],
                  expected_sequence=meta["sequence"], preview=True)
    client.assertTrue(client.call("/admin/reform", reform, client.nana)["preview"])
    client.call("/admin/reform", dict(reform, preview=False), client.nana, "family-reform")
    client.assertEqual(client.balance("alice"), before * 10)
    client.assertTrue(client.get("/status")["ledger_balanced"])
    report["business_families_passed"] = ["posting", "identity", "marketplace", "forex", "commerce", "lending", "lotto", "reform"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-report", type=Path, required=True)
    parser.add_argument("--emulator", type=Path, required=True)
    parser.add_argument("--port", type=int, default=18080)
    parser.add_argument("--distribution", default="Ubuntu")
    parser.add_argument("--workload", action="store_true", help="Exercise public identity/posting/atomic retry routes")
    parser.add_argument("--observe-seconds", type=int, default=0, help="Additional wall seconds to collect post-workload health samples (0-90)")
    parser.add_argument("--history", action="store_true", help="Also cross the advertised retained-transaction capacity with balanced payments")
    parser.add_argument("--capacity", action="store_true", help="Fill all bounded business books simultaneously, then reform the bank")
    parser.add_argument("--families", action="store_true", help="Also exercise marketplace, forex, commerce, lending, lotto and reform over HTTP")
    parser.add_argument("--https", action="store_true", help="Use TLS with the board's CA through loopback forwarding")
    parser.add_argument("--tls-port", type=int, default=18443)
    parser.add_argument("--hosted-companion-report", type=Path,
                        help="Matching C6 slave report from build_hosted_emulator.py for P4")
    args = parser.parse_args()
    if not 0 <= args.observe_seconds <= 90 or (args.observe_seconds and not args.workload):
        parser.error("--observe-seconds requires --workload and a value from 0 to 90")
    if args.history and not args.capacity:
        parser.error("--history requires --capacity")
    if args.capacity and not args.families:
        parser.error("--capacity requires --families --workload")
    if args.families and not args.workload:
        parser.error("--families requires --workload")
    build = json.loads(args.build_report.read_text(encoding="utf-8"))
    if not build.get("synthetic_emulator_network") or not build.get("app_partition_fits"):
        parser.error("Requires a fitting, synthetic-network server image")
    if build["board"] not in ("s3", "p4"):
        parser.error("Server emulation supports S3 or P4")
    if build["board"] == "p4" and (args.hosted_companion_report is None or build.get("p4_emulator_revision") != "3.1"):
        parser.error("P4 requires --p4-emulator-rev3 firmware and --hosted-companion-report")
    if args.families and build.get("emulator_clock_initial_unix_seconds") is None:
        parser.error("--families requires firmware built with --emulator-clock for restricted-network time-dependent banking")
    profile = board(build["board"])
    release = Path(build["target_dir"]) / profile.target / "release"
    elf = release / "nanacoin-esp32"
    image = elf.with_suffix(".bin")
    for path, key in [(elf, "elf_sha256"), (image, "image_sha256")]:
        if hashlib.sha256(path.read_bytes()).hexdigest() != build[key]:
            parser.error("Server artifact changed: " + str(path))
    out = args.build_report.resolve().parent / "emulator"
    out.mkdir(parents=True, exist_ok=True)
    flash = out / "flash.bin"
    python = Path("C:/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe")
    with (out / "merge.log").open("w", encoding="utf-8") as log:
        subprocess.run([str(python), "-m", "esptool", "--chip=" + profile.chip, "merge_bin",
                        "--output=" + str(flash), "--fill-flash-size=" + profile.flash_size,
                        hex(profile.bootloader_offset), str(release / "bootloader.bin"),
                        "0x8000", str(release / "partition-table.bin"), "0x10000", str(image)],
                       stdout=log, stderr=subprocess.STDOUT, check=True)
    report = {"hardware_access": False, "deployment_supported": False,
              "board": profile.id, "p4_emulator_revision": build.get("p4_emulator_revision"),
              "complete_server_startup_verified": False, "workload_budget_verified": False,
              "banking_workload_passed": False, "https_exercised": args.https,
              "emulator_clock_initial_unix_seconds": build.get("emulator_clock_initial_unix_seconds"),
              "elf_sha256": build["elf_sha256"], "image_sha256": build["image_sha256"],
              "emulator_sha256": hashlib.sha256(args.emulator.read_bytes()).hexdigest()}
    started_at = time.monotonic()
    http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    scheme, port = "http", args.port
    if args.https:
        context = ssl.create_default_context(cafile=str(profile.root / profile.ca))
        # Loopback forwarding uses an IP instead of the bank's DNS name.
        # Keep issuer/signature checks against this board's actual CA.
        context.check_hostname = False
        http.add_handler(urllib.request.HTTPSHandler(context=context))
        scheme, port = "https", args.tls_port
    duration = 7200 if args.history else 3600 if args.capacity else 900 if args.workload else 180 if args.https else 60
    launch = ["wsl", "-d", args.distribution, "--", linux_path(args.emulator)]
    net = f"user,restrict=yes,hostfwd=tcp::{args.port}-:80,hostfwd=tcp::{args.tls_port}-:443"
    command = launch + ["--chip", profile.chip, "--firmware", linux_path(flash), "--timeout", f"{duration}s",
               "--wifi-ssid", "nc-emulator-ssid", "--wifi-password", "nc-emulator-password",
               "--psram-size", "8M" if profile.id == "s3" else "32M", "--net", net]
    owned_processes = []

    def start_emulator(command, log, label):
        # Terminating wsl.exe does not terminate its Linux child. Record the
        # exact exec PID and stop that owned process before closing the wrapper.
        pid_file = out / (label + ".pid")
        pid_file.unlink(missing_ok=True)
        executable = linux_path(args.emulator)
        launch_code = ("import os; from pathlib import Path; "
                       + "Path(" + repr(linux_path(pid_file)) + ").write_text(str(os.getpid())); "
                       + "os.execv(" + repr(executable) + ", " + repr(command[4:]) + ")")
        process = subprocess.Popen(["wsl", "-d", args.distribution, "--", "python3", "-c", launch_code],
                                   stdout=log, stderr=subprocess.STDOUT)
        owned_processes.append((process, pid_file, executable))
        return process

    def stop_emulator(process):
        for owner, pid_file, executable in owned_processes:
            if owner is not process or not pid_file.exists():
                continue
            stop_code = ("import os, signal; from pathlib import Path; "
                         + "pid=int(Path(" + repr(linux_path(pid_file)) + ").read_text()); "
                         + "exe=Path('/proc')/str(pid)/'exe'; "
                         + "os.kill(pid,signal.SIGTERM) if exe.exists() and "
                         + "str(exe.resolve()) == " + repr(executable) + " else None")
            subprocess.run(["wsl", "-d", args.distribution, "--", "python3", "-c", stop_code],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10, check=True)
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)

    companion = None
    companion_log = None
    if profile.id == "p4":
        slave = json.loads(args.hosted_companion_report.read_text(encoding="utf-8"))
        if slave["exit_code"] != 0:
            parser.error("Build the C6 companion before emulation")
        slave_build = Path(slave["build_dir"])
        for name, expected in slave["image_sha256"].items():
            if hashlib.sha256((slave_build / name).read_bytes()).hexdigest() != expected:
                parser.error("C6 companion image changed: " + name)
        slave_flash = out / "companion-flash.bin"
        with (out / "companion-merge.log").open("w", encoding="utf-8") as log:
            subprocess.run([str(python), "-m", "esptool", "--chip=esp32c6", "merge_bin",
                            "--output=" + str(slave_flash), "--fill-flash-size=4MB", "@flash_args"],
                           cwd=slave_build, stdout=log, stderr=subprocess.STDOUT, check=True)
        report["companion_flash_sha256"] = hashlib.sha256(slave_flash.read_bytes()).hexdigest()
        bridge = f"/tmp/nc-p4-{os.getpid()}.sock"
        # Hosted Wi-Fi interception needs the companion ELF. Its NAT backend
        # forwards the P4 station's raw Ethernet packets over the SDIO bridge.
        companion_log = (out / "companion-uart.log").open("w", encoding="utf-8")
        companion = start_emulator(launch + ["--chip", "esp32c6", "--firmware", linux_path(slave_flash),
            "--elf", linux_path(slave_build / "network_adapter.elf"), "--hosted", "bridge:slave:" + bridge,
            "--timeout", f"{duration + 10}s", "--wifi-ssid", "nc-emulator-ssid",
            "--wifi-password", "nc-emulator-password", "--net", net],
            companion_log, "companion")
        # Start the slave listener before the P4 host, per ESP-EMU's guide.
        time.sleep(2)
        command = launch + ["--chip", profile.chip, "--firmware", linux_path(flash),
            "--elf", linux_path(elf), "--timeout", f"{duration}s", "--psram-size", "32M",
            "--hosted", "bridge:host:" + bridge, "--net", "user,restrict=yes"]
    with (out / "uart.log").open("w", encoding="utf-8") as log:
        process = start_emulator(command, log, "server")
        try:
            deadline = time.monotonic() + duration
            while time.monotonic() < deadline and process.poll() is None:
                try:
                    for name in ("status", "diag", "diag/static"):
                        with http.open(f"{scheme}://127.0.0.1:{port}/api/v1/{name}", timeout=15 if args.https else 2) as response:
                            report[name] = json.load(response)
                    report["complete_server_startup_verified"] = True
                    break
                except (OSError, ValueError) as error:
                    report["last_http_error"] = str(error)
                    time.sleep(.5)
            if args.workload and report["complete_server_startup_verified"]:
                # Reuse the black-box suite's JSON client and assertions only;
                # never start a desktop server or inspect Rust/COBOL internals.
                spec = importlib.util.spec_from_file_location("nc_contract", ROOT / "conformance/run.py")
                contracts = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(contracts)
                # Emulated CPUs serialize several expensive TLS handshakes. Give
                # each request enough wall time without changing desktop tests.
                class EmulatorHTTP:
                    def open(self, request, timeout=10):
                        return http.open(request, timeout=180)

                contracts.HTTP = EmulatorHTTP()
                client = contracts.BankContract()
                client.base = f"{scheme}://127.0.0.1:{port}/api/v1"
                report["workload_stage"] = "provision"
                client.call("/provision", dict(household_name="Emulator contract", username="nana",
                            display_name="Nana", password="1234"), expected=201)
                report["workload_stage"] = "identity and PKCE"
                client.nana = client.login("nana", "1234")
                for name in ("alice", "bob"):
                    client.call("/users", dict(username=name, display_name=name.title(),
                                password="5678", grant=False), client.nana, expected=201)
                client.alice = client.login("alice", "5678")
                client.bob = client.login("bob", "5678")
                client.accounts = {u["username"]: u["account"] for u in client.get("/users", client.nana)["users"]}
                report["workload_stage"] = "issuance and payments"
                client.issue("alice", 100)
                client.issue("bob", 100, "seed-bob")
                for index in range(4):
                    client.transfer(client.alice, "bob", 1, f"forward-{index}")
                    client.transfer(client.bob, "alice", 1, f"return-{index}")
                with ThreadPoolExecutor(max_workers=4) as pool:
                    report["workload_stage"] = "concurrent idempotent retries"
                    report["transport_retries"] = 0
                    def retry_transfer(index):
                        for attempt in range(8):
                            try:
                                return client.transfer(client.alice, "bob", 1, "shared-key")
                            except (OSError, HTTPException):
                                report["transport_retries"] += 1
                                if attempt == 7:
                                    raise
                                time.sleep((index + 1) * .25 * (attempt + 1))
                    receipts = list(pool.map(retry_transfer, range(4)))
                client.assertTrue(all(receipt == receipts[0] for receipt in receipts))
                client.transfer(client.alice, "bob", 1000, "rejection", expected=409)
                client.assertEqual((client.balance("alice"), client.balance("bob")), (99, 101))
                client.assertTrue(client.get("/status")["ledger_balanced"])
                report["posting_workload_passed"] = True
                if args.families:
                    # The TLS bank above is the same state behind the HTTP
                    # listener. Broaden business coverage without paying for
                    # a new emulated ECDHE handshake for every assertion.
                    client.base = f"http://127.0.0.1:{args.port}/api/v1"
                    report["workload_stage"] = "additional business families"
                    exercise_families(client, report, build["emulator_clock_initial_unix_seconds"])
                if args.capacity:
                    capacity_spec = importlib.util.spec_from_file_location("nc_capacity", ROOT / "conformance/capacity.py")
                    capacities = importlib.util.module_from_spec(capacity_spec)
                    capacity_spec.loader.exec_module(capacities)
                    def capacity_progress(name):
                        report["workload_stage"] = "capacity: " + name
                        print(report["workload_stage"], flush=True)
                    report["book_capacity_results"] = capacities.exercise(
                        client, build["emulator_clock_initial_unix_seconds"], capacity_progress)
                    report["all_business_books_at_capacity_passed"] = True
                    if args.history:
                        report["history_capacity_results"] = capacities.saturate_history(client, capacity_progress)
                        report["retained_history_at_capacity_passed"] = True
                report["workload_stage"] = "complete"
                report["banking_workload_passed"] = True
                report["diag_after_workload"] = client.get("/diag")
                if args.observe_seconds:
                    log_offset = (out / "uart.log").stat().st_size
                    observation_deadline = time.monotonic() + args.observe_seconds
                    while time.monotonic() < observation_deadline:
                        if process.poll() is not None:
                            raise TimeoutError("Emulator ended during post-workload observation")
                        time.sleep(1)
                        report["diag_after_workload"] = client.get("/diag")
                    health = (out / "uart.log").read_bytes()[log_offset:].decode("utf-8", errors="replace")
                    stacks = re.findall(r"stack free main (\d+) tls (\d+) led (\d+)", health)
                    report["post_workload_stack_samples"] = [dict(main=int(main), tls=int(tls), led=int(led))
                                                             for main, tls, led in stacks]
                    report["representative_workload_resources_observed"] = bool(stacks)
                    if not stacks:
                        raise AssertionError("No post-workload serving stack sample was captured")
        except Exception as error:
            report["probe_error"] = f"{type(error).__name__}: {error}"
        finally:
            stop_emulator(process)
            report["emulator_exit_code"] = process.returncode
            report["elapsed_wall_seconds"] = round(time.monotonic() - started_at, 2)
            if companion is not None:
                stop_emulator(companion)
                report["companion_exit_code"] = companion.returncode
                companion_log.close()
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("Complete server emulator evidence:", out / "report.json",
          "startup", report["complete_server_startup_verified"])
    passed = (report["complete_server_startup_verified"] and not report.get("probe_error")
              and (not args.workload or report["banking_workload_passed"])
              and (not args.observe_seconds or report.get("representative_workload_resources_observed")))
    raise SystemExit(0 if passed else 1)


if __name__ == "__main__":
    main()
