# One-time: erase the board and install MicroPython.
#
#   1. Hold BOOT (0), tap RESET (RST), release BOOT
#   2. [System.IO.Ports.SerialPort]::GetPortNames()
#   3. .\flash_micropython.ps1 -Port COM4
#   4. Tap RESET
#
# This ERASES whatever is on the board, including the C firmware. The C
# sources are untouched - `idf.py flash` puts that back whenever you want.

param(
    [Parameter(Mandatory = $true)]
    [string]$Port
)

$ErrorActionPreference = 'Stop'

function Find-Python {
    $candidates = @()
    $current = (Get-Command python -ErrorAction SilentlyContinue).Source
    if ($current) { $candidates += $current }
    $candidates += 'C:\Users\matth\AppData\Local\Programs\Python\Python312\python.exe'
    $candidates += 'C:\Espressif\python_env\idf5.5_py3.11_env\Scripts\python.exe'

    foreach ($py in $candidates) {
        if (-not (Test-Path $py)) { continue }
        & $py -m esptool version *> $null
        if ($LASTEXITCODE -eq 0) { return $py }
    }
    return $null
}

$PY = Find-Python
if (-not $PY) {
    Write-Host "Could not find a Python with esptool installed." -ForegroundColor Red
    Write-Host "  Install it with:  python -m pip install mpremote esptool"
    exit 1
}

$firmware = Get-ChildItem "firmware\ESP32_GENERIC_S2-*.bin" -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending | Select-Object -First 1

if (-not $firmware) {
    Write-Host "No firmware found in firmware\" -ForegroundColor Red
    Write-Host "  Download from https://micropython.org/download/ESP32_GENERIC_S2/"
    exit 1
}

Write-Host "firmware: $($firmware.Name)"
Write-Host "port:     $Port"
Write-Host ""
Write-Host "This ERASES the board, including the C firmware." -ForegroundColor Yellow
$reply = Read-Host "Continue? (y/N)"
if ($reply -ne 'y') {
    Write-Host "cancelled."
    exit 0
}

Write-Host "`nerasing ..."
& $PY -m esptool --chip esp32s2 --port $Port erase_flash
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nErase failed." -ForegroundColor Red
    Write-Host "  Is the board in bootloader mode? Hold BOOT, tap RESET, release BOOT."
    Write-Host "  Is $Port the right port? Check GetPortNames() again - it changes."
    exit 1
}

# erase_flash ends by resetting the chip, which makes it re-enumerate - and on
# a now-empty flash there is no firmware to bring USB back up, so the port can
# vanish entirely. Pause for the user to re-enter the bootloader, and re-detect
# the port rather than assuming the one we erased on is still there.
Write-Host ""
Write-Host "Erase complete. The board has reset and the port has probably moved." -ForegroundColor Yellow
Write-Host "  Put it back in bootloader mode: hold BOOT (0), tap RESET (RST), release BOOT."
Read-Host "  Press Enter once you have done that"

$ports = [System.IO.Ports.SerialPort]::GetPortNames() | Where-Object { $_ -ne 'COM3' }
if ($ports -notcontains $Port) {
    if ($ports.Count -eq 1) {
        Write-Host "  port moved: $Port -> $($ports[0])" -ForegroundColor Cyan
        $Port = $ports[0]
    } elseif ($ports.Count -eq 0) {
        Write-Host "`nNo board port found." -ForegroundColor Red
        Write-Host "  The flash is now empty, so the board only appears in bootloader mode."
        Write-Host "  Retry the BOOT/RESET sequence, then run:"
        Write-Host "    python -m esptool --chip esp32s2 --port COMn --baud 460800 ``"
        Write-Host "      write_flash -z 0x1000 $($firmware.Name)"
        exit 1
    } else {
        Write-Host "  several ports found: $($ports -join ', '); using $Port" -ForegroundColor Yellow
    }
}

Write-Host "`nwriting firmware to $Port ..."
& $PY -m esptool --chip esp32s2 --port $Port --baud 460800 `
    write_flash -z 0x1000 $firmware.FullName
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nFlash failed." -ForegroundColor Red
    Write-Host "  The flash is empty until this succeeds, so the board will not"
    Write-Host "  enumerate except in bootloader mode. This is recoverable -"
    Write-Host "  redo BOOT/RESET and run the script again."
    exit 1
}

Write-Host "`nDone." -ForegroundColor Green
Write-Host "  1. Tap RESET on the board"
Write-Host "  2. [System.IO.Ports.SerialPort]::GetPortNames()   (the port will change)"
Write-Host "  3. .\deploy.ps1 -Port COM5                        (use the new port)"
