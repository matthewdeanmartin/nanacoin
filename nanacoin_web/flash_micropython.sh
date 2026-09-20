#!/usr/bin/env bash
# One-time: erase the board and install MicroPython.
#
#   ./flash_micropython.sh              # finds the port itself
#   ./flash_micropython.sh -p COM4      # or name it
#   ./flash_micropython.sh -y           # skip the confirmation
#
# Before running: hold BOOT (0), tap RESET (RST), release BOOT.
#
# This ERASES whatever is on the board.
#
# The Git Bash twin of flash_micropython.ps1. Both exist because the port
# moves constantly on this board and retyping it in the wrong shell is a
# recurring waste of a minute.

set -euo pipefail
cd "$(dirname "$0")"

PORT=""
ASSUME_YES=0

while [ $# -gt 0 ]; do
    case "$1" in
        -p|--port) PORT="$2"; shift 2 ;;
        -y|--yes)  ASSUME_YES=1; shift ;;
        -h|--help) sed -n '2,12p' "$0" | sed 's/^# \?//'; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

red()   { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }
cyan()  { printf '\033[36m%s\033[0m\n' "$*"; }
amber() { printf '\033[33m%s\033[0m\n' "$*"; }

# --- python with esptool ----------------------------------------------------
#
# Invoked as "<python> -m esptool" rather than relying on a bare `esptool` on
# PATH, which varies between a plain shell and an ESP-IDF-activated one.

find_python() {
    local candidates=()
    command -v python >/dev/null 2>&1 && candidates+=("$(command -v python)")
    candidates+=("/c/Users/matth/AppData/Local/Programs/Python/Python312/python.exe")
    candidates+=("/c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe")

    local py
    for py in "${candidates[@]}"; do
        [ -x "$py" ] || continue
        if "$py" -m esptool version >/dev/null 2>&1; then
            printf '%s' "$py"
            return 0
        fi
    done
    return 1
}

PY="$(find_python)" || {
    red "Could not find a Python with esptool installed."
    echo "  python -m pip install esptool mpremote pyserial"
    exit 1
}

# --- firmware ---------------------------------------------------------------

FIRMWARE="$(ls -1 firmware/ESP32_GENERIC_S2-*.bin 2>/dev/null | sort -r | head -1 || true)"
if [ -z "$FIRMWARE" ]; then
    red "No firmware in firmware/"
    echo "  Download from https://micropython.org/download/ESP32_GENERIC_S2/"
    echo "  It must be the ESP32_GENERIC_S2 build - an S3 or SPIRAM_OCT image"
    echo "  flashes 'successfully' and leaves a board that never boots."
    exit 1
fi

# --- port -------------------------------------------------------------------

if [ -z "$PORT" ]; then
    PORT="$(./find_port.sh || true)"
fi
if [ -z "$PORT" ]; then
    red "No board found."
    echo "  Put it in bootloader mode: hold BOOT (0), tap RESET (RST), release BOOT."
    echo "  An S2 with empty flash appears ONLY in bootloader mode - it has no"
    echo "  bridge chip to enumerate on its own."
    exit 1
fi

echo "firmware: $(basename "$FIRMWARE")"
echo "port:     $PORT"
echo
amber "This ERASES the board."

if [ "$ASSUME_YES" -ne 1 ]; then
    read -r -p "Continue? (y/N) " reply
    case "$reply" in
        y|Y) ;;
        *) echo "cancelled."; exit 0 ;;
    esac
fi

# --- erase ------------------------------------------------------------------

echo
cyan "erasing ..."
if ! "$PY" -m esptool --chip esp32s2 --port "$PORT" erase-flash; then
    echo
    red "Erase failed."
    echo "  Is the board in bootloader mode? Hold BOOT, tap RESET, release BOOT."
    echo "  Is $PORT still the port? It changes on every reset."
    exit 1
fi

# erase-flash resets the chip when it finishes, and the flash is now empty, so
# there is no firmware left to bring USB back up. The port usually disappears
# entirely here. Re-detect rather than assuming the one we erased on survived.
echo
amber "Erase complete. The board has reset and the port has probably moved."
echo "  Put it back in bootloader mode: hold BOOT (0), tap RESET (RST), release BOOT."
read -r -p "  Press Enter once you have done that: " _

NEW_PORT="$(./find_port.sh || true)"
if [ -n "$NEW_PORT" ] && [ "$NEW_PORT" != "$PORT" ]; then
    cyan "  port moved: $PORT -> $NEW_PORT"
    PORT="$NEW_PORT"
elif [ -z "$NEW_PORT" ]; then
    echo
    red "No board port found."
    echo "  The flash is empty, so the board appears only in bootloader mode."
    echo "  Retry the BOOT/RESET sequence, then:"
    echo "    $PY -m esptool --chip esp32s2 --port COMn --baud 460800 \\"
    echo "      write-flash -z 0x1000 $FIRMWARE"
    exit 1
fi

# --- write ------------------------------------------------------------------

echo
cyan "writing firmware to $PORT ..."
if ! "$PY" -m esptool --chip esp32s2 --port "$PORT" --baud 460800 \
        write-flash -z 0x1000 "$FIRMWARE"; then
    echo
    red "Flash failed."
    echo "  The flash stays empty until this succeeds, so the board will not"
    echo "  enumerate except in bootloader mode. This is recoverable: redo the"
    echo "  BOOT/RESET sequence and run this script again."
    exit 1
fi

echo
green "Done."
echo "  1. Tap RESET on the board"
echo "  2. ./find_port.sh          (the port will have changed)"
echo "  3. ./deploy.sh             (finds the port itself)"
