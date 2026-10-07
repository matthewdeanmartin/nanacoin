"""Build investigation-only target GMP/libcob archives; never accesses hardware.

Requires MSYS make/m4, unpacked GNU GMP and GnuCOBOL source distributions,
and an installed Espressif cross compiler. Source distributions are not modified.
This is not a firmware packaging or deployment command.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def msys(path):
    path = Path(path).resolve().as_posix()
    if len(path) < 3 or path[1:3] != ":/":
        raise ValueError("Expected an absolute Windows drive path: " + path)
    return "/" + path[0].lower() + path[2:]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--gmp-source", type=Path, required=True)
    parser.add_argument("--gnucobol-source", type=Path, required=True)
    parser.add_argument("--target", choices=["esp32", "esp32s2", "esp32s3", "esp32p4"], required=True)
    parser.add_argument("--msys", type=Path, default=Path("C:/msys64"))
    parser.add_argument("--output-root", type=Path, default=ROOT / ".local/cobol-firmware-source")
    parser.add_argument("--lto", action="store_true", help="Investigate GCC link-time optimization in isolated archives")
    args = parser.parse_args()
    for source in (args.gmp_source, args.gnucobol_source):
        if not (source / "configure").is_file():
            parser.error("Missing source configure: " + str(source))
    compiler = args.compiler.resolve()
    if not compiler.is_file():
        parser.error("Missing cross compiler: " + str(compiler))
    prefix = compiler.stem.removesuffix("-gcc")
    expected = "riscv32-esp-elf" if args.target == "esp32p4" else "xtensa-" + args.target + "-elf"
    if prefix != expected:
        parser.error(f"Target {args.target} requires {expected}-gcc, got {prefix}-gcc")
    out = args.output_root.resolve()
    gmp = out / ("build-gmp-" + args.target)
    runtime = out / ("build-libcob-" + args.target)
    shim = out / ("static-loader-" + args.target)
    for directory in (gmp, runtime, shim):
        directory.mkdir(parents=True, exist_ok=True)
    for name in ("ltdl.c", "ltdl.h"):
        shutil.copy2(Path(__file__).parent / "firmware_probe" / name, shim / name)
    q = lambda path: shlex.quote(msys(path))
    # Autoconf embeds srcdir in generated C #include strings. MSYS drive paths
    # there are not converted by argv handling; relative paths work for GCC.
    def configure(source, build):
        return shlex.quote(os.path.relpath(source.resolve() / "configure", build).replace("\\", "/"))
    flags = "-Os --specs=nosys.specs -ffunction-sections -fdata-sections"
    if args.lto:
        flags += " -flto"
    if prefix.startswith("xtensa-"):
        # IDF places flash code far from ROM functions such as abort().
        flags += " -mlongcalls"
    else:
        # P4 ESP-IDF uses the single-float ABI. The generic GCC default is
        # soft-float, which cannot be linked into that image.
        flags += " -march=rv32imafc_zicsr_zifencei -mabi=ilp32f"
    flags_marker = out / ("compiler-flags-" + args.target + ".txt")
    clean = "make clean" if not flags_marker.exists() or flags_marker.read_text() != flags else ":"
    script = f"""#!/usr/bin/env bash
set -eu
export PATH={q(args.msys / 'ucrt64/bin')}:{q(compiler.parent)}:{q(args.msys / 'usr/bin')}:"$PATH"
cd {q(gmp)}
{configure(args.gmp_source, gmp)} --host={prefix} --build=x86_64-w64-mingw32 --disable-assembly --disable-shared --enable-static CC={prefix}-gcc AR={prefix}-ar RANLIB={prefix}-ranlib CC_FOR_BUILD=gcc CFLAGS='{flags}' LDFLAGS='-Wl,--gc-sections'
{clean}
make -j4
{prefix}-gcc {flags} -c {q(shim / 'ltdl.c')} -o {q(shim / 'ltdl.o')}
{prefix}-ar rcs {q(shim / 'libltdl.a')} {q(shim / 'ltdl.o')}
cd {q(runtime)}
{configure(args.gnucobol_source, runtime)} --host={prefix} --build=x86_64-w64-mingw32 --disable-shared --enable-static --without-curses --without-db --without-xml2 --with-json=no --without-dl --disable-nls CC={prefix}-gcc AR={prefix}-ar RANLIB={prefix}-ranlib GMP_CFLAGS='-I{msys(gmp)}' GMP_LIBS={q(gmp / '.libs/libgmp.a')} CPPFLAGS='-I{msys(shim)}' CFLAGS='{flags}' LDFLAGS='-Wl,--gc-sections -L{msys(shim)}'
{clean} -C libcob
make -j4 -C libcob
"""
    script_path = out / ("build-runtime-" + args.target + ".sh")
    script_path.write_text(script, encoding="utf-8", newline="\n")
    log = out / ("runtime-" + args.target + ".log")
    with log.open("w", encoding="utf-8") as stream:
        result = subprocess.run([str(args.msys / "usr/bin/bash.exe"), str(script_path)],
                                stdout=stream, stderr=subprocess.STDOUT)
    report = {"target": args.target, "compiler": str(compiler),
              "exit_code": result.returncode, "hardware_access": False,
              "gmp_archive": str(gmp / ".libs/libgmp.a"),
              "libcob_archive": str(runtime / "libcob/.libs/libcob.a"),
              "static_loader_archive": str(shim / "libltdl.a"),
              "compiler_flags": flags,
              "script_sha256": hashlib.sha256(script_path.read_bytes()).hexdigest(),
              "runtime_execution_verified": False,
              "dynamic_modules_supported": False,
              "firmware_link_verified": False, "deployment_supported": False}
    if result.returncode == 0:
        flags_marker.write_text(flags, encoding="utf-8")
        report["archive_sha256"] = {str(path): hashlib.sha256(path.read_bytes()).hexdigest()
                                    for path in [gmp / ".libs/libgmp.a",
                                                 runtime / "libcob/.libs/libcob.a", shim / "libltdl.a"]}
    (out / ("runtime-" + args.target + ".json")).write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("Runtime investigation:", log, "exit", result.returncode)
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
