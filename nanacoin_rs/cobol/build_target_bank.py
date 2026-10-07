"""Compile the current banking module for optional ESP-IDF static linkage.

Consumes target GMP/libcob archives from build_target_runtime.py. This only
builds files; it never connects to a board or invokes flashing/deployment.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
RUST_TARGETS = {"esp32": "xtensa-esp32-espidf", "esp32s2": "xtensa-esp32s2-espidf",
                "esp32s3": "xtensa-esp32s3-espidf", "esp32p4": "riscv32imafc-esp-espidf"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=RUST_TARGETS, required=True)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--gnucobol-source", type=Path, required=True)
    parser.add_argument("--runtime-root", type=Path, default=ROOT / ".local/cobol-firmware-source")
    parser.add_argument("--cob-prefix", type=Path, default=Path("C:/msys64/ucrt64"))
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--lto", action="store_true")
    args = parser.parse_args()
    prefix = "riscv32-esp-elf" if args.target == "esp32p4" else "xtensa-" + args.target + "-elf"
    compiler = args.compiler.resolve()
    if compiler.name != prefix + "-gcc.exe":
        parser.error("Compiler must match target: " + prefix + "-gcc.exe")
    out = args.output_dir.resolve() if args.output_dir else ROOT / ".local" / ("cobol-target-" + args.target)
    out.mkdir(parents=True, exist_ok=True)
    runtime_root = args.runtime_root.resolve()
    flags_marker = runtime_root / ("compiler-flags-" + args.target + ".txt")
    if not flags_marker.is_file() or ("-flto" in flags_marker.read_text()) != args.lto:
        parser.error("Runtime compiler flags missing or LTO mode does not match; rebuild target runtime")
    gmp = runtime_root / ("build-gmp-" + args.target)
    runtime = runtime_root / ("build-libcob-" + args.target)
    shim = runtime_root / ("static-loader-" + args.target)
    archives = {"libcob.a": runtime / "libcob/.libs/libcob.a",
                "libgmp.a": gmp / ".libs/libgmp.a", "libltdl.a": shim / "libltdl.a"}
    for archive in archives.values():
        if not archive.is_file():
            parser.error("Missing target runtime archive: " + str(archive))
    flags = ["-Os", "-ffunction-sections", "-fdata-sections"]
    if args.lto:
        flags.append("-flto")
    flags += ["-march=rv32imafc_zicsr_zifencei", "-mabi=ilp32f"] if args.target == "esp32p4" else ["-mlongcalls"]
    env = dict(os.environ, COB_CONFIG_DIR=str(args.cob_prefix / "share/gnucobol/config"))
    with (out / "build.log").open("w", encoding="utf-8") as log:
        def run(command):
            subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        run([str(args.cob_prefix / "bin/cobc.exe"), "-C", "-free", "-o", str(out / "bank.c"), str(HERE / "bank.cob")])
        objects = []
        for source in [out / "bank.c", HERE / "bridge.c", HERE / "static_posix.c"]:
            obj = out / (source.stem + ".o")
            run([str(compiler)] + flags + ["-I", str(gmp), "-I", str(runtime), "-I", str(args.gnucobol_source.resolve()),
                                         "-c", str(source), "-o", str(obj)])
            objects.append(str(obj))
        archive = out / "libncbank.a"
        # Start a new archive: never retain removed objects from a prior build.
        if archive.exists():
            archive.unlink()
        run([str(compiler.with_name(prefix + "-ar.exe")), "rcs", str(archive)] + objects)
    for name, path in archives.items():
        shutil.copy2(path, out / name)
    hashes = {name: hashlib.sha256((out / name).read_bytes()).hexdigest()
              for name in ["libncbank.a"] + list(archives)}
    manifest = {"target": RUST_TARGETS[args.target], "abi": 7, "slots": 64,
                "capabilities": "FFFFFFFFFFFFFFFE", "hardware_access": False,
                "compiler": str(compiler), "compiler_flags": flags,
                "source_sha256": hashlib.sha256((HERE / "bank.cob").read_bytes()).hexdigest(),
                "bridge_sha256": hashlib.sha256((HERE / "bridge.c").read_bytes()).hexdigest(),
                "static_posix_sha256": hashlib.sha256((HERE / "static_posix.c").read_bytes()).hexdigest(),
                "archives": hashes, "deployment_supported": False}
    (out / "bank-engine.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    # Small machine-readable build contract consumed without new build deps.
    (out / "bank-target.txt").write_text(RUST_TARGETS[args.target] + "\n7\n64\n", encoding="utf-8", newline="\n")
    print("Static target module:", out)


if __name__ == "__main__":
    main()
