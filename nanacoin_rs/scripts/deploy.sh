#!/usr/bin/env bash
# Usage: bash scripts/deploy.sh s3|s2|p4 PORT [--dry-run]
# Application-only upgrade of an existing bank. The board is required and is
# checked against the chip, its MAC and its partition table before writing.
set -euo pipefail
cd "$(dirname "$0")/.."
usage='Usage: bash scripts/deploy.sh s3|s2|p4 PORT [--dry-run]'
[[ ${1:-} == s3 || ${1:-} == s2 || ${1:-} == p4 ]] || { echo "$usage" >&2; echo 'The board (s3 = nanacoin.local, s2 = nanacoin-s2.local) is required.' >&2; exit 2; }
board=$1
[[ -n "${2:-}" ]] || { echo "$usage" >&2; exit 2; }
port=$2
shift 2
[[ $# == 0 || ( $# == 1 && $1 == --dry-run ) ]] || { echo 'Only --dry-run is accepted; no erase/recovery flags.' >&2; exit 2; }
if [[ -d /c/Espressif/frameworks/esp-idf-v5.5.3 ]]; then
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$(python scripts/boards.py "$board" target_dir)}"
fi
bash scripts/build-esp32.sh "$board"
esp_python="${NANACOIN_ESPTOOL_PYTHON:-python}"
if [[ -z "${NANACOIN_ESPTOOL_PYTHON:-}" && -f /c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe ]]; then
  esp_python=/c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe
fi
"$esp_python" scripts/deploy.py --board "$board" --port "$port" \
  --image "${CARGO_TARGET_DIR:-target}/$(python scripts/boards.py "$board" target)/release/nanacoin-esp32.bin" "$@"
