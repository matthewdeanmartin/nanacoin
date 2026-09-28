#!/usr/bin/env bash
# Offline certificate gate for every firmware build. In particular, do not
# regress to an EC server key: Firefox/NSS reported SEC_ERROR_INVALID_KEY for
# that appliance certificate even though OpenSSL could complete a handshake.
# Usage: bash scripts/test-certs.sh [s3|s2]   (s3 when omitted)
set -euo pipefail
cd "$(dirname "$0")/.."

board=${1:-s3}
case $board in
  s3) leaf=certs/nanacoin-ca-signed.crt; key=certs/nanacoin-ca-signed.key; hostname=nanacoin.local ;;
  s2) leaf=certs/nanacoin-s2-ca-signed.crt; key=certs/nanacoin-s2-ca-signed.key; hostname=nanacoin-s2.local ;;
  *) echo 'Usage: bash scripts/test-certs.sh [s3|s2]' >&2; exit 2 ;;
esac
ca=certs/home-ca.crt
for file in "$leaf" "$key" "$ca"; do
  [[ -f $file ]] || { echo "Missing certificate input: $file" >&2; exit 1; }
done

openssl verify -CAfile "$ca" -verify_hostname "$hostname" "$leaf"
openssl x509 -in "$leaf" -noout -checkend 0
openssl x509 -in "$ca" -noout -checkend 0
# A one-year tolerance keeps this stable after generation while proving these
# are century certificates rather than mkcert's former two/ten-year defaults.
minimum_lifetime=$((99 * 365 * 24 * 60 * 60))
openssl x509 -in "$leaf" -noout -checkend "$minimum_lifetime" || { echo 'Server certificate has less than 99 years remaining.' >&2; exit 1; }
openssl x509 -in "$ca" -noout -checkend "$minimum_lifetime" || { echo 'Household CA has less than 99 years remaining.' >&2; exit 1; }
openssl pkey -in "$key" -check -noout

leaf_public=$(openssl x509 -in "$leaf" -pubkey -noout | openssl pkey -pubin -outform DER | openssl dgst -sha256)
key_public=$(openssl pkey -in "$key" -pubout -outform DER | openssl dgst -sha256)
[[ "$leaf_public" == "$key_public" ]] || { echo 'Server certificate and key do not match.' >&2; exit 1; }

leaf_text=$(openssl x509 -in "$leaf" -noout -text)
ca_text=$(openssl x509 -in "$ca" -noout -text)
if ! grep -q 'Public Key Algorithm: rsaEncryption' <<<"$leaf_text"; then
  echo 'Server certificate must use RSA for ESP-IDF browser compatibility.' >&2
  echo 'Run: make rotate-certs  (clients must then trust the new CA)' >&2
  exit 1
fi
if ! grep -q 'Public Key Algorithm: rsaEncryption' <<<"$ca_text"; then
  echo 'Household CA must use RSA for browser compatibility.' >&2
  echo 'Run: make rotate-certs  (clients must then trust the new CA)' >&2
  exit 1
fi
grep -q 'TLS Web Server Authentication' <<<"$leaf_text" || { echo 'Server certificate lacks TLS server usage.' >&2; exit 1; }
grep -q "DNS:$hostname\b" <<<"$leaf_text" || { echo "Server certificate lacks $hostname SAN." >&2; exit 1; }
# A bank must never present the other bank's name.
for other in nanacoin.local nanacoin-s2.local; do
  [[ $other == "$hostname" ]] && continue
  if grep -q "DNS:$other\b" <<<"$leaf_text"; then
    echo "Server certificate for $hostname also names $other; refusing." >&2
    exit 1
  fi
done
grep -q 'CA:TRUE' <<<"$ca_text" || { echo 'Household certificate is not a CA.' >&2; exit 1; }
echo "Certificate checks passed for $board: 100-year RSA CA/server pair, matching key, $hostname and TLS usages."
