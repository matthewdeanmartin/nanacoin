"""Reject incompatible Windows banking modules before opening bank storage.

Supplemental executable/module integration checks, separate from HTTP contracts.
Each fixture is a metadata-only DLL; no real module or bank data is modified.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--server", type=Path, required=True)
    parser.add_argument("--prefix", type=Path, default=Path("C:/msys64/ucrt64"))
    args = parser.parse_args()
    out = ROOT / ".local/conformance-cobol/startup-rejections"
    out.mkdir(parents=True, exist_ok=True)
    compiler_env = dict(os.environ)
    compiler_env["PATH"] = str(args.prefix / "bin") + os.pathsep + compiler_env.get("PATH", "")
    results = []
    for name, abi, slots, capabilities in (
            ("wrong-abi", 6, 64, "0xFFFFFFFFFFFFFFFE"),
            ("wrong-frame", 7, 32, "0xFFFFFFFFFFFFFFFE"),
            ("missing-capabilities", 7, 64, "0")):
        source = out / (name + ".c")
        module = out / (name + ".dll")
        source.write_text(
            "#include <stdint.h>\n"
            f"__declspec(dllexport) int32_t nc_bank_abi(void) {{return {abi};}}\n"
            f"__declspec(dllexport) int32_t nc_bank_slots(void) {{return {slots};}}\n"
            f"__declspec(dllexport) uint64_t nc_bank_capabilities(void) {{return UINT64_C({capabilities});}}\n"
            "__declspec(dllexport) int32_t nc_bank_v7(int32_t op, int64_t *frame) {return 0;}\n",
            encoding="utf-8")
        subprocess.run([str(args.prefix / "bin/gcc.exe"), "-shared", "-O2", str(source),
                        "-o", str(module)], env=compiler_env, check=True)
        with tempfile.TemporaryDirectory(prefix="nanacoin-module-rejection-") as temporary:
            env = dict(os.environ, NANACOIN_COBOL_DLL=str(module.resolve()),
                       NANACOIN_JOURNAL=str(Path(temporary) / "bank.journal"))
            result = subprocess.run([str(args.server.resolve())], env=env, cwd=temporary,
                                    capture_output=True, text=True, timeout=10)
            assert result.returncode != 0, (name, result.stdout, result.stderr)
            assert "ABI mismatch" in result.stderr, (name, result.stderr)
            assert not list(Path(temporary).iterdir()), (name, "storage was opened")
            results.append(dict(case=name, exit_code=result.returncode, journal_writes=False,
                                stderr=result.stderr.strip()))
        print(f"{name}: rejected before storage writes", flush=True)
    report = out / "report.json"
    report.write_text(json.dumps(dict(abi_required=7, cases=results), indent=2), encoding="utf-8")
    print("Module startup rejection checks passed;", report, flush=True)


if __name__ == "__main__":
    main()
