"""Build the Windows COBOL experiment; optionally run the HTTP contracts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", type=Path, default=Path("C:/msys64/ucrt64"),
                        help="64-bit MSYS2 GnuCOBOL installation prefix")
    parser.add_argument("--check", action="store_true", help="Build Rust and run conformance")
    args = parser.parse_args()
    subprocess.run([sys.executable, str(ROOT / "nanacoin_rs/cobol/inventory.py")], check=True)
    prefix = args.prefix.resolve()
    out = ROOT / ".local" / "conformance-cobol"
    out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env.update(PATH=str(prefix / "bin") + os.pathsep + env.get("PATH", ""),
               COB_CONFIG_DIR=str(prefix / "share/gnucobol/config"),
               COB_COPY_DIR=str(prefix / "share/gnucobol/copy"),
               COB_CFLAGS=f'-I"{prefix / "include"}"',
               COB_LIBS=f'-L"{prefix / "lib"}" -lcob',
               COB_CC=str(prefix / "bin/gcc.exe"))
    cobc = prefix / "bin/cobc.exe"
    subprocess.run([str(cobc), "-V"], env=env, check=True)
    dll = out / "ncposting.dll"
    subprocess.run([str(cobc), "-b", "-free", "-Wall", "-o", str(dll),
                    str(ROOT / "nanacoin_rs/cobol/bank.cob"),
                    str(ROOT / "nanacoin_rs/cobol/bridge.c")], env=env, check=True)
    # Copy dependent runtimes beside the module: don't depend on a user's PATH
    # when Windows loads the DLL from Rust. All are local toolchain artifacts.
    for dependency in (prefix / "bin").glob("*.dll"):
        shutil.copy2(dependency, out / dependency.name)
    sources = [ROOT / "nanacoin_rs/cobol/bank.cob", ROOT / "nanacoin_rs/cobol/bridge.c"]
    manifest = {
        "bank_engine": "cobol", "abi": 7, "slots": 64, "capabilities": "0xFFFFFFFFFFFFFFFE",
        "target": "x86_64-pc-windows-msvc", "module": dll.name,
        "compiler": subprocess.check_output([str(cobc), "-V"], env=env, text=True).splitlines()[0],
        "source_sha256": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sources},
        "module_sha256": hashlib.sha256(dll.read_bytes()).hexdigest(),
        "runtime_dlls": sorted(p.name for p in out.glob("*.dll") if p != dll),
    }
    (out / "bank-engine.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    env["NANACOIN_COBOL_DLL"] = str(dll)
    if args.check:
        subprocess.run([sys.executable, str(ROOT / "nanacoin_rs/cobol/test_abi.py"), str(dll)],
                       env=env, check=True)
        subprocess.run(["cargo", "build", "--locked", "--manifest-path", str(ROOT / "nanacoin_rs/Cargo.toml"),
                        "--bin", "nanacoin", "--features", "cobol-core", "--target-dir", str(out / "rust")],
                       env=env, check=True)
        subprocess.run([sys.executable, str(ROOT / "conformance/run.py"), "--server",
                        str(out / "rust/debug/nanacoin.exe")], env=env, check=True)
    print(f"COBOL module: {dll}")


if __name__ == "__main__":
    main()
