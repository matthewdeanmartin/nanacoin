# Applies the espradio DHCP patch to a local clone, and points go.mod at it.
#
# The patch cannot live in the repo as a vendored copy: espradio ships 37MB of
# WiFi blobs, which is not worth committing for a one-line change. So the
# clone is made on demand into third_party/ (gitignored) and go.mod's replace
# directive points there.
#
# Run once after cloning this repo, before building for the board:
#
#   .\patches\apply.ps1
#
# See patches/README.md for what the patch does and why it is needed.

param(
    # Pin the version the patch was written against. A newer espradio may have
    # made the retry count configurable, in which case the patch is obsolete -
    # see the "Removing this patch" section of patches/README.md.
    [string]$Version = "v0.3.0",
    [string]$Dest = "third_party/espradio",

    # lneto is the network stack under espradio. Its TCP pool builds every
    # connection with loss recovery disabled, which on a marginal link means a
    # lost segment is never retransmitted. See patches/README.md change 3.
    [string]$LnetoVersion = "v0.3.2",
    [string]$LnetoDest = "third_party/lneto"
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$skipEspradio = Test-Path $Dest
if ($skipEspradio) {
    Write-Host "$Dest already exists; skipping espradio (remove it to re-apply)." -ForegroundColor Yellow
}

# Applies one find-and-replace, refusing rather than patching blind if the
# target text is not exactly where it was. An espradio upgrade that moved or
# fixed this code should fail loudly here, not silently produce a different
# binary.
function Edit-Source {
    param([string]$Path, [string]$Old, [string]$New, [string]$What)

    # Normalise line endings on both sides before matching. git may check
    # espradio out with CRLF depending on core.autocrlf, while the patch text
    # in this script is LF, and a mismatch there would look identical to
    # "upstream changed the code".
    $content = (Get-Content $Path -Raw) -replace "`r`n", "`n"
    $old = $Old -replace "`r`n", "`n"
    $new = $New -replace "`r`n", "`n"

    if ($content -notmatch [regex]::Escape($old)) {
        throw "$Path does not contain the expected text for '$What' - espradio may have changed. See patches/README.md."
    }
    Set-Content -Path $Path -Value ($content -replace [regex]::Escape($old), $new) -NoNewline
    Write-Host "  patched: $What" -ForegroundColor Green
}

if (-not $skipEspradio) {

Write-Host "Cloning espradio $Version into $Dest..."
New-Item -ItemType Directory -Force (Split-Path -Parent $Dest) | Out-Null
git clone --depth 1 --branch $Version https://github.com/tinygo-org/espradio.git $Dest
if ($LASTEXITCODE -ne 0) { throw "clone failed" }

Remove-Item -Recurse -Force "$Dest/.git"

Write-Host "Applying patches..."

# 1. DHCP gets more attempts. Three is not enough at ~-81 dBm.
Edit-Source -Path (Join-Path $Dest "espstack.go") `
    -What "DHCP retry budget" `
    -Old "dhcpResults, err := rstack.DoDHCPv4(reqaddr, 3*time.Second, 3)" `
    -New @'
// PATCHED FOR NANACOIN: upstream allows 3 attempts at 3s. This board sits
	// several floors from its access point at about -81 dBm, where the
	// four-packet DHCP exchange reliably loses a packet, and NetConnect
	// cannot be retried because espradio.Enable is a one-shot. More attempts
	// here is the only place the retry can go. See patches/README.md.
	dhcpResults, err := rstack.DoDHCPv4(reqaddr, 5*time.Second, 20)
'@

# 2a. The retry loop below needs strings; add it to the import block.
Edit-Source -Path (Join-Path $Dest "netlink/netlink.go") `
    -What "strings import" `
    -Old @'
import (
	"net"
	"net/netip"
	"sync"
	"time"
'@ `
    -New @'
import (
	"net"
	"net/netip"
	"strings"
	"sync"
	"time"
'@

# 2. The WPA2 handshake gets retried. It fails intermittently at this range
#    and the next attempt usually works.
Edit-Source -Path (Join-Path $Dest "netlink/netlink.go") `
    -What "association retry" `
    -Old @'
err = espradio.Connect(espradio.STAConfig{
		SSID:     params.Ssid,
		Password: params.Passphrase,
	})
	if err != nil {
		if debug {
			println("connect failed:", err)
		}
		return err
	}
'@ `
    -New @'
// PATCHED FOR NANACOIN: retry the association, patiently.
	//
	// espradio.Connect is safely retryable - it sets the STA config and calls
	// esp_wifi_connect_internal, with no one-shot guard - and on a marginal
	// link it fails intermittently two different ways: "4-way handshake
	// timeout" when a handshake frame is lost, and "AP not found" when the
	// access point is momentarily below the scan threshold. Both clear up on
	// their own. NetConnect as a whole cannot be retried by the caller,
	// because Enable above is a one-shot, so the retry has to live here.
	//
	// There is no attempt limit. The access point for this board is several
	// floors away and drops out of range for minutes at a time; a board that
	// gave up would need a power cycle to come back, while one that keeps
	// trying rejoins by itself. A wrong passphrase is reported by the AP as a
	// distinct error and is not retried. See patches/README.md.
	for attempt := 1; ; attempt++ {
		err = espradio.Connect(espradio.STAConfig{
			SSID:     params.Ssid,
			Password: params.Passphrase,
		})
		if err == nil {
			break
		}

		// Only a genuine credential rejection is fatal. Matching loosely on
		// "auth" would catch WIFI_REASON_AUTH_EXPIRE ("auth expired"), which
		// is a transient WPA2 timing failure that the next attempt clears -
		// so the test is against the two reasons that actually mean the
		// passphrase is wrong: WIFI_REASON_AUTH_FAIL (202) and
		// WIFI_REASON_802_1X_AUTH_FAILED (23). Everything else - expired
		// auth, handshake timeouts, the NO_AP_FOUND family - is worth
		// retrying.
		msg := err.Error()
		if strings.Contains(msg, "authentication failed") || strings.Contains(msg, "802.1X") {
			if debug {
				println("association rejected, passphrase is wrong:", err)
			}
			return err
		}

		// Back off to 30s. A distant AP is worth checking twice a minute
		// indefinitely; checking every two seconds forever is just noise on
		// the console.
		wait := time.Duration(attempt) * 2 * time.Second
		if wait > 30*time.Second {
			wait = 30 * time.Second
		}
		println("association attempt", attempt, "failed:", msg, "- retrying in", int(wait.Seconds()), "s")
		time.Sleep(wait)
	}
'@

} # end espradio block

# ---------------------------------------------------------------------------
# 3. lneto: enable TCP loss recovery on pooled connections.
# ---------------------------------------------------------------------------
#
# xnet.NewTCPPool builds each tcp.Conn with no LossRecovery and no Nanotime,
# and tcp.ConnConfig documents that leaving LossRecovery nil disables loss
# recovery outright. The algorithm itself ships in the same module and is
# complete - tcp.RTO, an RFC 6298 estimator with its own tests - it is simply
# never wired up by the pool.
#
# The effect is that a dropped outgoing segment is never retransmitted on a
# timer. The connection waits for an ACK that cannot arrive until something
# else forces a resend, which on a weak link is what "the board just stops
# answering" looks like from a browser. It is the strongest candidate for why
# MicroPython works from the same shelf where this struggles: lwIP has had RTO
# since forever, and this pool has had it available but switched off.

$skipLneto = Test-Path $LnetoDest
if ($skipLneto) {
    Write-Host "$LnetoDest already exists; skipping lneto (remove it to re-apply)." -ForegroundColor Yellow
}

if (-not $skipLneto) {

Write-Host "Cloning lneto $LnetoVersion into $LnetoDest..."
New-Item -ItemType Directory -Force (Split-Path -Parent $LnetoDest) | Out-Null
git clone --depth 1 --branch $LnetoVersion https://github.com/soypat/lneto.git $LnetoDest
if ($LASTEXITCODE -ne 0) { throw "lneto clone failed" }

Remove-Item -Recurse -Force "$LnetoDest/.git"

# Each pooled connection gets its own RTO instance. It must be per-connection:
# RTO holds a shadow of one connection's send sequence space and its own
# timer, so sharing one across the pool would mix unrelated sequence numbers
# and produce nonsense retransmissions.
Edit-Source -Path (Join-Path $LnetoDest "x/xnet/tcppool.go") `
    -What "per-connection RTO storage" `
    -Old @'
	allocPerConn := cfg.TxBufSize + cfg.RxBufSize
	bufSpace := make([]byte, n*allocPerConn)
'@ `
    -New @'
	allocPerConn := cfg.TxBufSize + cfg.RxBufSize
	bufSpace := make([]byte, n*allocPerConn)

	// PATCHED FOR NANACOIN: one RFC 6298 retransmission timer per connection.
	//
	// Allocated as one slice up front, like the buffers above, so the pool
	// keeps its property of allocating everything at construction and nothing
	// per request. An RTO is a small value type holding no pointers.
	rtos := make([]tcp.RTO, n)
'@

# Nanotime is required whenever LossRecovery is set - Configure returns
# ErrInvalidConfig otherwise - and the pool already has a clock for its own
# timeouts, so the same one is reused.
Edit-Source -Path (Join-Path $LnetoDest "x/xnet/tcppool.go") `
    -What "wire LossRecovery into ConnConfig" `
    -Old @'
		conncfg := tcp.ConnConfig{
			RxBuf:             bufSpace[bufoff:txOff],
			TxBuf:             bufSpace[txOff : txOff+cfg.TxBufSize],
			TxPacketQueueSize: cfg.QueueSize,
			Logger:            cfg.ConnLogger,
			RWBackoff:         cfg.NewBackoff(),
		}
'@ `
    -New @'
		conncfg := tcp.ConnConfig{
			RxBuf:             bufSpace[bufoff:txOff],
			TxBuf:             bufSpace[txOff : txOff+cfg.TxBufSize],
			TxPacketQueueSize: cfg.QueueSize,
			Logger:            cfg.ConnLogger,
			RWBackoff:         cfg.NewBackoff(),

			// PATCHED FOR NANACOIN: enable packet-loss recovery.
			//
			// Without these two fields tcp.Conn runs with no retransmission
			// timer at all, so a lost segment stalls the connection until
			// something else happens to force a resend. On a link several
			// floors from the access point that is most segments eventually.
			//
			// Nanotime is mandatory when LossRecovery is set (Configure
			// returns ErrInvalidConfig otherwise). The pool's own clock is
			// reused rather than introducing a second time source, so the
			// retransmission timer and the pool timeouts cannot disagree
			// about what "now" means.
			LossRecovery: &rtos[i],
			Nanotime:     pool.now,
		}
'@

}

go mod edit -replace "tinygo.org/x/espradio=./$Dest"
if (Test-Path $LnetoDest) {
    go mod edit -replace "github.com/soypat/lneto=./$LnetoDest"
}
go mod tidy
Write-Host "go.mod now points at the patched copy." -ForegroundColor Green
Write-Host ""
Write-Host "Build for the board with:"
Write-Host '  tinygo build -target=esp32s3-generic -o nanacoin.bin `'
Write-Host '    -ldflags="-X main.ssid=YourSSID -X main.password=YourPassword" `'
Write-Host '    ./cmd/nanacoin-esp32'
