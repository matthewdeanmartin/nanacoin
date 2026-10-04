# Local patches to espradio and lneto

Clones of `tinygo.org/x/espradio` v0.3.0 and `github.com/soypat/lneto` v0.3.2
with three changes, wired in via `replace` directives in `../go.mod` and
applied by `apply.ps1`.

All three do the same thing at three different layers: **make the networking
survive a link that loses packets.** Upstream's defaults are reasonable for a
board sitting near its access point and wrong for one several floors away.

## Change 1: DHCP attempts

`espstack.go`, in `Stack.SetupWithDHCP`:

```go
-dhcpResults, err := rstack.DoDHCPv4(reqaddr, 3*time.Second, 3)
+dhcpResults, err := rstack.DoDHCPv4(reqaddr, 5*time.Second, 20)
```

## Change 2: association attempts

`netlink/netlink.go`, in `Esplink.NetConnect`: wrap the `espradio.Connect`
call in a 10-attempt loop with a 2s gap.

Without it, a transient failure kills the boot even though the very next
attempt usually succeeds. Three show up on this link:

- `espradio: 4-way handshake timeout` - the WPA2 handshake lost a frame
- `espradio: AP not found` - the AP was below the scan threshold this second
- `espradio: auth expired` - `WIFI_REASON_AUTH_EXPIRE`, a WPA2 timing failure

`espradio.Connect` is safely retryable: it sets the STA config and calls
`esp_wifi_connect_internal`, with no one-shot guard.

The retry is unlimited, because an AP that is out of range for minutes should
not require a power cycle. Only a real credential rejection stops it, matched
on `authentication failed` (`WIFI_REASON_AUTH_FAIL`) and `802.1X` - **not** on
a substring like `auth`, which would catch `auth expired` and make a transient
failure fatal again.

## Change 3: TCP loss recovery (lneto)

`x/xnet/tcppool.go`, in `NewTCPPool`: give each pooled connection an RFC 6298
retransmission timer.

```go
+rtos := make([]tcp.RTO, n)
 for i := range pool.conns {
     conncfg := tcp.ConnConfig{
         ...
+        LossRecovery: &rtos[i],
+        Nanotime:     pool.now,
     }
```

This is the most consequential of the three.

`tcp.ConnConfig` documents that leaving `LossRecovery` nil **disables loss
recovery**, and `NewTCPPool` never sets it. So every connection this board
serves runs with no retransmission timer: a dropped outgoing segment is never
resent on a timeout, and the connection waits for an ACK that cannot arrive
until something else happens to force a resend.

From a browser that looks exactly like the board hanging. It is the leading
candidate for why MicroPython works from the same shelf where this struggled -
lwIP has had an RTO since forever, and this pool has had one *available but
switched off*.

The algorithm is not written here. `tcp.RTO` ships in lneto itself, complete,
with its own unit tests and its own RFC citations; it is simply never wired
into the pool. The patch is four lines.

One instance per connection, not one shared: `RTO` holds a shadow of a single
connection's send sequence space and its own timer, so sharing would mix
unrelated sequence numbers. `Nanotime` is mandatory whenever `LossRecovery` is
set (`Configure` returns `ErrInvalidConfig` otherwise) and reuses the pool's
existing clock, so the retransmission timer and the pool timeouts cannot
disagree about what "now" means.

Verified two ways: lneto's own `./tcp/...` and `./x/xnet/...` suites pass with
the patch applied, and `internal/boardhttp/lossrecovery_test.go` fails if the
patch is missing from the vendored tree.

## Why

On the ESP32-S3-N16R8 used here, the access point is several floors away and
reads about -81 dBm. The board associates cleanly and the passphrase is
correct, but the four-packet DHCP exchange loses a packet often enough that
three attempts are not enough. Observed behaviour before the patch:

```
connecting to WiFi: Fios-Martin
Retrying DHCP
Retrying DHCP
WiFi failed: cywnet: retries exceeded
```

With 20 attempts it connects, typically after 14-15 tries:

```
connecting to WiFi: Fios-Martin
Retrying DHCP          (x15)
connected, address 192.168.1.158
```

The same board also fails intermittently one step earlier, at the WPA2
handshake:

```
connecting to WiFi: Fios-Martin
FATAL: WiFi: espradio: 4-way handshake timeout
```

MicroPython connects from the same shelf to the same AP without trouble, so
these are retry budgets that are too small for a marginal link rather than a
link that cannot work.

## Change 3: TCP loss recovery (lneto)

`x/xnet/tcppool.go`, in `NewTCPPool`: give each pooled connection an RFC 6298
retransmission timer.

```go
+rtos := make([]tcp.RTO, n)
 for i := range pool.conns {
     conncfg := tcp.ConnConfig{
         ...
+        LossRecovery: &rtos[i],
+        Nanotime:     pool.now,
     }
```

This is the most consequential of the three.

`tcp.ConnConfig` documents that leaving `LossRecovery` nil **disables loss
recovery**, and `NewTCPPool` never sets it. So every connection this board
serves runs with no retransmission timer: a dropped outgoing segment is never
resent on a timeout, and the connection waits for an ACK that cannot arrive
until something else happens to force a resend.

From a browser that looks exactly like the board hanging. It is the leading
candidate for why MicroPython works from the same shelf where this struggled -
lwIP has had an RTO since forever, and this pool has had one *available but
switched off*.

The algorithm is not written here. `tcp.RTO` ships in lneto itself, complete,
with its own unit tests and its own RFC citations; it is simply never wired
into the pool. The patch is four lines.

One instance per connection, not one shared: `RTO` holds a shadow of a single
connection's send sequence space and its own timer, so sharing would mix
unrelated sequence numbers. `Nanotime` is mandatory whenever `LossRecovery` is
set (`Configure` returns `ErrInvalidConfig` otherwise) and reuses the pool's
existing clock, so the retransmission timer and the pool timeouts cannot
disagree about what "now" means.

Verified two ways: lneto's own `./tcp/...` and `./x/xnet/...` suites pass with
the patch applied, and `internal/boardhttp/lossrecovery_test.go` fails if the
patch is missing from the vendored tree.

## Why they have to be patched here

Neither retry count is configurable. `DHCPConfig` exposes only
`RequestedAddr`, and the count is a literal argument inside
`SetupWithDHCP`.

Retrying at the caller is not possible either: `netlink.Esplink.NetConnect`
is the only public route to a usable `net/http` stack, and it can succeed at
most once. Its first act is `espradio.Enable`, which is a deliberate one-shot
guarded by an atomic with no teardown path, so a second `NetConnect` returns
`ErrAlreadyEnabled` and returns *before* building its stack - leaving
`Esplink.netstack` nil and any later `Addr()` call a nil dereference. Building
the stack by hand does not help, because `Esplink`'s `netstack` and `berkeley`
fields are unexported with no setter, and `net/http` needs the netdev socket
layer that only `NetConnect` installs.

## Upstream issues worth filing

1. **DHCP retry count should be configurable** - add a field to `DHCPConfig`,
   or derive the budget from signal strength. This is the actual fix; the
   patch here is a workaround.

2. **A failed `NetConnect` leaves an unusable, unretryable `Esplink`.**
   Whatever the retry policy, the second call returning `ErrAlreadyEnabled`
   early and leaving `netstack` nil turns a recoverable network problem into a
   panic. Either `NetConnect` should treat `ErrAlreadyEnabled` as benign and
   continue, or it should be documented as strictly single-shot and return a
   clearer error on reuse.

Also noticed while debugging, unrelated to DHCP:

3. **`Scan` requires `Enable` first**, but its doc comment on `Start` says
   `Enable` is "separate from `Start` to allow ... scanning without starting
   the driver", which reads as though `Scan` works after `Enable` only. In
   practice `Scan` before `Enable` fails with "wifi not initialized
   (driver was not installed by esp_wifi_init)".

## Removing this patch

When upstream makes these retry budgets configurable, delete `third_party/`
and the `replace` line in `go.mod`, then set the counts through config.

`apply.ps1` refuses to patch if either target line has changed, rather than
applying blind - so an espradio upgrade that moves or fixes this code fails
loudly instead of silently producing a different binary.
