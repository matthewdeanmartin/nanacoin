"""Compile-only ESP probe. Never connects to, flashes, or erases hardware.

Generated COBOL C compiling is only the first gate: a target libcob and GMP
must also link, and firmware budgets must be measured before support is claimed.
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
    parser.add_argument("--prefix", type=Path, default=Path("C:/msys64/ucrt64"))
    parser.add_argument("--tools", type=Path, default=Path("C:/Espressif/tools"))
    parser.add_argument("--runtime-root", type=Path,
                        default=ROOT / ".local/cobol-firmware-source")
    args = parser.parse_args()
    prefix = args.prefix.resolve()
    out = ROOT / ".local/cobol-firmware-probe"
    out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, COB_CONFIG_DIR=str(prefix / "share/gnucobol/config"))
    subprocess.run([str(prefix / "bin/cobc.exe"), "-C", "-free", "-o",
                    str(out / "bank.c"), str(ROOT / "nanacoin_rs/cobol/bank.cob")],
                   env=env, check=True)
    # Only copy the portable COBOL interface, avoiding host stdlib headers.
    include = out / "include"
    include.mkdir(exist_ok=True)
    shutil.copy2(prefix / "include/libcob.h", include / "libcob.h")
    shutil.copytree(prefix / "include/libcob", include / "libcob", dirs_exist_ok=True)
    report = {"hardware_access": False, "generated_c": "bank.c",
              "source_sha256": hashlib.sha256((ROOT / "nanacoin_rs/cobol/bank.cob").read_bytes()).hexdigest(),
              "generated_c_sha256": hashlib.sha256((out / "bank.c").read_bytes()).hexdigest(),
              "targets": []}
    compilers = sorted(args.tools.rglob("*-elf-gcc.exe"))
    for compiler in compilers:
        if compiler.name not in ("xtensa-esp32-elf-gcc.exe", "xtensa-esp32s2-elf-gcc.exe",
                                  "xtensa-esp32s3-elf-gcc.exe", "riscv32-esp-elf-gcc.exe"):
            continue
        name = compiler.stem
        target = {"xtensa-esp32-elf-gcc": "esp32", "xtensa-esp32s2-elf-gcc": "esp32s2",
                  "xtensa-esp32s3-elf-gcc": "esp32s3", "riscv32-esp-elf-gcc": "esp32p4"}[name]
        gmp = args.runtime_root / ("build-gmp-" + target)
        runtime_archive = args.runtime_root / ("build-libcob-" + target) / "libcob/.libs/libcob.a"
        gmp_archive = gmp / ".libs/libgmp.a"
        obj = out / (name + ".o")
        command = [str(compiler), "-Os", "-ffunction-sections", "-fdata-sections",
                   "-I", str(include), "-c", str(out / "bank.c"), "-o", str(obj)]
        if (gmp / "gmp.h").is_file():
            command[1:1] = ["-I", str(gmp.resolve())]
        result = subprocess.run(command, capture_output=True, text=True)
        (out / (name + ".log")).write_text(result.stdout + result.stderr, encoding="utf-8")
        runtime = list(compiler.parent.parent.rglob("libcob.a"))
        arithmetic = list(compiler.parent.parent.rglob("libgmp.a"))
        entry = {"compiler": str(compiler), "generated_c_compiles": result.returncode == 0,
                 "object_bytes": obj.stat().st_size if result.returncode == 0 else None,
                 "target_libcob_found": runtime_archive.is_file() or bool(runtime),
                 "target_gmp_found": gmp_archive.is_file() or bool(arithmetic),
                 "target_gmp_header": str(gmp / "gmp.h") if (gmp / "gmp.h").is_file() else None,
                 "firmware_link_verified": False, "deployment_supported": False}
        report["targets"].append(entry)
        print(name, "C compile passed" if result.returncode == 0 else "C compile failed",
              "; firmware runtime/link unverified")
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("Compile-only evidence:", out / "report.json")


if __name__ == "__main__":
    main()
