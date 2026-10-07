"""Execute the isolated static banking probe in Espressif QEMU, never hardware.

Checks target financial results and records emulator heap/allocation/stack
observations. These are probe-workload measurements, not full bank firmware
budgets. QEMU has no network interface in this runner.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=["esp32", "esp32s2", "esp32s3", "esp32p4"], required=True)
    parser.add_argument("--esp-root", type=Path, default=Path("C:/Espressif"))
    args = parser.parse_args()
    out = ROOT / ".local/cobol-firmware-probe" / ("idf-" + args.target)
    build = out / "build"
    report = {"target": args.target, "hardware_access": False, "network_access": False,
              "bank_firmware_validated": False, "deployment_supported": False}
    report_path = out / "qemu-report.json"
    idf = args.esp_root / "frameworks/esp-idf-v5.5.3"
    python = args.esp_root / "python_env/idf5.5_py3.11_env/Scripts/python.exe"
    # Use this installation's official machine arguments and emulated eFuses.
    metadata_code = ("import sys,json;sys.path.insert(0,sys.argv[1]);"
                     "from idf_py_actions.qemu_ext import QEMU_TARGETS;"
                     "t=QEMU_TARGETS.get(sys.argv[2]);"
                     "print(json.dumps(None if t is None else [t.qemu_prog,t.qemu_args,t.default_efuse.hex()]))")
    metadata = json.loads(subprocess.check_output([str(python), "-c", metadata_code,
                          str(idf / "tools"), args.target], text=True))
    report["emulation_supported"] = metadata is not None
    out.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if metadata is None:
        print("Installed Espressif QEMU profile does not support", args.target)
        raise SystemExit(2)
    linked = json.loads((out / "report.json").read_text(encoding="utf-8"))
    elf = build / "nanacoin_cobol_probe.elf"
    if not linked["rust_cobol_idf_link_verified"] or hashlib.sha256(elf.read_bytes()).hexdigest() != linked["elf_sha256"]:
        raise RuntimeError("Build the current probe image before running QEMU")
    if linked["source_sha256"] != hashlib.sha256((ROOT / "nanacoin_rs/cobol/bank.cob").read_bytes()).hexdigest():
        raise RuntimeError("Probe image does not contain the current COBOL source")
    binaries = sorted((args.esp_root / "tools/qemu-xtensa").rglob(metadata[0] + ".exe"))
    if not binaries:
        raise RuntimeError("Install Espressif qemu-xtensa with idf_tools.py")
    flash = build / "qemu_flash.bin"
    efuse = build / "qemu_efuse.bin"
    efuse.write_bytes(bytes.fromhex(metadata[2]))
    config = json.loads((build / "config/sdkconfig.json").read_text(encoding="utf-8"))
    merge = [str(python), "-m", "esptool", "--chip=" + args.target, "merge_bin",
             "--output=" + str(flash), "--fill-flash-size=" + config["ESPTOOLPY_FLASHSIZE"], "@flash_args"]
    with (out / "qemu-merge.log").open("w", encoding="utf-8") as log:
        subprocess.run(merge, cwd=build, stdout=log, stderr=subprocess.STDOUT, check=True)
    command = [str(binaries[-1])] + metadata[1].split() + [
        "-drive", f"file={flash},if=mtd,format=raw",
        "-drive", f"file={efuse},if=none,format=raw,id=efuse",
        "-global", f"driver=nvram.{args.target}.efuse,property=drive,value=efuse",
        "-global", f"driver=timer.{args.target}.timg,property=wdt_disable,value=true",
        "-nic", "none", "-nographic", "-serial", "stdio", "-monitor", "none"]
    log_path = out / "qemu.log"
    observation = None
    with log_path.open("wb") as log:
        proc = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT,
                                creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        try:
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                content = log_path.read_text(encoding="utf-8", errors="replace")
                for line in content.splitlines():
                    if line.startswith("NC_PROBE "):
                        observation = json.loads(line.removeprefix("NC_PROBE "))
                        break
                if observation is not None or proc.poll() is not None:
                    break
                time.sleep(0.25)
        finally:
            if proc.poll() is None:
                proc.terminate()
            proc.wait(timeout=10)
    report.update(elf_sha256=linked["elf_sha256"], observation=observation,
                  runtime_probe_passed=bool(observation and observation["result"] == 0 and observation["calls"] == linked["expected_calls"]))
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("Emulator evidence:", report_path, observation)
    if not report["runtime_probe_passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
