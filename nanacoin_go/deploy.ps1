# Build, flash and watch NanaCoin on the ESP32-S3.
#
#   .\deploy.ps1                                      # uses saved credentials
#   .\deploy.ps1 -Ssid "YourWiFi" -Password "pw" -SaveCredentials
#
# Run .\patches\apply.ps1 once first - the board build needs a patched
# espradio and lneto to survive a weak link. See patches/README.md.
#
# # Credentials
#
# The passphrase is linked into the firmware, so a build needs it in the
# clear. Retyping it on every flash is how it ends up pasted into terminals,
# chat logs and scratch files - so -SaveCredentials writes it once to
# wifi.local.json, which .gitignore excludes, and later runs read it from
# there.
#
# That file is plain text on purpose. It protects against the credential
# spreading, which is the failure that actually happens; it does not pretend
# to protect against someone who already has the disk, and DPAPI encryption
# here would only make it look like it does.

param(
    [string]$Ssid,
    [string]$Password,

    # Write -Ssid/-Password to wifi.local.json for later runs.
    [switch]$SaveCredentials,

    # Where saved credentials live. Gitignored.
    [string]$CredentialFile = "wifi.local.json",

    # The CH343 bridge port. Flashing goes here because the bridge enumerates
    # on power regardless of what the S3 is doing and drives BOOT/RESET over
    # RTS/DTR, so esptool enters the bootloader by itself - no button dance.
    [string]$Port = "COM8",

    # Build tags. The diagnostics are the board's largest optional cost:
    # nanacoin_nologs drops the 58-entry event ring (4,640 bytes measured) and
    # with it /logs, which is the most allocation-heavy endpoint in the API at
    # ~43KB and 286 allocations per request. nanacoin_nodiag drops /diag.
    #
    # Left on by default, because a board you cannot ask what it has been
    # doing is a board you debug with a USB cable. Turn them off when the
    # heap matters more than the diagnostics.
    [string]$Tags = "",

    [string]$Target = "esp32s3-generic",
    [int]$WatchSeconds = 90,
    [switch]$NoWatch
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

if (-not (Test-Path "third_party/espradio")) {
    Write-Host "espradio is not patched yet. Run .\patches\apply.ps1 first." -ForegroundColor Yellow
    exit 1
}

if (-not (Test-Path "third_party/lneto")) {
    # Without this the board builds and runs but every pooled TCP connection
    # has loss recovery disabled, so a dropped segment stalls the connection
    # instead of being retransmitted. That failure looks like a hang, not
    # like a missing patch, which is why it is refused here rather than
    # warned about. See patches/README.md change 3.
    Write-Host "lneto is not patched yet. Run .\patches\apply.ps1 first." -ForegroundColor Yellow
    exit 1
}

# Resolve credentials: explicit parameters win, then the saved file.
if ($SaveCredentials) {
    if (-not $Ssid -or -not $Password) {
        throw "-SaveCredentials needs both -Ssid and -Password."
    }
    @{ ssid = $Ssid; password = $Password } |
        ConvertTo-Json | Set-Content -Path $CredentialFile -Encoding UTF8
    Write-Host "Saved credentials to $CredentialFile (gitignored)." -ForegroundColor Green
}

if (-not $Ssid -or -not $Password) {
    if (Test-Path $CredentialFile) {
        $saved = Get-Content $CredentialFile -Raw | ConvertFrom-Json
        if (-not $Ssid) { $Ssid = $saved.ssid }
        if (-not $Password) { $Password = $saved.password }
        Write-Host "Using credentials from $CredentialFile for SSID '$Ssid'." -ForegroundColor Cyan
    }
}

if (-not $Ssid -or -not $Password) {
    Write-Host "No WiFi credentials." -ForegroundColor Yellow
    Write-Host "Save them once with:"
    Write-Host '  .\deploy.ps1 -Ssid "YourWiFi" -Password "YourPassword" -SaveCredentials'
    exit 1
}

$bin = Join-Path $env:TEMP "nanacoin-board.bin"

Write-Host "Building for $Target..." -ForegroundColor Cyan

# The ldflags value contains a space, so it has to reach tinygo as one
# argument. Passing it inline as -ldflags="...$Ssid..." lets PowerShell split
# it at the space and tinygo sees a stray "-X"; building the argument list
# explicitly keeps it intact.
$ldflags = "-X main.ssid=$Ssid -X main.password=$Password"
$buildArgs = @(
    "build",
    "-target=$Target",
    "-o", $bin,
    "-ldflags", $ldflags
)
if ($Tags) {
    $buildArgs += @("-tags", $Tags)
    Write-Host "Build tags: $Tags" -ForegroundColor Cyan
}
$buildArgs += "./cmd/nanacoin-esp32"
& tinygo @buildArgs
if ($LASTEXITCODE -ne 0) { throw "build failed" }

$size = (Get-Item $bin).Length
Write-Host ("Built {0:N0} bytes" -f $size) -ForegroundColor Green

# Offset 0x0, not 0x1000. The S3 bootloader lives at zero; flashing at 0x1000
# reports success, verifies the hash, and leaves a board that never boots.
Write-Host "Flashing over $Port..." -ForegroundColor Cyan
python -m esptool --chip esp32s3 --port $Port --baud 460800 write-flash -z 0x0 $bin
if ($LASTEXITCODE -ne 0) { throw "flash failed" }

if ($NoWatch) {
    Write-Host "Flashed. Skipping serial watch." -ForegroundColor Green
    exit 0
}

# Serial output goes to the NATIVE USB port, not the bridge we just flashed
# over - and that port re-enumerates on every reboot, so it has to be found
# rather than assumed.
Start-Sleep -Seconds 2
$native = Get-CimInstance Win32_PnPEntity |
    Where-Object { $_.Name -match 'USB Serial Device \(COM(\d+)\)' } |
    ForEach-Object { if ($_.Name -match 'COM(\d+)') { "COM$($Matches[1])" } } |
    Select-Object -First 1

if (-not $native) {
    Write-Host "Flashed, but no native USB serial port appeared - nothing to watch." -ForegroundColor Yellow
    Write-Host "The board is probably running; look for it on your network."
    exit 0
}

Write-Host "Watching $native (DHCP on a weak link takes ~45s)..." -ForegroundColor Cyan
Write-Host ""

$serial = New-Object System.IO.Ports.SerialPort $native, 115200, None, 8, one
$serial.DtrEnable = $true
$serial.Open()
try {
    $seen = New-Object System.Text.StringBuilder
    $deadline = (Get-Date).AddSeconds($WatchSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            $chunk = $serial.ReadExisting()
            if ($chunk) {
                [void]$seen.Append($chunk)
                Write-Host -NoNewline $chunk
                # Stop as soon as the board is serving; no reason to make the
                # user wait out the whole window.
                if ($seen.ToString() -match 'listening on http://') { break }
            }
        } catch {}
        Start-Sleep -Milliseconds 250
    }
} finally {
    $serial.Close()
}

Write-Host ""
if ($seen.ToString() -match 'listening on (http://[0-9.]+)') {
    Write-Host "NanaCoin is up at $($Matches[1])" -ForegroundColor Green
    Write-Host "The first request after boot can take 20-40s to connect. That is normal."
} else {
    Write-Host "Did not see the board report an address within ${WatchSeconds}s." -ForegroundColor Yellow
    Write-Host "A 'Retrying DHCP' loop is expected on a weak link - give it longer, or re-run with -WatchSeconds 180."
}
