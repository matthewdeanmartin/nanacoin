#!/usr/bin/env bash
# Usage: bash scripts/provision.sh s3|s2 PORT [--dry-run]
# FIRST INSTALLATION ONLY: erases the whole chip. Refuses a chip that already
# holds any NanaCoin bank. Routine upgrades use scripts/deploy.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
usage='Usage: bash scripts/provision.sh s3|s2 PORT [--dry-run]'
[[ ${1:-} == s3 || ${1:-} == s2 ]] || { echo "$usage" >&2; exit 2; }
board=$1
[[ -n "${2:-}" ]] || { echo "$usage" >&2; exit 2; }
port=$2
shift 2
[[ $# == 0 || ( $# == 1 && $1 == --dry-run ) ]] || { echo 'Only --dry-run is accepted.' >&2; exit 2; }
if [[ -d /c/Espressif/frameworks/esp-idf-v5.5.3 ]]; then
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$(python scripts/boards.py "$board" target_dir)}"
fi
bash scripts/build-esp32.sh "$board"
release="${CARGO_TARGET_DIR:-target}/$(python scripts/boards.py "$board" target)/release"
# The IDF bootloader and partition table from this board's most recent build.
idf=$(ls -td "$release"/build/esp-idf-sys-*/out/build 2>/dev/null | head -1)
[[ -n $idf && -f $idf/bootloader/bootloader.bin && -f $idf/partition_table/partition-table.bin ]] || {
  echo "Could not find the $board bootloader/partition table under $release/build" >&2; exit 1; }
esp_python="${NANACOIN_ESPTOOL_PYTHON:-python}"
if [[ -z "${NANACOIN_ESPTOOL_PYTHON:-}" && -f /c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe ]]; then
  esp_python=/c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe
fi
"$esp_python" scripts/provision.py --board "$board" --port "$port" \
  --image "$release/nanacoin-esp32.bin" \
  --bootloader "$idf/bootloader/bootloader.bin" \
  --partition-table "$idf/partition_table/partition-table.bin" "$@"
