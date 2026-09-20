#!/usr/bin/env bash
# Explicit, recoverable household CA rotation. Nothing is deleted: the old CA,
# leaf and private keys move under ignored .local/cert-backups.
set -euo pipefail
cd "$(dirname "$0")/.."
[[ ${1:-} == --yes ]] || { echo 'Usage: bash scripts/rotate-certs.sh --yes' >&2; exit 2; }

command -v openssl >/dev/null || { echo 'Install OpenSSL first.' >&2; exit 1; }

stamp=$(date -u +%Y%m%dT%H%M%SZ)
backup=".local/cert-backups/$stamp"
[[ ! -e $backup ]] || { echo "Certificate backup already exists: $backup" >&2; exit 1; }
mkdir -p "$backup/certs"

for file in home-ca.crt home-ca.der nanacoin-ca-signed.crt nanacoin-ca-signed.key; do
  [[ ! -e "certs/$file" ]] || mv "certs/$file" "$backup/certs/$file"
done
if [[ -d .local/ca ]]; then mv .local/ca "$backup/ca"; fi

if ! bash scripts/dev-certs.sh; then
  echo "New certificate generation failed. Previous material is preserved at $backup" >&2
  exit 1
fi

echo "Previous certificate material archived at $backup"
echo 'Every client must install the new CA from http://nanacoin.local/trust after deployment.'
