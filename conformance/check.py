"""Build and test the Rust bank, optionally both selectable banking builds."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def run(command, env):
    subprocess.run(command, cwd=ROOT, env=env, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cobol", action="store_true", help="Also test COBOL and reopening data across builds in both directions")
    parser.add_argument("--prefix", type=Path, default=Path("C:/msys64/ucrt64"))
    parser.add_argument("--clock", action="store_true", help="Also build the opt-in launcher clock and run deterministic time contracts")
    args = parser.parse_args()
    env = dict(os.environ)
    rust_target = ROOT / ".local/conformance-rust"
    cobol_target = ROOT / ".local/conformance-cobol/rust"
    banks = [("rust", rust_target, ["--features", "conformance-clock"] if args.clock else [])]
    if args.cobol:
        run([sys.executable, str(ROOT / "nanacoin_rs/cobol/build.py"), "--prefix", str(args.prefix)], env)
        dll = ROOT / ".local/conformance-cobol/ncposting.dll"
        env["NANACOIN_COBOL_DLL"] = str(dll)
        run([sys.executable, str(ROOT / "nanacoin_rs/cobol/test_abi.py"), str(dll)], env)
        banks.append(("cobol", cobol_target, ["--features", "cobol-core,conformance-clock" if args.clock else "cobol-core"]))
    for _, target, features in banks:
        run(["cargo", "build", "--locked", "--manifest-path", str(ROOT / "nanacoin_rs/Cargo.toml"),
             "--bin", "nanacoin", "--target-dir", str(target), *features], env)
    exe = "nanacoin.exe" if os.name == "nt" else "nanacoin"
    if args.cobol:
        run([sys.executable, str(ROOT / "nanacoin_rs/cobol/test_startup.py"),
             "--server", str(cobol_target / "debug" / exe),
             "--prefix", str(args.prefix)], env)
    def check_bank(index):
        name, target, _ = banks[index]
        actual = subprocess.check_output([str(target / "debug" / exe), "--bank-engine"], env=env, text=True).strip()
        if actual != name:
            raise SystemExit(f"Expected {name} banking build, got {actual!r}")
        command = [sys.executable, str(ROOT / "conformance/run.py"), "--server", str(target / "debug" / exe),
                   "--xml", str(target / "conformance.xml")]
        if args.clock:
            command.append("--clock")
        if args.cobol:
            command.extend(["--restart-server", str(banks[1 - index][1] / "debug" / exe)])
        print(f"Checking {name}" + ("; restart fixtures reopen with the other build" if args.cobol else ""), flush=True)
        log_path = target / "conformance.log"
        with log_path.open("w", encoding="utf-8") as log:
            result = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        if result.returncode:
            print(log_path.read_text(encoding="utf-8", errors="replace"), flush=True)
            raise subprocess.CalledProcessError(result.returncode, command)
        print(f"{name}: contracts passed; {log_path}", flush=True)

    # Both executables and the DLL are built before any server starts. Each
    # contract owns a separate ephemeral port/data directory; restart targets
    # are immutable executables, so engines can be checked concurrently.
    with ThreadPoolExecutor(max_workers=len(banks)) as pool:
        results = [pool.submit(check_bank, index) for index in range(len(banks))]
        for result in results:
            result.result()
    if args.cobol and args.clock:
        run([sys.executable, str(ROOT / "conformance/differential.py"),
             "--rust", str(rust_target / "debug" / exe),
             "--cobol", str(cobol_target / "debug" / exe),
             "--report", str(ROOT / ".local/conformance-differential.json")], env)


if __name__ == "__main__":
    main()
