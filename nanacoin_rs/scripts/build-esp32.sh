#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Usage: bash scripts/build-esp32.sh s3|s2 [cargo args...]
# There is no default board: each bank has its own target, sdkconfig,
# partitions, certificate, web bundle and Cargo target directory.
[[ ${1:-} == s3 || ${1:-} == s2 ]] || { echo 'Usage: bash scripts/build-esp32.sh s3|s2' >&2; exit 2; }
board=$1
shift
field() { python scripts/boards.py "$board" "$1"; }
target=$(field target)
export NANACOIN_BOARD=$board
features=esp32
if [[ $board == s2 ]]; then
  features=esp32,board-s2
  # The S2 Mini has one plain LED on GPIO15, not a WS2812 pixel; never
  # inherit the S3's GPIO48 setting.
  export NANACOIN_STATUS_LED_PIN=${NANACOIN_S2_STATUS_LED_PIN:-15}
fi
echo "Building NanaCoin firmware for board $board ($(field name)) at $(field hostname)"
# Credentials may come from the environment or, failing that, from a
# gitignored .env / config.py that build.rs discovers. Only fail here when
# neither source can supply them.
if [[ -z "${NANACOIN_WIFI_SSID:-}" || -z "${NANACOIN_WIFI_PASSWORD:-}" ]]; then
  have_file=
  for candidate in .env config.py ../.env ../nanacoin_web/config.py; do
    if [[ -f "$candidate" ]] && grep -qE '^[[:space:]]*(export[[:space:]]+)?(NANACOIN_)?WIFI_PASSWORD[[:space:]]*=' "$candidate"; then
      have_file=$candidate; break
    fi
  done
  if [[ -z "$have_file" ]]; then
    echo 'Set NANACOIN_WIFI_SSID and NANACOIN_WIFI_PASSWORD, or put WIFI_SSID/WIFI_PASSWORD in a gitignored .env or config.py' >&2
    exit 1
  fi
  echo "Wi-Fi credentials: $have_file (override by exporting NANACOIN_WIFI_*)"
fi
bash scripts/build-web.sh
# Git Bash support for the existing official Windows ESP-IDF installation.
# Elsewhere, source your ESP-IDF and espup export scripts before this script.
if [[ -d /c/Espressif/frameworks/esp-idf-v5.5.3 ]]; then
  # esp-idf-sys rejects long Windows output paths before running CMake.
  # Each board keeps its own directory so their IDF builds never mix.
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$(field target_dir)}"
  export IDF_PATH="C:/Espressif/frameworks/esp-idf-v5.5.3"
  export IDF_TOOLS_PATH="C:/Espressif"
  export ESP_IDF_TOOLS_INSTALL_DIR=fromenv
  export IDF_PYTHON_ENV_PATH="C:/Espressif/python_env/idf5.5_py3.11_env"
  export ESP_ROM_ELF_DIR="C:/Espressif/tools/esp-rom-elfs/20241011"
  export PATH="/c/Espressif/frameworks/esp-idf-v5.5.3/tools:/c/Espressif/python_env/idf5.5_py3.11_env/Scripts:/c/Espressif/tools/cmake/3.30.2/bin:/c/Espressif/tools/ninja/1.12.1:/c/Espressif/tools/xtensa-esp-elf/esp-14.2.0_20251107/xtensa-esp-elf/bin:$PATH"
  export LIBCLANG_PATH="$(cygpath -m "$USERPROFILE")/.rustup/toolchains/esp/xtensa-esp32-elf-clang/esp-clang/bin/libclang.dll"
  # Keep the compiler selected by this ESP-IDF installation ahead of Rustup's
  # bundled GCC (which may be newer than the version this IDF release accepts).
  export PATH="/c/Espressif/tools/xtensa-esp-elf/esp-14.2.0_20251107/xtensa-esp-elf/bin:$(cygpath -u "$USERPROFILE")/.rustup/toolchains/esp/xtensa-esp32-elf-clang/esp-clang/bin:$(cygpath -u "$USERPROFILE")/.rustup/toolchains/esp/xtensa-esp-elf/bin:$PATH"
fi
# esp-idf-sys generates a CMake project in its output directory; a relative
# partition CSV would resolve there rather than beside this Cargo.toml.
mkdir -p .embuild
python - "$board" <<'PY'
import sys
sys.path.insert(0, 'scripts')
from boards import ROOT, board
b = board(sys.argv[1])
defaults = (ROOT / b.sdkconfig).read_text()
defaults = defaults.replace('"partitions.csv"', '"' + (ROOT / b.partitions).as_posix() + '"')
destination = ROOT / f'.embuild/board-{b.id}.defaults'
if not destination.exists() or destination.read_text() != defaults:
    destination.write_text(defaults)
PY
export ESP_IDF_SDKCONFIG_DEFAULTS="$(pwd)/.embuild/board-$board.defaults"
export MCU=$(field chip)
if command -v cygpath >/dev/null 2>&1; then
  export ESP_IDF_SDKCONFIG_DEFAULTS="$(cygpath -m "$ESP_IDF_SDKCONFIG_DEFAULTS")"
fi
if [[ -d /c/Espressif/frameworks/esp-idf-v5.5.3 ]]; then
  # Calling the Rustup proxy adds its newer GCC directory ahead of PATH in
  # Cargo build-script environments. Use the same installed Rust toolchain
  # directly so esp-idf-sys inherits the IDF-supported GCC from PATH.
  esp_toolchain="$(cygpath -u "$USERPROFILE")/.rustup/toolchains/esp/bin"
  export RUSTC="$(cygpath -m "$esp_toolchain/rustc.exe")"
  "$esp_toolchain/cargo.exe" build --locked --release --no-default-features --features "$features" \
    --bin nanacoin-esp32 --target "$target" -Z build-std=std,panic_abort "$@"
else
  cargo +esp build --locked --release --no-default-features --features "$features" \
    --bin nanacoin-esp32 --target "$target" -Z build-std=std,panic_abort "$@"
fi
esp_python="${NANACOIN_ESPTOOL_PYTHON:-python}"
if [[ -z "${NANACOIN_ESPTOOL_PYTHON:-}" && -f /c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe ]]; then
  esp_python=/c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe
fi
"$esp_python" scripts/firmware-image.py "$board" "${CARGO_TARGET_DIR:-target}/$target/release/nanacoin-esp32"
