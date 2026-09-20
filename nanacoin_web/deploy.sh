#!/usr/bin/env bash
# Build the Angular client, compress it, and push it to the board.
#
#   ./deploy.sh                 # build, then deploy to whichever port is found
#   ./deploy.sh -p COM4         # name the port
#   ./deploy.sh -s              # skip the build, push what is in dist/
#   ./deploy.sh -n              # skip the reset, to poke about in the REPL
#   ./deploy.sh -a 192.168.1.158   # bake in where the API board lives
#
# The Git Bash twin of deploy.ps1.

set -euo pipefail
cd "$(dirname "$0")"

PORT=""
SKIP_BUILD=0
NO_RESET=0
API=""

while [ $# -gt 0 ]; do
    case "$1" in
        -p|--port)       PORT="$2"; shift 2 ;;
        -s|--skip-build) SKIP_BUILD=1; shift ;;
        -n|--no-reset)   NO_RESET=1; shift ;;
        -a|--api)        API="$2"; shift 2 ;;
        -h|--help) sed -n '2,9p' "$0" | sed 's/^# \?//'; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

red()   { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }
cyan()  { printf '\033[36m%s\033[0m\n' "$*"; }

ANGULAR_DIR="../nanacoin/angular"

# Angular 22 puts the actual site under dist/<project>/browser/, alongside
# build metadata (3rdpartylicenses.txt, prerendered-routes.json) that is not
# part of the site. Publishing the parent would put index.html at
# /www/browser/index.html, where static.py does not look for it, and ship
# 18KB of licence text to a board with 4MB of flash.
DIST_ROOT="$ANGULAR_DIR/dist/nanacoin-web"
DIST_DIR="$DIST_ROOT/browser"

find_python() {
    local candidates=()
    command -v python >/dev/null 2>&1 && candidates+=("$(command -v python)")
    candidates+=("/c/Users/matth/AppData/Local/Programs/Python/Python312/python.exe")
    candidates+=("/c/Espressif/python_env/idf5.5_py3.11_env/Scripts/python.exe")

    local py
    for py in "${candidates[@]}"; do
        [ -x "$py" ] || continue
        if "$py" -m mpremote --version >/dev/null 2>&1; then
            printf '%s' "$py"
            return 0
        fi
    done
    return 1
}

PY="$(find_python)" || {
    red "Could not find a Python with mpremote installed."
    echo "  python -m pip install mpremote esptool pyserial"
    exit 1
}

# --- build ------------------------------------------------------------------

if [ "$SKIP_BUILD" -ne 1 ]; then
    cyan "building the Angular client..."
    ( cd "$ANGULAR_DIR" && npm run build )
fi

if [ ! -d "$DIST_DIR" ]; then
    # Older Angular layouts put the site straight in dist/<project>.
    if [ -f "$DIST_ROOT/index.html" ]; then
        DIST_DIR="$DIST_ROOT"
    else
        red "No build at $DIST_DIR"
        echo "  Run without -s, or build it by hand first."
        exit 1
    fi
fi

if [ ! -f "$DIST_DIR/index.html" ]; then
    red "No index.html in $DIST_DIR"
    echo "  The build looks incomplete; check 'npm run build' output."
    exit 1
fi

# --- compress ---------------------------------------------------------------
#
# Gzipping happens here, on a PC with a spare CPU, rather than on a board with
# 4MB of flash and one core. The board never compresses anything at runtime -
# it serves the .gz when the browser says it accepts it.
#
# Only text compresses usefully. A .png or .woff2 is already compressed and a
# .gz of one is usually larger, costing flash twice.

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

MANIFEST="$STAGE/.manifest"
: > "$MANIFEST"

original_bytes=0
shipped_bytes=0

size_of() { stat -c %s "$1"; }

# One manifest row: a published path and its size, tab separated. A function
# rather than an inline printf so the format string lives in exactly one place.
record_to() { printf '%s\t%s\n' "$2" "$3" >> "$1"; }
record() { record_to "$MANIFEST" "$1" "$2"; }

while IFS= read -r -d '' file; do
    rel="${file#"$DIST_DIR"/}"
    bytes="$(size_of "$file")"
    original_bytes=$(( original_bytes + bytes ))

    # Source maps are the largest thing in the build and exist for debugging.
    # On 4MB of flash they do not earn their place.
    case "$rel" in *.map) continue ;; esac

    target="$STAGE/$rel"
    mkdir -p "$(dirname "$target")"

    case "${rel##*.}" in
        html|js|css|json|svg|txt)
            gzip -9 -c "$file" > "$target.gz"
            gz_bytes="$(size_of "$target.gz")"

            # index.html ships BOTH ways, which is the one exception to
            # "smaller copy only".
            #
            # It is the only file a client that refuses gzip ever asks for:
            # assets are fetched by a browser that has already parsed this
            # HTML, and every browser sends Accept-Encoding: gzip. Without a
            # plain copy, such a client makes the board inflate a file into
            # RAM to answer - which works, but is real work for a 240MHz core
            # with 2MB of heap. A ~1.7KB second copy makes that path
            # effectively dead code, kept only as a correctness backstop.
            if [ "$(basename "$rel")" = "index.html" ]; then
                cp "$file" "$target"
                shipped_bytes=$(( shipped_bytes + bytes + gz_bytes ))
                record "$rel" "$bytes"
                record "$rel.gz" "$gz_bytes"
                continue
            fi

            # Everything else: ship whichever is smaller. A tiny file can gzip
            # larger than it started, and shipping both wastes the flash twice.
            if [ "$gz_bytes" -lt "$bytes" ]; then
                shipped_bytes=$(( shipped_bytes + gz_bytes ))
                printf '%s\t%s\n' "$rel.gz" "$gz_bytes" >> "$MANIFEST"
            else
                rm -f "$target.gz"
                cp "$file" "$target"
                shipped_bytes=$(( shipped_bytes + bytes ))
                printf '%s\t%s\n' "$rel" "$bytes" >> "$MANIFEST"
            fi
            ;;
        *)
            cp "$file" "$target"
            shipped_bytes=$(( shipped_bytes + bytes ))
            printf '%s\t%s\n' "$rel" "$bytes" >> "$MANIFEST"
            ;;
    esac
done < <(find "$DIST_DIR" -type f -print0)

# Bake the API address into the page.
#
# This board serves files and has no API. Without an address the client falls
# back to its own origin, asks THIS board for /api/v1/status, and gets
# index.html back from the single-page fallback - which fails as a JSON parse
# error rather than as anything a person could act on. index.html carries a
# <meta name="nanacoin-api"> for exactly this, and it ships empty by default.
#
# Done on the staged copy, so the Angular build output is never modified.
if [ -n "$API" ]; then
    staged_index="$STAGE/index.html"
    if [ -f "$staged_index" ]; then
        python stamp_api.py "$staged_index" "$API" || exit 1
        # The gzipped copy was made before the stamp, so redo it.
        if [ -f "$STAGE/index.html.gz" ]; then
            gzip -9 -c "$staged_index" > "$STAGE/index.html.gz"
            # The manifest's recorded sizes are stale now; rewrite both rows.
            new_plain="$(size_of "$staged_index")"
            new_gz="$(size_of "$STAGE/index.html.gz")"
            # Drop the two index rows, keeping every other row untouched.
            # Field-wise rather than a regex over the line: the path is the
            # first tab-separated field, so matching it exactly is clearer
            # and immune to a name that merely starts with "index.html".
            awk -F'\t' '$1 != "index.html" && $1 != "index.html.gz"' "$MANIFEST" > "$MANIFEST.tmp"
            record_to "$MANIFEST.tmp" "index.html" "$new_plain"
            record_to "$MANIFEST.tmp" "index.html.gz" "$new_gz"
            mv "$MANIFEST.tmp" "$MANIFEST"
        fi
    fi
fi

file_count="$(wc -l < "$MANIFEST" | tr -d ' ')"
cyan "build $(( original_bytes / 1024 )) KB -> shipping $(( shipped_bytes / 1024 )) KB in $file_count files"

# 4MB of flash, most of it already firmware. Refuse rather than half-fill it.
if [ "$shipped_bytes" -gt 1572864 ]; then
    red "That is more than 1.5MB, which is more than this board should hold."
    echo "  Check for source maps or unoptimised assets in the build."
    exit 1
fi

# --- port -------------------------------------------------------------------

if [ -z "$PORT" ]; then
    PORT="$(./find_port.sh || true)"
fi
if [ -z "$PORT" ]; then
    red "No board found."
    echo "  Tap RESET, then try again. The port moves on every reset."
    exit 1
fi

# Every mpremote call gets a deadline.
#
# Without one, a board sitting in main.py's serve loop never answers: the
# infinite accept() blocks the REPL, mpremote waits forever, and the process
# wedges in a driver read that survives taskkill. The port is then unusable
# until the board is physically unplugged. A timeout turns that from a stuck
# machine into an error message.
mp() {
    timeout "${MP_TIMEOUT:-25}" "$PY" -m mpremote connect "$PORT" "$@"
}

cyan "checking the board on $PORT..."
if ! mp eval "1+1" >/dev/null 2>&1; then
    red "No MicroPython answering on $PORT."
    echo
    echo "  The usual cause is the board running its serve loop, which blocks"
    echo "  the REPL. Put it in a state mpremote can reach:"
    echo "    hold BOOT (0), tap RESET (RST), release BOOT"
    echo
    echo "  If the port is wedged from an earlier attempt, unplug the board and"
    echo "  plug it back in - a hung serial read cannot be killed from here."
    echo
    echo "  No files were changed; the site already on the board is untouched."
    exit 1
fi

# --- push -------------------------------------------------------------------
#
# Copy into /www.new, then swap it into place at the end.
#
# The obvious order - wipe /www, then copy - takes the site down for the whole
# transfer and leaves it down if anything fails partway. That is exactly what
# happened: a copy failed on a wedged serial port after the wipe had already
# run, and the board served 404s until it could be reached again. Staging
# means a failed deploy changes nothing the browser can see.
#
# The wipe is still needed somewhere, because Angular emits hashed filenames:
# a rebuild writes new names rather than overwriting old ones, so without
# clearing, the board accumulates every build it has ever been given.

cyan "staging into /www.new..."
mp exec '
import os
def rm(d):
    try: entries = os.listdir(d)
    except OSError: return
    for e in entries:
        p = d + "/" + e
        try:
            if os.stat(p)[0] & 0x4000: rm(p); os.rmdir(p)
            else: os.remove(p)
        except OSError as err: print("could not remove", p, err)
# Only the staging directory is touched here. /www is left alone until the
# swap at the end, so the site keeps serving throughout.
rm("/www.new")
try: os.mkdir("/www.new")
except OSError: pass
' || { red "could not prepare /www.new"; echo "  The site on the board is untouched."; exit 1; }

# config.py is checked before anything is copied, not after: finding it
# missing halfway through leaves a board with a new site and no credentials.
if [ ! -f config.py ]; then
    red "config.py missing - copy config_example.py and add your WiFi details"
    echo "  Nothing was changed on the board."
    exit 1
fi

cyan "copying the site..."
made_dirs=""
while IFS=$'\t' read -r rel bytes; do
    reldir="$(dirname "$rel")"
    if [ "$reldir" != "." ]; then
        case " $made_dirs " in
            *" $reldir "*) ;;
            *)
                mp exec "import os
try: os.mkdir('/www.new/$reldir')
except OSError: pass" >/dev/null 2>&1
                made_dirs="$made_dirs $reldir"
                ;;
        esac
    fi
    if ! mp fs cp "$STAGE/$rel" ":/www.new/$rel"; then
        red "failed copying $rel"
        echo "  The site already on the board is untouched - /www was not modified."
        echo "  Unplug and replug the board, then run this again."
        exit 1
    fi
    printf '  %-46s %8s B\n' "$rel" "$bytes"
done < "$MANIFEST"

# --- swap --------------------------------------------------------------------
#
# Everything is on the board now. Replacing /www is the only destructive step
# and it happens once, at the end, after every byte has landed.

cyan "swapping /www.new into place..."
mp exec '
import os
def rm(d):
    try: entries = os.listdir(d)
    except OSError: return
    for e in entries:
        p = d + "/" + e
        try:
            if os.stat(p)[0] & 0x4000: rm(p); os.rmdir(p)
            else: os.remove(p)
        except OSError as err: print("could not remove", p, err)

# MicroPython has os.rename, but renaming onto a non-empty directory fails,
# so the old tree goes first. The window where neither exists is the few
# milliseconds between these two calls, rather than the whole transfer.
rm("/www")
try: os.rmdir("/www")
except OSError: pass
try:
    os.rename("/www.new", "/www")
except OSError as err:
    print("SWAP FAILED", err)
    raise
' || { red "the swap failed - the new site is in /www.new on the board"; exit 1; }

# The server code last, so a half-copied site never runs.
for f in static.py config.py main.py; do
    [ -f "$f" ] || continue
    if ! mp fs cp "$f" ":$f"; then
        red "failed copying $f"
        exit 1
    fi
    echo "  $f"
done

if [ "$NO_RESET" -ne 1 ]; then
    cyan "resetting..."
    mp reset || true
fi

echo
green "Done. The site should come up at http://nanacoin.local/"
echo "Point it at the API board once:"
echo "  http://nanacoin.local/?api=<nanacoin-s3-ip>"
echo
echo "Watch it boot:  $PY -m mpremote connect $PORT repl"
