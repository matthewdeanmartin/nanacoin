#!/usr/bin/env bash
# Usage: bash scripts/dev-certs.sh [s3|s2]   (s3 when omitted)
# One household CA signs a separate server leaf per bank:
#   s3: certs/nanacoin-ca-signed.{crt,key}     for nanacoin.local
#   s2: certs/nanacoin-s2-ca-signed.{crt,key}  for nanacoin-s2.local
# Existing files are never replaced. The s2 leaf is only ever signed by the
# existing CA, so household devices that trust the S3 also trust the S2.
#
# Which CA: NanaCoin's own by default (.local/ca, created on first use). A
# household that already has a CA (for example mastomini's) can sign with
# that instead: set NANACOIN_CA_DIR, or run `make adopt-ca CA_DIR=...`, which
# remembers the choice in .local/ca-dir. The directory holds rootCA.pem and
# rootCA-key.pem; only another tool's existing CA is used, never created.
set -euo pipefail
cd "$(dirname "$0")/.."
board=${1:-s3}
[[ $board == s3 || $board == s2 ]] || { echo 'Usage: bash scripts/dev-certs.sh [s3|s2]' >&2; exit 2; }
umask 077
mkdir -p certs
ca=certs/home-ca.crt
default_ca_dir=.local/ca
if [[ -n ${NANACOIN_CA_DIR:-} ]]; then
  ca_dir=$NANACOIN_CA_DIR
elif [[ -f .local/ca-dir ]]; then
  ca_dir=$(<.local/ca-dir)
else
  ca_dir=$default_ca_dir
fi
root_key=$ca_dir/rootCA-key.pem
root_cert=$ca_dir/rootCA.pem

# Sign a 100-year RSA server leaf for one hostname with the household CA.
sign_leaf() {
  local hostname=$1 leaf=$2 key=$3 csr extensions serial
  # Scratch files stay in this repository, never in another tool's CA folder.
  mkdir -p .local/csr
  csr=.local/csr/$hostname.csr
  extensions=.local/csr/$hostname.ext
  MSYS2_ARG_CONV_EXCL='*' openssl req -new -newkey rsa:2048 -sha256 -nodes -keyout "$key" -out "$csr" \
    -subj "/O=NanaCoin household/OU=Private home server/CN=$hostname"
  cat >"$extensions" <<EOF
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=DNS:$hostname,DNS:localhost,IP:127.0.0.1
subjectKeyIdentifier=hash
authorityKeyIdentifier=keyid,issuer
EOF
  serial=$(openssl rand -hex 16)
  openssl x509 -req -in "$csr" -CA "$root_cert" -CAkey "$root_key" \
    -set_serial "0x$serial" -days 36525 -sha256 -extfile "$extensions" -out "$leaf"
  rm -f "$csr" "$extensions"
}

if [[ $board == s3 ]]; then
  leaf=certs/nanacoin-ca-signed.crt
  key=certs/nanacoin-ca-signed.key
  if [[ -e $leaf || -e $key || -e $ca ]]; then
    [[ -f $leaf && -f $key && -f $ca ]] || { echo 'Incomplete CA-signed certificate set; refusing to replace it.' >&2; exit 1; }
  elif [[ -f $root_cert && -f $root_key ]]; then
    # An existing household CA (NanaCoin's own, or an adopted one).
    command -v openssl >/dev/null || { echo 'Install OpenSSL first.' >&2; exit 1; }
    sign_leaf nanacoin.local "$leaf" "$key"
    cp "$root_cert" "$ca"
    echo "Signed a new nanacoin.local leaf with the CA in $ca_dir."
  elif [[ $ca_dir != "$default_ca_dir" ]]; then
    echo "No CA in $ca_dir (rootCA.pem and rootCA-key.pem); refusing to create one there." >&2
    exit 1
  else
    command -v openssl >/dev/null || { echo 'Install OpenSSL first.' >&2; exit 1; }
    mkdir -p .local/ca
    # RSA gives the ESP-IDF ECDHE-RSA server and supported browsers the same
    # operation. 36,525 days spans a full century including the usual leap-day
    # average; this is a private household trust anchor, not a public Web PKI
    # certificate.
    # Git Bash otherwise rewrites a leading /O= X.509 subject as a Windows path.
    MSYS2_ARG_CONV_EXCL='*' openssl req -x509 -newkey rsa:3072 -sha256 -days 36525 -nodes \
      -keyout "$root_key" -out "$root_cert" \
      -subj '/O=NanaCoin household/OU=Private home CA/CN=NanaCoin Home CA' \
      -addext 'basicConstraints=critical,CA:TRUE,pathlen:0' \
      -addext 'keyUsage=critical,keyCertSign,cRLSign' \
      -addext 'subjectKeyIdentifier=hash'
    sign_leaf nanacoin.local "$leaf" "$key"
    cp "$root_cert" "$ca"
  fi
else
  leaf=certs/nanacoin-s2-ca-signed.crt
  key=certs/nanacoin-s2-ca-signed.key
  if [[ -e $leaf || -e $key ]]; then
    [[ -f $leaf && -f $key ]] || { echo 'Incomplete S2 certificate pair; refusing to replace it.' >&2; exit 1; }
  else
    # Never mint a second CA for the second bank.
    [[ -f $ca && -f $root_key && -f $root_cert ]] || {
      echo "The S2 leaf must be signed by the existing household CA, but certs/home-ca.crt or $ca_dir is missing." >&2
      echo 'Restore the household CA; do not generate a new one for the S2.' >&2
      exit 1
    }
    cmp -s "$ca" "$root_cert" || { echo "certs/home-ca.crt does not match $root_cert; refusing to sign." >&2; exit 1; }
    command -v openssl >/dev/null || { echo 'Install OpenSSL first.' >&2; exit 1; }
    sign_leaf nanacoin-s2.local "$leaf" "$key"
    echo 'Signed a new nanacoin-s2.local leaf with the existing household CA.'
  fi
fi
bash scripts/test-certs.sh "$board"
openssl x509 -in "$ca" -outform DER -out certs/home-ca.der
echo 'Compare this CA SHA-256 fingerprint through a trusted channel:'
openssl x509 -in "$ca" -noout -fingerprint -sha256
echo "CA private key stays in $ca_dir, never in the firmware or downloads. Existing certificate files are preserved."
