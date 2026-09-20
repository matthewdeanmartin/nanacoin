#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
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
bash scripts/dev-certs.sh
# Git Bash support for the existing official Windows ESP-IDF installation.
# Elsewhere, source your ESP-IDF and espup export scripts before this script.
if [[ -d /c/Espressif/frameworks/esp-idf-v5.5.3 ]]; then
  # esp-idf-sys rejects long Windows output paths before running CMake.
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-C:/ncr}"
  export IDF_PATH="C:/Espressif/frameworks/esp-idf-v5.5.3"
  export IDF_TOOLS_PATH="C:/Espressif"
  export ESP_IDF_TOOLS_INSTALL_DIR=fromenv
  export IDF_PYTHON_ENV_PATH="C:/Espressif/python_env/idf5.5_py3.11_env"
  export ESP_ROM_ELF_DIR="C:/Espressif/tools/esp-rom-elfs/20241011"
  export PATH="/c/Espressif/frameworks/esp-idf-v5.5.3/tools:/c/Espressif/python_env/idf5.5_py3.11_env/Scripts:/c/Espressif/tools/cmake/3.30.2/bin:/c/Espressif/tools/ninja/1.12.1:/c/Espressif/tools/xtensa-esp-elf/esp-14.2.0_20251107/xtensa-esp-elf/bin:$PATH"
  export LIBCLANG_PATH="$(cygpath -m "$USERPROFILE")/.rustup/toolchains/esp/xtensa-esp32-elf-clang/esp-clang/bin/libclang.dll"
  export PATH="$(cygpath -u "$USERPROFILE")/.rustup/toolchains/esp/xtensa-esp32-elf-clang/esp-clang/bin:$(cygpath -u "$USERPROFILE")/.rustup/toolchains/esp/xtensa-esp-elf/bin:$PATH"
fi
# esp-idf-sys generates a CMake project in its output directory; a relative
# partition CSV would resolve there rather than beside this Cargo.toml.
mkdir -p .embuild
python - <<'PY'
from pathlib import Path
root = Path.cwd()
defaults = (root / 'sdkconfig.defaults').read_text()
defaults = defaults.replace('"partitions.csv"', '"' + (root / 'partitions.csv').as_posix() + '"')
destination = root / '.embuild/board.defaults'
if not destination.exists() or destination.read_text() != defaults:
    destination.write_text(defaults)
PY
export ESP_IDF_SDKCONFIG_DEFAULTS="$(pwd)/.embuild/board.defaults"
if command -v cygpath >/dev/null 2>&1; then
  export ESP_IDF_SDKCONFIG_DEFAULTS="$(cygpath -m "$ESP_IDF_SDKCONFIG_DEFAULTS")"
fi
cargo +esp build --locked --release --no-default-features --features esp32 \
  --bin nanacoin-esp32 --target xtensa-esp32s3-espidf -Z build-std=std,panic_abort "$@"
esp_python="${NANACOIN_ESPTOOL_PYTHON:-python}"
if [[ -z "${NANACOIN_ESPTOOL_PYTHON:-}" && -f /c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe ]]; then
  esp_python=/c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe
fi
"$esp_python" scripts/firmware-image.py "${CARGO_TARGET_DIR:-target}/xtensa-esp32s3-espidf/release/nanacoin-esp32"
