"""Build and measure the actual NanaCoin Rust or static-COBOL firmware offline.

Never invokes a board runner, flashing, deployment or erasure. Separate target
directories keep the regular Rust artifacts intact during the investigation.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
BANK = ROOT / "nanacoin_rs"
sys.path.insert(0, str(BANK / "scripts"))
from boards import board  # noqa: E402


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--board", choices=["s3", "s2", "p4"], required=True)
    parser.add_argument("--engine", choices=["rust", "cobol"], required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--static-module", type=Path)
    parser.add_argument("--emulator-network", action="store_true",
                        help="Use synthetic Wi-Fi credentials for offline emulator investigation")
    parser.add_argument("--emulator-single-register", action="store_true", help="Use the existing Hosted single-register fallback in an already built isolated P4 emulator target")
    parser.add_argument("--emulator-packet-rx", action="store_true", help="Disable Hosted streaming reads in an isolated P4 emulator target")
    parser.add_argument("--emulator-debug", action="store_true", help="Enable SDK debug logs in a fresh isolated emulator target directory")
    parser.add_argument("--emulator-clock", type=int, help="Explicit initial Unix seconds for an isolated emulator-clock firmware")
    parser.add_argument("--p4-emulator-rev3", action="store_true",
                        help="Build an isolated P4 v3.1 image matching ESP-EMU's modeled silicon")
    parser.add_argument("--esp-root", type=Path, default=Path("C:/Espressif"))
    parser.add_argument("--rust-toolchain", type=Path, default=Path.home() / ".rustup/toolchains/esp")
    args = parser.parse_args()
    if args.p4_emulator_rev3 and (args.board != "p4" or not args.emulator_network):
        parser.error("P4 emulator revision requires --board p4 --emulator-network")
    if args.emulator_clock is not None and (not args.emulator_network or not 1700000000 <= args.emulator_clock <= 2000000000):
        parser.error("--emulator-clock requires synthetic networking and a valid modern Unix timestamp")
    if args.emulator_debug and not args.emulator_network:
        parser.error("--emulator-debug requires --emulator-network")
    if args.emulator_single_register and (args.board != "p4" or not args.emulator_network):
        parser.error("--emulator-single-register requires --board p4 --emulator-network")
    if args.emulator_packet_rx and (args.board != "p4" or not args.emulator_network):
        parser.error("--emulator-packet-rx requires --board p4 --emulator-network")
    profile = board(args.board)
    target_dir = args.target_dir.resolve()
    # Match esp-idf-sys's Windows 88-character output-directory gate before
    # compiling dependencies. Cargo/embuild canonical paths include \\?\.
    native_out = str(target_dir / profile.target / "release/build/esp-idf-sys-" / "out")
    predicted_out_length = len(native_out) + 16 + 4
    if os.name == "nt" and predicted_out_length > 88:
        parser.error("ESP-IDF needs a shorter --target-dir, for example C:/np4dbg")
    out = ROOT / ".local" / ("firmware-" + ("emulated-" if args.emulator_network else "") + args.engine + "-" + args.board)
    if args.emulator_debug:
        out = out.with_name(out.name + "-debug")
    for existing_sdk in target_dir.glob(profile.target + "/release/build/esp-idf-sys-*/out/sdkconfig"):
        is_debug = "CONFIG_LOG_DEFAULT_LEVEL=4" in existing_sdk.read_text()
        if is_debug != args.emulator_debug:
            parser.error("Use a fresh isolated --target-dir when changing the SDK debug profile")
        if not args.emulator_packet_rx and "CONFIG_ESP_HOSTED_SDIO_OPTIMIZATION_RX_NONE=y" in existing_sdk.read_text():
            parser.error("Target contains experimental packet reads; select --emulator-packet-rx or use a fresh target")
    adapters = list(target_dir.glob(profile.target + "/release/build/esp-idf-sys-*/out/managed_components/espressif__esp_hosted/host/drivers/transport/sdio/sdio_drv.c"))
    adapter_hash = None
    if args.emulator_single_register:
        if len(adapters) != 1:
            parser.error("First build the isolated P4 emulator target to obtain its matching Hosted driver")
        adapter = adapters[0]
        source = adapter.read_text(encoding="utf-8")
        if source.count("#define DO_COMBINED_REG_READ (1)") == 1:
            source = source.replace("#define DO_COMBINED_REG_READ (1)", "#define DO_COMBINED_REG_READ (0)")
            adapter.write_text(source, encoding="utf-8")
        elif source.count("#define DO_COMBINED_REG_READ (0)") != 1:
            parser.error("Hosted driver changed; review its register-read paths before patching")
        adapter_hash = hashlib.sha256(adapter.read_bytes()).hexdigest()
        out = out.with_name(out.name + "-single-register")
    elif any("#define DO_COMBINED_REG_READ (0)" in path.read_text(encoding="utf-8") for path in adapters):
        parser.error("Target contains the experimental driver; explicitly select --emulator-single-register or use a fresh target")
    if args.emulator_packet_rx:
        out = out.with_name(out.name + "-packet-rx")
    out.mkdir(parents=True, exist_ok=True)
    report = {"board": args.board, "engine": args.engine, "hardware_access": False,
              "target": profile.target, "target_dir": str(target_dir),
              "server_firmware_link_verified": False, "deployment_supported": False}
    env = dict(os.environ)
    env.pop("NANACOIN_EMULATOR_UNIX_SECONDS", None)
    env.pop("MSYSTEM", None)
    env.pop("NANACOIN_COBOL_STATIC_DIR", None)
    if args.emulator_network:
        env.update(NANACOIN_WIFI_SSID="nc-emulator-ssid", NANACOIN_WIFI_PASSWORD="nc-emulator-password")
    if args.emulator_clock is not None:
        env["NANACOIN_EMULATOR_UNIX_SECONDS"] = str(args.emulator_clock)
    report["emulator_clock_initial_unix_seconds"] = args.emulator_clock
    report["emulator_single_register_reads"] = args.emulator_single_register
    report["emulator_packet_reads"] = args.emulator_packet_rx
    report["emulator_hosted_driver_sha256"] = adapter_hash
    report["emulator_debug_logging"] = args.emulator_debug
    report["synthetic_emulator_network"] = args.emulator_network
    report["p4_emulator_revision"] = "3.1" if args.p4_emulator_rev3 else None
    if args.engine == "cobol":
        if args.static_module is None:
            parser.error("COBOL requires --static-module from build_target_bank.py")
        module = args.static_module.resolve()
        manifest = json.loads((module / "bank-engine.json").read_text(encoding="utf-8"))
        if manifest["target"] != profile.target or manifest["abi"] != 7 or manifest["slots"] != 64:
            parser.error("Static module target/ABI does not match firmware")
        if "-flto" in manifest.get("compiler_flags", []):
            parser.error("Experimental GCC LTO archives are unsupported by ESP-IDF's -fno-lto final link; use baseline archives")
        for name, expected in manifest["archives"].items():
            if hashlib.sha256((module / name).read_bytes()).hexdigest() != expected:
                parser.error("Static archive hash mismatch: " + name)
        for name, key in [("bank.cob", "source_sha256"), ("bridge.c", "bridge_sha256"),
                          ("static_posix.c", "static_posix_sha256")]:
            if manifest[key] != hashlib.sha256((Path(__file__).parent / name).read_bytes()).hexdigest():
                parser.error("Static module source changed; rebuild: " + name)
        env["NANACOIN_COBOL_STATIC_DIR"] = str(module)
        report["module"] = manifest
    defaults = (BANK / profile.sdkconfig).read_text(encoding="utf-8")
    if args.p4_emulator_rev3:
        # The real board profile remains unchanged. ESP-EMU models v3 silicon,
        # whose SRAM/ROM layout differs from the current board's v1 profile.
        defaults = defaults.replace("CONFIG_ESP32P4_REV_MIN_100=y", "CONFIG_ESP32P4_REV_MIN_301=y")
        defaults = defaults.replace("CONFIG_ESP32P4_SELECTS_REV_LESS_V3=y", "CONFIG_ESP32P4_SELECTS_REV_LESS_V3=n")
    if args.emulator_debug:
        defaults += "\nCONFIG_LOG_DEFAULT_LEVEL_DEBUG=y\nCONFIG_LOG_DEFAULT_LEVEL=4\nCONFIG_LOG_MAXIMUM_LEVEL=4\n"
    if args.emulator_packet_rx:
        for prefix in ("ESP_HOSTED_SDIO_OPTIMIZATION_RX_", "ESP_SDIO_OPTIMIZATION_RX_"):
            defaults += "\nCONFIG_" + prefix + "NONE=y\nCONFIG_" + prefix + "MAX_SIZE=n\nCONFIG_" + prefix + "STREAMING_MODE=n\n"
        for generated in target_dir.glob(profile.target + "/release/build/esp-idf-sys-*/out/sdkconfig"):
            text = generated.read_text()
            for prefix in ("ESP_HOSTED_SDIO_OPTIMIZATION_RX_", "ESP_SDIO_OPTIMIZATION_RX_"):
                for suffix in ("MAX_SIZE", "STREAMING_MODE"):
                    option = "CONFIG_" + prefix + suffix
                    text = text.replace(option + "=y", "# " + option + " is not set")
                option = "CONFIG_" + prefix + "NONE"
                text = text.replace("# " + option + " is not set", option + "=y")
                if option + "=y" not in text:
                    text += "\n" + option + "=y\n"
            if text != generated.read_text():
                generated.write_text(text, encoding="utf-8")
    defaults = defaults.replace('"partitions.csv"', '"' + (BANK / profile.partitions).as_posix() + '"')
    sdkconfig = out / "sdkconfig.defaults"
    if not sdkconfig.exists() or sdkconfig.read_text(encoding="utf-8") != defaults:
        sdkconfig.write_text(defaults, encoding="utf-8")
    if args.p4_emulator_rev3:
        # SDK defaults do not override values in an existing generated config.
        # Update only this explicitly selected experiment's Cargo output files.
        for generated in target_dir.glob(profile.target + "/release/build/esp-idf-sys-*/out/sdkconfig"):
            text = generated.read_text()
            for option in ("ESP32P4_SELECTS_REV_LESS_V3", "ESP32P4_REV_MIN_0", "ESP32P4_REV_MIN_1", "ESP32P4_REV_MIN_100", "ESP32P4_REV_MIN_300"):
                text = text.replace("CONFIG_" + option + "=y", "# CONFIG_" + option + " is not set")
            text = text.replace("# CONFIG_ESP32P4_REV_MIN_301 is not set", "CONFIG_ESP32P4_REV_MIN_301=y")
            if "CONFIG_ESP32P4_REV_MIN_301=y" not in text:
                text += "\nCONFIG_ESP32P4_REV_MIN_301=y\n"
            if text != generated.read_text():
                generated.write_text(text, encoding="utf-8")
    idf = args.esp_root / "frameworks/esp-idf-v5.5.3"
    python = args.esp_root / "python_env/idf5.5_py3.11_env/Scripts/python.exe"
    env.update(CARGO_TARGET_DIR=str(target_dir), NANACOIN_BOARD=args.board, MCU=profile.chip,
               IDF_PATH=str(idf), IDF_TOOLS_PATH=str(args.esp_root), ESP_IDF_TOOLS_INSTALL_DIR="fromenv",
               IDF_PYTHON_ENV_PATH=str(python.parent.parent), ESP_IDF_SDKCONFIG_DEFAULTS=str(sdkconfig),
               ESP_ROM_ELF_DIR=str(args.esp_root / "tools/esp-rom-elfs/20241011"),
               LIBCLANG_PATH=str(args.rust_toolchain / "xtensa-esp32-elf-clang/esp-clang/bin/libclang.dll"),
               RUSTC=str(args.rust_toolchain / "bin/rustc.exe"))
    if args.board == "s2":
        env["NANACOIN_STATUS_LED_PIN"] = env.get("NANACOIN_S2_STATUS_LED_PIN", "15")
    elif args.board == "p4":
        env["NANACOIN_STATUS_LED_PIN"] = "off"
    family = "riscv32-esp-elf" if args.board == "p4" else "xtensa-esp-elf"
    cross = args.esp_root / "tools" / family / "esp-14.2.0_20251107" / family / "bin"
    env["PATH"] = os.pathsep.join(str(p) for p in [cross,
        args.rust_toolchain / "xtensa-esp32-elf-clang/esp-clang/bin",
        idf / "tools", python.parent, args.esp_root / "tools/cmake/3.30.2/bin",
        args.esp_root / "tools/ninja/1.12.1"]) + os.pathsep + env["PATH"]
    features = ["esp32"]
    if args.emulator_clock is not None:
        features.append("emulator-clock")
    if args.board in ("s2", "p4"):
        features.append("board-" + args.board)
    if args.engine == "cobol":
        features.append("cobol-core")
    report_path = out / "report.json"

    def run(stage, command):
        with (out / (stage + ".log")).open("w", encoding="utf-8") as log:
            result = subprocess.run(command, cwd=BANK, env=env, stdout=log, stderr=subprocess.STDOUT)
        report[stage + "_exit_code"] = result.returncode
        report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(stage, "exit", result.returncode, "log", out / (stage + ".log"), flush=True)
        return result.returncode

    if run("cargo", [str(args.rust_toolchain / "bin/cargo.exe"), "build", "--locked", "--release",
                     "--no-default-features", "--features", ",".join(features), "--bin", "nanacoin-esp32",
                     "--target", profile.target, "-Z", "build-std=std,panic_abort"]):
        raise SystemExit(1)
    elf = target_dir / profile.target / "release/nanacoin-esp32"
    report.update(server_firmware_link_verified=True, elf_sha256=hashlib.sha256(elf.read_bytes()).hexdigest())
    # This is offline conversion plus the existing board-marker/partition gate.
    image_exit = run("image", [str(python), str(BANK / "scripts/firmware-image.py"), args.board, str(elf)])
    image = elf.with_suffix(".bin")
    if image.is_file():
        report.update(app_image_bytes=image.stat().st_size, app_partition_bytes=profile.app_size(),
                      app_partition_fits=image.stat().st_size <= profile.app_size(),
                      image_sha256=hashlib.sha256(image.read_bytes()).hexdigest())
    prefix = "riscv32-esp-elf" if args.board == "p4" else "xtensa-" + profile.chip + "-elf"
    run("sections", [str(cross / (prefix + "-size.exe")), "-A", str(elf)])
    if args.board == "p4":
        # esp_idf_size's generic P4 table does not describe both revision
        # layouts correctly. Check actual HP section addresses against IDF
        # 5.5.3's soc.h physical limits, rather than its reported free space.
        physical_start, physical_end = 0x4FF00000, 0x4FFC0000
        regions = []
        for line in (out / "sections.log").read_text().splitlines():
            parts = line.split()
            if len(parts) == 3 and parts[0].startswith("."):
                size, address = int(parts[1]), int(parts[2])
                if size and physical_start <= address < physical_end:
                    regions.append({"section": parts[0], "start": address, "end": address + size})
        report["p4_physical_static_sram"] = {
            "physical_start": physical_start, "physical_end": physical_end,
            "allocated_sections": regions,
            "allocated_bytes": sum(r["end"] - r["start"] for r in regions),
            "all_section_ends_within_physical_sram": bool(regions) and all(r["end"] <= physical_end for r in regions),
            "runtime_free_heap_verified": False,
            "source": str(idf / "components/soc/esp32p4/include/soc/soc.h"),
        }
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("Full server firmware evidence:", report_path)
    raise SystemExit(image_exit)


if __name__ == "__main__":
    main()
