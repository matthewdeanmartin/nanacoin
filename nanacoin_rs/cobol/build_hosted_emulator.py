"""Build a local ESP-Hosted C6 companion for P4 emulation; never flash hardware.

Copies the matching host component's supplied slave example and common code
into disposable investigation storage. No source distribution is modified.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--hosted-source", type=Path, required=True)
    parser.add_argument("--build-dir", type=Path, default=Path("C:/nc-emu-c6"))
    parser.add_argument("--esp-root", type=Path, default=Path("C:/Espressif"))
    args = parser.parse_args()
    source = args.hosted_source.resolve()
    out = ROOT / ".local/cobol-hosted-emulator"
    project = out / "slave"
    if not (source / "slave/CMakeLists.txt").is_file() or not (source / "common").is_dir():
        parser.error("Requires the complete matching espressif__esp_hosted component")
    for name in ("slave", "common"):
        shutil.copytree(source / name, out / name, dirs_exist_ok=True)
    idf = args.esp_root / "frameworks/esp-idf-v5.5.3"
    python = args.esp_root / "python_env/idf5.5_py3.11_env/Scripts/python.exe"
    cross = args.esp_root / "tools/riscv32-esp-elf/esp-14.2.0_20251107/riscv32-esp-elf/bin"
    env = dict(os.environ)
    env.pop("MSYSTEM", None)
    env.update(IDF_PATH=str(idf), IDF_TOOLS_PATH=str(args.esp_root), IDF_TARGET="esp32c6",
               IDF_PYTHON_ENV_PATH=str(python.parent.parent),
               ESP_ROM_ELF_DIR=str(args.esp_root / "tools/esp-rom-elfs/20241011"))
    env["PATH"] = os.pathsep.join(map(str, [cross, idf / "tools", python.parent,
        args.esp_root / "tools/cmake/3.30.2/bin", args.esp_root / "tools/ninja/1.12.1"])) + os.pathsep + env["PATH"]
    build = args.build_dir.resolve()
    with (out / "build.log").open("w", encoding="utf-8") as log:
        result = subprocess.run([str(python), str(idf / "tools/idf.py"), "-B", str(build),
                                 "-DIDF_TARGET=esp32c6", "build"], cwd=project, env=env,
                                stdout=log, stderr=subprocess.STDOUT)
    report = {"hardware_access": False, "deployment_supported": False,
              "source": str(source), "build_dir": str(build), "exit_code": result.returncode}
    if result.returncode == 0:
        report["image_sha256"] = {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                                  for path in build.glob("*.bin")}
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("Hosted emulator companion:", out / "report.json", "exit", result.returncode)
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
