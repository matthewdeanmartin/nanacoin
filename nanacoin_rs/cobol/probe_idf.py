"""Link a minimal Rust -> static COBOL ESP-IDF image without accessing hardware.

Requires target archives from build_target_runtime.py. This does not enable
cobol-core in the bank firmware or certify runtime heap/stack budgets.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
from firmware_vectors import emit

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
TARGETS = {"esp32": "xtensa-esp32-espidf", "esp32s2": "xtensa-esp32s2-espidf",
           "esp32s3": "xtensa-esp32s3-espidf", "esp32p4": "riscv32imafc-esp-espidf"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--gnucobol-source", type=Path, required=True)
    parser.add_argument("--runtime-root", type=Path, default=ROOT / ".local/cobol-firmware-source")
    parser.add_argument("--esp-root", type=Path, default=Path("C:/Espressif"))
    parser.add_argument("--cob-prefix", type=Path, default=Path("C:/msys64/ucrt64"))
    parser.add_argument("--abi-vectors", type=Path, help="Exported public C ABI fixtures to exercise in target runtime")
    args = parser.parse_args()
    out = ROOT / ".local/cobol-firmware-probe" / ("idf-" + args.target)
    component = out / "main"
    component.mkdir(parents=True, exist_ok=True)
    runtime_root = args.runtime_root.resolve()
    gmp = runtime_root / ("build-gmp-" + args.target)
    runtime = runtime_root / ("build-libcob-" + args.target)
    shim = runtime_root / ("static-loader-" + args.target)
    archives = [runtime / "libcob/.libs/libcob.a", gmp / ".libs/libgmp.a", shim / "libltdl.a"]
    for archive in archives:
        if not archive.is_file():
            parser.error("Missing target archive: " + str(archive))
    idf = args.esp_root / "frameworks/esp-idf-v5.5.3"
    python = args.esp_root / "python_env/idf5.5_py3.11_env/Scripts/python.exe"
    env = dict(os.environ)
    env.pop("MSYSTEM", None)  # Native Windows CMake/Python, not an MSYS IDF build.
    env.update(IDF_PATH=str(idf), IDF_TOOLS_PATH=str(args.esp_root),
               IDF_PYTHON_ENV_PATH=str(python.parent.parent),
               ESP_ROM_ELF_DIR=str(args.esp_root / "tools/esp-rom-elfs/20241011"))
    cross = args.esp_root / "tools" / ("riscv32-esp-elf" if args.target == "esp32p4" else "xtensa-esp-elf")
    cross = cross / "esp-14.2.0_20251107" / cross.name / "bin"
    env["PATH"] = os.pathsep.join(str(p) for p in [args.esp_root / "tools/cmake/3.30.2/bin",
                           args.esp_root / "tools/ninja/1.12.1", cross, python.parent]) + os.pathsep + env["PATH"]
    rust = out / "rust"
    (rust / "src").mkdir(parents=True, exist_ok=True)
    shutil.copy2(HERE / "firmware_probe/rust.rs", rust / "src/lib.rs")
    (rust / "Cargo.toml").write_text('''[package]
name="nanacoin-cobol-static-probe"
version="0.0.0"
edition="2021"
[lib]
crate-type=["staticlib"]
[profile.release]
opt-level="s"
panic="abort"
[workspace]
''', encoding="utf-8")
    # Keep Cargo independent of the caller's bank output directory.
    env.pop("CARGO_TARGET_DIR", None)
    rust_target = TARGETS[args.target]
    report = {"target": args.target, "hardware_access": False,
              "source_sha256": hashlib.sha256((HERE / "bank.cob").read_bytes()).hexdigest(),
              "rust_cobol_idf_link_verified": False, "bank_firmware_integrated": False,
              "runtime_heap_stack_measured": False, "deployment_supported": False}
    compile_vectors = ""
    if args.abi_vectors:
        count = emit(args.abi_vectors, component / "vectors.h")
        report["boundary_vectors"] = count
        report["expected_calls"] = 387 + 2 * count
        report["vectors_sha256"] = hashlib.sha256(args.abi_vectors.read_bytes()).hexdigest()
        compile_vectors = 'target_compile_definitions(${COMPONENT_LIB} PRIVATE NC_ABI_VECTORS=1)\n'
    else:
        report["expected_calls"] = 387
    report_path = out / "report.json"

    def run(stage, command, stage_env=env):
        with (out / (stage + ".log")).open("w", encoding="utf-8") as log:
            result = subprocess.run(command, cwd=out, env=stage_env, stdout=log, stderr=subprocess.STDOUT)
        report[stage + "_exit_code"] = result.returncode
        report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(stage, "exit", result.returncode, "log", out / (stage + ".log"), flush=True)
        if result.returncode:
            raise SystemExit(result.returncode)

    run("rust", ["cargo", "+esp", "build", "--manifest-path", str(rust / "Cargo.toml"),
                 "-Z", "build-std=core", "--target", rust_target, "--release"])
    cob_env = dict(env, COB_CONFIG_DIR=str(args.cob_prefix / "share/gnucobol/config"))
    run("generate", [str(args.cob_prefix / "bin/cobc.exe"), "-C", "-free", "-o",
                     str(component / "bank.c"), str(HERE / "bank.cob")], cob_env)
    for name in ("main.c", "posix.c", "allocations.c"):
        shutil.copy2(HERE / "firmware_probe" / name, component / name)
    rust_archive = rust / "target" / rust_target / "release/libnanacoin_cobol_static_probe.a"
    libraries = " ".join('"' + p.as_posix() + '"' for p in [rust_archive] + archives)
    includes = " ".join('"' + p.resolve().as_posix() + '"' for p in [gmp, runtime, args.gnucobol_source])
    (component / "CMakeLists.txt").write_text(
        f'idf_component_register(SRCS "main.c" "bank.c" INCLUDE_DIRS {includes} REQUIRES vfs)\n'
        + compile_vectors +
        # Runtime and Rust archives refer back to IDF and generated bank objects.
        # Rescan the complete archive list rather than depend on first-pass order.
        'target_link_options(${COMPONENT_LIB} INTERFACE "-Wl,--start-group")\n'
        'target_link_options(${COMPONENT_LIB} INTERFACE "-Wl,--wrap=malloc" "-Wl,--wrap=calloc" "-Wl,--wrap=realloc" "-Wl,--wrap=free")\n'
        f'target_link_libraries(${{COMPONENT_LIB}} PUBLIC {libraries} "-Wl,--end-group")\n', encoding="utf-8")
    (out / "CMakeLists.txt").write_text('''cmake_minimum_required(VERSION 3.16)
set(COMPONENTS main)
include($ENV{IDF_PATH}/tools/cmake/project.cmake)
project(nanacoin_cobol_probe)
''', encoding="utf-8")
    (out / "sdkconfig.defaults").write_text("CONFIG_ESP_MAIN_TASK_STACK_SIZE=16384\n", encoding="utf-8")
    run("idf", [str(python), str(idf / "tools/idf.py"), "-C", str(out),
                "-D", "IDF_TARGET=" + args.target, "build"])
    elf = out / "build/nanacoin_cobol_probe.elf"
    if not elf.is_file():
        raise RuntimeError("IDF returned success without the expected linked image")
    report.update(rust_cobol_idf_link_verified=True, elf_sha256=hashlib.sha256(elf.read_bytes()).hexdigest())
    run("size", [str(python), "-m", "esp_idf_size", "--format", "json",
                 str(out / "build/nanacoin_cobol_probe.map")])
    size_output = (out / "size.log").read_text(encoding="utf-8")
    start = size_output.index("{")
    report["size_warnings"] = size_output[:start].strip()
    report["size_data"] = json.loads(size_output[start:])
    report["app_image_bytes"] = (out / "build/nanacoin_cobol_probe.bin").stat().st_size
    # P4's installed linker SRAM region extends beyond the size tool's chip
    # map. Keep raw section sizes; do not advertise that surplus as free RAM.
    size_tool = cross / (("riscv32-esp-elf" if args.target == "esp32p4" else "xtensa-" + args.target + "-elf") + "-size.exe")
    run("sections", [str(size_tool), "-A", str(elf)])
    report["allocated_sections"] = {}
    for line in (out / "sections.log").read_text(encoding="utf-8").splitlines():
        fields = line.split()
        if len(fields) == 3 and fields[0].startswith(".") and fields[1].isdigit() and fields[2].isdigit():
            if int(fields[2]) and not fields[0].endswith("dummy"):
                report["allocated_sections"][fields[0]] = int(fields[1])
    report["free_ram_budget_verified"] = not bool(report["size_warnings"])
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("Build-only evidence:", report_path)


if __name__ == "__main__":
    main()
