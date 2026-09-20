# Build the Angular client, compress it, and push it to the board.
#
#   .\deploy.ps1 -Port COM4
#   .\deploy.ps1 -Port COM4 -SkipBuild     # push what is already in dist/
#
# The board's port moves on every reset - it has no bridge chip - so find it
# first rather than assuming:
#
#   [System.IO.Ports.SerialPort]::GetPortNames()
#
# See BOARD_SKILL_ESP32_S2_MINI.md.

param(
    [Parameter(Mandatory = $true)]
    [string]$Port,

    [switch]$SkipBuild,

    # Skip the reset, e.g. when copying files to inspect from the REPL.
    [switch]$NoReset
)

$ErrorActionPreference = 'Stop'

$AngularDir = Join-Path $PSScriptRoot '..\nanacoin\angular'

# Angular 22 puts the actual site under dist/<project>/browser/, alongside
# build metadata (3rdpartylicenses.txt, prerendered-routes.json) that is not
# part of the site. Publishing the parent would put index.html at
# /www/browser/index.html, where static.py does not look for it, and ship 18KB
# of licence text to a board with 4MB of flash.
$DistRoot = Join-Path $AngularDir 'dist\nanacoin-web'
$DistDir = Join-Path $DistRoot 'browser'

function Find-Python {
    $candidates = @()
    $current = (Get-Command python -ErrorAction SilentlyContinue).Source
    if ($current) { $candidates += $current }
    $candidates += 'C:\Users\matth\AppData\Local\Programs\Python\Python312\python.exe'
    $candidates += 'C:\Espressif\python_env\idf5.5_py3.11_env\Scripts\python.exe'

    foreach ($py in $candidates) {
        if (-not (Test-Path $py)) { continue }
        & $py -m mpremote --version *> $null
        if ($LASTEXITCODE -eq 0) { return $py }
    }
    return $null
}

$PY = Find-Python
if (-not $PY) {
    Write-Host "Could not find a Python with mpremote installed." -ForegroundColor Red
    Write-Host "  Install it with:  python -m pip install mpremote esptool"
    exit 1
}

# --- build ------------------------------------------------------------------

if (-not $SkipBuild) {
    Write-Host "building the Angular client..." -ForegroundColor Cyan
    Push-Location $AngularDir
    try {
        & npm run build
        if ($LASTEXITCODE -ne 0) { throw "ng build failed" }
    }
    finally { Pop-Location }
}

if (-not (Test-Path $DistDir)) {
    # Older Angular layouts put the site straight in dist/<project>.
    if (Test-Path (Join-Path $DistRoot 'index.html')) {
        $DistDir = $DistRoot
    }
    else {
        Write-Host "No build at $DistDir" -ForegroundColor Red
        Write-Host "  Run without -SkipBuild, or build it by hand first."
        exit 1
    }
}

if (-not (Test-Path (Join-Path $DistDir 'index.html'))) {
    Write-Host "No index.html in $DistDir" -ForegroundColor Red
    Write-Host "  The build looks incomplete; check 'npm run build' output."
    exit 1
}

# --- compress ---------------------------------------------------------------
#
# Gzipping happens here, on a PC with a spare CPU, rather than on a board with
# 4MB of flash and one core. The board only ever picks the .gz file when the
# browser says it accepts it - it never compresses anything at runtime.
#
# Only text compresses usefully. A .png or .woff2 is already compressed, and a
# .gz of one is usually larger than the original while costing flash twice.

$stage = Join-Path $env:TEMP "nanacoin-web-stage"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null

$compressible = @('.html', '.js', '.css', '.json', '.svg', '.txt')

$originalBytes = 0
$shippedBytes = 0
$files = @()

foreach ($f in Get-ChildItem $DistDir -Recurse -File) {
    $rel = $f.FullName.Substring($DistDir.Length).TrimStart('\', '/')
    $originalBytes += $f.Length

    # Source maps are for debugging and are the largest thing in the build.
    # The board has 4MB; they do not earn their place.
    if ($rel -like '*.map') { continue }

    $ext = [System.IO.Path]::GetExtension($f.Name).ToLower()
    $target = Join-Path $stage $rel
    New-Item -ItemType Directory -Path (Split-Path $target) -Force | Out-Null

    if ($compressible -contains $ext) {
        $gzTarget = "$target.gz"
        $in = [System.IO.File]::OpenRead($f.FullName)
        $out = [System.IO.File]::Create($gzTarget)
        $gz = New-Object System.IO.Compression.GZipStream($out, [System.IO.Compression.CompressionLevel]::Optimal)
        try { $in.CopyTo($gz) } finally { $gz.Dispose(); $out.Dispose(); $in.Dispose() }

        # Keep only the smaller of the two. A tiny file can gzip larger than
        # it started, and shipping both wastes the flash twice over.
        $gzLen = (Get-Item $gzTarget).Length
        if ($gzLen -lt $f.Length) {
            $shippedBytes += $gzLen
            $files += [pscustomobject]@{ Rel = "$rel.gz"; Local = $gzTarget; Size = $gzLen }
        }
        else {
            Remove-Item $gzTarget
            Copy-Item $f.FullName $target
            $shippedBytes += $f.Length
            $files += [pscustomobject]@{ Rel = $rel; Local = $target; Size = $f.Length }
        }
    }
    else {
        Copy-Item $f.FullName $target
        $shippedBytes += $f.Length
        $files += [pscustomobject]@{ Rel = $rel; Local = $target; Size = $f.Length }
    }
}

$kb = { param($n) "{0:N0} KB" -f ($n / 1KB) }
Write-Host ("build {0} -> shipping {1} in {2} files" -f (& $kb $originalBytes), (& $kb $shippedBytes), $files.Count) -ForegroundColor Cyan

# 4MB of flash, most of it firmware. Refuse rather than half-fill the board.
if ($shippedBytes -gt 1.5MB) {
    Write-Host "That is more than 1.5MB, which is more than this board should hold." -ForegroundColor Red
    Write-Host "  Check for source maps or unoptimised assets in the build."
    exit 1
}

# --- push -------------------------------------------------------------------

Write-Host "checking the board on $Port..." -ForegroundColor Cyan
& $PY -m mpremote connect $Port eval "1+1" *> $null
if ($LASTEXITCODE -ne 0) {
    Write-Host "No MicroPython on $Port." -ForegroundColor Red
    Write-Host "  Find the port:  [System.IO.Ports.SerialPort]::GetPortNames()"
    Write-Host "  The S2's port moves on every reset - see BOARD_SKILL_ESP32_S2_MINI.md"
    exit 1
}

# Wipe /www first. Angular emits hashed filenames, so a rebuild writes new
# names rather than overwriting the old ones - without this the board silently
# accumulates every build it has ever been given until the flash fills.
Write-Host "clearing /www..." -ForegroundColor Cyan
& $PY -m mpremote connect $Port exec @'
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
rm("/www")
try: os.mkdir("/www")
except OSError: pass
'@
if ($LASTEXITCODE -ne 0) { Write-Host "could not clear /www" -ForegroundColor Red; exit 1 }

Write-Host "copying the site..." -ForegroundColor Cyan
$dirsMade = @{}
foreach ($f in $files) {
    $relDir = Split-Path $f.Rel -Parent
    if ($relDir -and -not $dirsMade.ContainsKey($relDir)) {
        $boardDir = "/www/" + ($relDir -replace '\\', '/')
        & $PY -m mpremote connect $Port exec "import os`ntry: os.mkdir('$boardDir')`nexcept OSError: pass" *> $null
        $dirsMade[$relDir] = $true
    }
    $boardPath = "/www/" + ($f.Rel -replace '\\', '/')
    & $PY -m mpremote connect $Port fs cp $f.Local ":$boardPath"
    if ($LASTEXITCODE -ne 0) { Write-Host "  failed: $($f.Rel)" -ForegroundColor Red; exit 1 }
    Write-Host ("  {0,-46} {1,8:N0} B" -f $f.Rel, $f.Size)
}

# The server itself, last, so a half-copied site never runs.
foreach ($py in @('static.py', 'config.py', 'main.py')) {
    $local = Join-Path $PSScriptRoot $py
    if (-not (Test-Path $local)) {
        if ($py -eq 'config.py') {
            Write-Host "config.py missing - copy config_example.py and add your WiFi details" -ForegroundColor Red
            exit 1
        }
        continue
    }
    & $PY -m mpremote connect $Port fs cp $local ":$py"
    if ($LASTEXITCODE -ne 0) { Write-Host "  failed: $py" -ForegroundColor Red; exit 1 }
    Write-Host "  $py"
}

if (-not $NoReset) {
    Write-Host "resetting..." -ForegroundColor Cyan
    & $PY -m mpremote connect $Port reset
}

Write-Host ""
Write-Host "Done. The site should come up at http://nanacoin.local/" -ForegroundColor Green
Write-Host "Point it at the API board once:"
Write-Host "  http://nanacoin.local/?api=<nanacoin-s3-ip>"
Write-Host ""
Write-Host "Watch it boot:  $PY -m mpremote connect $Port repl"
