# Household HTTP and HTTPS

The ESP32 serves the same app/API on HTTP port 80 and HTTPS port 443 in **Easy
mode**, the default for a missing transport-policy key. Nothing needs installing
on the children's iPads: bookmark `http://nanacoin.local/`. This is a deliberate
plaintext mode, not protection against someone intercepting the home network.
Passwords, bearer tokens and transactions can be observed or altered in transit.
HTTPS on one person's device does not protect accounts used over HTTP elsewhere.

## Prepare devices, then require HTTPS

1. Visit `http://nanacoin.local/trust`. `/ca` downloads the public root certificate
   as `NanaCoin-Home-CA.crt` (DER), on either listener, without authentication.
2. Verify the SHA-256 fingerprint against Nana's build console or another trusted
   channel. The HTTP page and its download cannot authenticate one another.
3. Install/trust the CA using the page's iPad/iPhone or Android instructions.
   On iOS, installing a profile and enabling **full trust** are separate steps.
4. Open `https://nanacoin.local/?api=` and sign in. The empty API override selects
   the board itself rather than a previously saved server. Test every device.
5. Nana opens Household → Household connection security → Check household mode,
   confirms every device is ready, and selects **Require HTTPS for the household**.

Only Nana can enable this, and only through the TLS listener. It commits a
one-byte policy and revokes all sessions and pending authorization codes under
the ledger mutex. Everyone signs in again. If saving fails, the running process
fails closed and reports a storage error; after restart, the last committed
policy is authoritative. Check the mode again over HTTPS after a lost response.
The switch does not undo earlier exposure: change passwords if compromise is
suspected. It is not a defense against a compromised client or stolen password.

In Secure mode HTTP serves only `/`, `/trust` (instructions), and `/ca` (public
certificate). All other HTTP paths are refused, including login, diagnostics,
provisioning and Angular assets. API enforcement is repeated while holding the
service lock, so a queued HTTP write cannot sneak through after the switch.
There is no remote downgrade endpoint, automatic HTTPS-to-HTTP discovery from
the HTTPS page, or HSTS policy that would prevent deliberate Easy-mode use.
Do not use a certificate-warning bypass as a substitute for establishing trust.
After upgrading, use the HTTPS bookmark directly. Refusing HTTP on the real
board cannot stop a network attacker from impersonating an HTTP site or setup
page; never enter credentials into an old HTTP bookmark in Secure mode.

## Certificates and deployment

`scripts/dev-certs.sh` uses installed `mkcert` to create a dedicated CA in ignored
`.local/ca`, and a CA-signed server certificate in ignored
`certs/nanacoin-ca-signed.crt` / `.key`. It does **not** run `mkcert -install` or
change the computer trust store. Existing self-signed files are preserved.
Existing generated certificates are checked, not silently replaced. Renewal
must keep the same CA if household devices should keep their existing trust.

The server's private key is necessarily embedded for TLS. The **CA private key
`rootCA-key.pem` is never bundled, served, or flashed**. Only `home-ca.der` and the
fingerprinted trust instructions are added to the static bundle. Keep the CA
private key on the trusted build computer. Trusting a root grants broader
certificate authority than just NanaCoin, even though this CA is dedicated to it.

`bash scripts/deploy.sh COM9` builds certificates, UI, and firmware, then validates
the existing partition layout and writes only the application partition.
It preserves the economy and transport policy. The certificate change means
clients trusting only the previous self-signed leaf must install the new CA.

## Explicit USB recovery (only if locked out)

These commands re-enable Easy mode without resetting the economy. They require
physical USB access and perform two application deployments. Run from
`nanacoin_rs`, substituting the actual serial port:

```bash
NANACOIN_RECOVER_HTTP=1 bash scripts/deploy.sh COM9
NANACOIN_RECOVER_HTTP=0 bash scripts/deploy.sh COM9
```

The first build clears only the transport policy during startup. Let it boot
and verify that HTTP works before the second command. The second restores normal
firmware. **Do not leave the recovery build installed:** it enables HTTP on every
boot. No ledger erase, backup, restore, or reprovision is part of this procedure.

## Storage and memory

`src/bin/esp32/journal.rs` stores `ncmeta` → `https_only` as a one-byte NVS blob:
absent or `0` means Easy, `1` means Secure; invalid contents fail startup.
It is separate from journal/checkpoint banks and survives closing books and
economy reset. `src/journal/file.rs` uses an atomically replaced `.transport`
companion for desktop tests; the desktop development listener is HTTP-only.

`src/bin/esp32.rs` runs two bounded core-1 servers: four TLS sockets and two HTTP
sockets, each with a 24 KiB stack. Each worker lazily allocates and reuses one
512 KiB response buffer; static/trust requests bypass it and stream borrowed
flash bytes in 2 KiB chunks. Both servers share the existing locked service;
the policy adds only booleans, not a per-client table. lwIP's socket ceiling is
16 to accommodate both listeners and their control sockets. TLS/SDK allocations
remain dynamic; dual-listener internal-heap headroom needs hardware measurement.

References: [mkcert](https://github.com/FiloSottile/mkcert),
[Apple certificate trust](https://support.apple.com/en-ae/102390),
[Android certificates](https://support.google.com/android/answer/9654714?hl=en).
