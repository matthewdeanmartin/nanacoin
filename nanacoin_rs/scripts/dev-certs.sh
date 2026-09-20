#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
umask 077
mkdir -p certs
leaf=certs/nanacoin-ca-signed.crt
key=certs/nanacoin-ca-signed.key
ca=certs/home-ca.crt
if [[ -e $leaf || -e $key || -e $ca ]]; then
  [[ -f $leaf && -f $key && -f $ca ]] || { echo 'Incomplete CA-signed certificate set; refusing to replace it.' >&2; exit 1; }
else
  command -v mkcert >/dev/null || { echo 'Install mkcert first.' >&2; exit 1; }
  mkdir -p .local/ca
  export CAROOT="$(pwd)/.local/ca"
  if command -v cygpath >/dev/null; then CAROOT="$(cygpath -m "$CAROOT")"; export CAROOT; fi
  # No -install: system trust stores are changed only by the owner.
  mkcert -ecdsa -cert-file "$leaf" -key-file "$key" nanacoin.local localhost 127.0.0.1
  cp "$CAROOT/rootCA.pem" "$ca"
fi
openssl verify -CAfile "$ca" -verify_hostname nanacoin.local "$leaf"
openssl x509 -in "$leaf" -noout -checkend 0
openssl x509 -in "$ca" -noout -checkend 0
leaf_public=$(openssl x509 -in "$leaf" -pubkey -noout | openssl pkey -pubin -outform DER | openssl dgst -sha256)
key_public=$(openssl pkey -in "$key" -pubout -outform DER | openssl dgst -sha256)
[[ "$leaf_public" == "$key_public" ]] || { echo 'Server certificate and key do not match.' >&2; exit 1; }
openssl x509 -in "$ca" -outform DER -out certs/home-ca.der
echo 'Compare this CA SHA-256 fingerprint through a trusted channel:'
openssl x509 -in "$ca" -noout -fingerprint -sha256
echo 'CA private key stays in .local/ca, never in the firmware or downloads. Existing self-signed files are preserved.'
