#!/usr/bin/env bash
# Usage: bash scripts/adopt-ca.sh <ca-dir> --yes
#
# Sign NanaCoin's server certificates with a household CA that already exists
# (for example mastomini's: ../../mastomini/mastomini_rs/.local/ca), so every
# household device trusts one CA. Optional: by default NanaCoin uses its own
# CA, and nobody needs any other tool's.
#
# <ca-dir> holds rootCA.pem and rootCA-key.pem. The key never leaves it: it is
# not copied into this repository or the firmware. The choice is remembered
# in .local/ca-dir, so later `make certs` signs with the same CA.
#
# Nothing is deleted: the current CA certificate and both banks' leaves move
# under ignored .local/cert-backups/<time>. NanaCoin's own CA stays in
# .local/ca; `make adopt-ca CA_DIR=.local/ca` goes back to it.
#
# Afterwards a board serves the new certificate only once it is deployed, and
# devices must trust the adopted CA (http://nanacoin.local/trust serves it).
set -euo pipefail
cd "$(dirname "$0")/.."
[[ $# -eq 2 && $2 == --yes ]] || { echo 'Usage: bash scripts/adopt-ca.sh <ca-dir> --yes' >&2; exit 2; }
command -v openssl >/dev/null || { echo 'Install OpenSSL first.' >&2; exit 1; }

ca_dir=$(cd "$1" && pwd)
root_cert=$ca_dir/rootCA.pem
root_key=$ca_dir/rootCA-key.pem
[[ -f $root_cert && -f $root_key ]] || { echo "No CA in $ca_dir (rootCA.pem and rootCA-key.pem)." >&2; exit 1; }
openssl x509 -in "$root_cert" -noout -text | grep -q 'CA:TRUE' || { echo "$root_cert is not a CA certificate." >&2; exit 1; }
# The key must be the certificate's.
cert_public=$(openssl x509 -in "$root_cert" -pubkey -noout | openssl pkey -pubin -outform DER | openssl dgst -sha256)
key_public=$(openssl pkey -in "$root_key" -pubout -outform DER | openssl dgst -sha256)
[[ $cert_public == "$key_public" ]] || { echo "$root_key does not belong to $root_cert." >&2; exit 1; }

if [[ -f certs/home-ca.crt ]] && cmp -s certs/home-ca.crt "$root_cert"; then
  echo "NanaCoin already uses the CA in $ca_dir; nothing to do."
  exit 0
fi

stamp=$(date -u +%Y%m%dT%H%M%SZ)
backup=".local/cert-backups/$stamp"
[[ ! -e $backup ]] || { echo "Certificate backup already exists: $backup" >&2; exit 1; }
mkdir -p "$backup/certs"
had_s2=false
[[ -e certs/nanacoin-s2-ca-signed.crt ]] && had_s2=true
for file in home-ca.crt home-ca.der nanacoin-ca-signed.crt nanacoin-ca-signed.key \
  nanacoin-s2-ca-signed.crt nanacoin-s2-ca-signed.key; do
  [[ ! -e "certs/$file" ]] || mv "certs/$file" "$backup/certs/$file"
done
[[ ! -e .local/ca-dir ]] || cp .local/ca-dir "$backup/ca-dir"

restore() {
  echo "Signing failed; restoring the previous certificates from $backup" >&2
  rm -f certs/home-ca.crt certs/home-ca.der certs/nanacoin-ca-signed.* certs/nanacoin-s2-ca-signed.*
  cp "$backup"/certs/* certs/ 2>/dev/null || true
  if [[ -e $backup/ca-dir ]]; then cp "$backup/ca-dir" .local/ca-dir; else rm -f .local/ca-dir; fi
}
printf '%s\n' "$ca_dir" >.local/ca-dir
if ! { bash scripts/dev-certs.sh s3 && { ! $had_s2 || bash scripts/dev-certs.sh s2; }; }; then
  restore
  exit 1
fi

echo "Previous certificates archived at $backup"
echo "NanaCoin now signs with the CA in $ca_dir."
echo 'Deploy each board to serve its new certificate; devices must trust this CA.'
