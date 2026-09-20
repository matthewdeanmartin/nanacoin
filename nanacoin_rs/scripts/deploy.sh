#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ -n "${1:-}" ]] || { echo 'Usage: bash scripts/deploy.sh COM9 [--dry-run]' >&2; exit 2; }
port=$1
shift
[[ $# == 0 || ( $# == 1 && $1 == --dry-run ) ]] || { echo 'Only --dry-run is accepted; no erase/recovery flags.' >&2; exit 2; }
if [[ -d /c/Espressif/frameworks/esp-idf-v5.5.3 ]]; then
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-C:/ncr}"
fi
bash scripts/build-esp32.sh
esp_python="${NANACOIN_ESPTOOL_PYTHON:-python}"
if [[ -z "${NANACOIN_ESPTOOL_PYTHON:-}" && -f /c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe ]]; then
  esp_python=/c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe
fi
"$esp_python" scripts/deploy.py --port "$port" \
  --image "${CARGO_TARGET_DIR:-target}/xtensa-esp32s3-espidf/release/nanacoin-esp32.bin" "$@"
