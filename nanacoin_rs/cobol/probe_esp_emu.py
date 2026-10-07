"""Run the isolated P4/S3 probe in Espressif's Linux emulator via Windows WSL.

The HTTP conformance suite remains native Windows. This separate runtime
investigation uses a checksum-verified Espressif emulator release; it never
connects to hardware. It does not validate the complete bank firmware workload.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def linux_path(path):
    path = Path(path).resolve().as_posix()
    if path[1:3] != ":/":
        raise ValueError("Expected Windows drive path")
    return "/mnt/" + path[0].lower() + path[2:]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=["esp32p4", "esp32s3"], required=True)
    parser.add_argument("--emulator", type=Path, required=True)
    parser.add_argument("--distribution", default="Ubuntu")
    parser.add_argument("--esp-root", type=Path, default=Path("C:/Espressif"))
    args = parser.parse_args()
    out = ROOT / ".local/cobol-firmware-probe" / ("idf-" + args.target)
    build = out / "build"
    linked = json.loads((out / "report.json").read_text(encoding="utf-8"))
    elf = build / "nanacoin_cobol_probe.elf"
    if not linked["rust_cobol_idf_link_verified"] or hashlib.sha256(elf.read_bytes()).hexdigest() != linked["elf_sha256"]:
        raise RuntimeError("Build the current probe image before emulation")
    if linked["source_sha256"] != hashlib.sha256((ROOT / "nanacoin_rs/cobol/bank.cob").read_bytes()).hexdigest():
        raise RuntimeError("Probe image does not contain the current COBOL source")
    report = {"target": args.target, "hardware_access": False,
              "emulator_sha256": hashlib.sha256(args.emulator.read_bytes()).hexdigest(),
              "elf_sha256": linked["elf_sha256"], "bank_firmware_validated": False,
              "deployment_supported": False, "runtime_probe_passed": False}
    report_path = out / "esp-emu-report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    python = args.esp_root / "python_env/idf5.5_py3.11_env/Scripts/python.exe"
    config = json.loads((build / "config/sdkconfig.json").read_text(encoding="utf-8"))
    flash = build / "esp_emu_flash.bin"
    with (out / "esp-emu-merge.log").open("w", encoding="utf-8") as log:
        subprocess.run([str(python), "-m", "esptool", "--chip=" + args.target, "merge_bin",
                        "--output=" + str(flash), "--fill-flash-size=" + config["ESPTOOLPY_FLASHSIZE"],
                        "@flash_args"], cwd=build, stdout=log, stderr=subprocess.STDOUT, check=True)
    command = ["wsl", "-d", args.distribution, "--", linux_path(args.emulator),
               "--chip", args.target, "--firmware", linux_path(flash), "--timeout", "30s",
               "--exit-on", "NC_PROBE_DONE", "--net", "user,restrict=yes", "--psram-size", "0"]
    log_path = out / "esp-emu.log"
    with log_path.open("w", encoding="utf-8") as log:
        result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=50)
    observation = None
    for line in log_path.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith("NC_PROBE "):
            observation = json.loads(line.removeprefix("NC_PROBE "))
    report.update(exit_code=result.returncode, observation=observation,
                  runtime_probe_passed=bool(result.returncode == 0 and observation and
                                          observation["result"] == 0 and observation["calls"] == linked["expected_calls"]))
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("Espressif emulator evidence:", report_path, observation)
    if not report["runtime_probe_passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
