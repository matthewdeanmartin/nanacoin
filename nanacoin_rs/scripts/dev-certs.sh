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
  command -v openssl >/dev/null || { echo 'Install OpenSSL first.' >&2; exit 1; }
  mkdir -p .local/ca
  root_key=.local/ca/rootCA-key.pem
  root_cert=.local/ca/rootCA.pem
  csr=.local/ca/nanacoin.csr
  extensions=.local/ca/nanacoin.ext

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
  MSYS2_ARG_CONV_EXCL='*' openssl req -new -newkey rsa:2048 -sha256 -nodes -keyout "$key" -out "$csr" \
    -subj '/O=NanaCoin household/OU=Private home server/CN=nanacoin.local'
  cat >"$extensions" <<'EOF'
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=DNS:nanacoin.local,DNS:localhost,IP:127.0.0.1
subjectKeyIdentifier=hash
authorityKeyIdentifier=keyid,issuer
EOF
  serial=$(openssl rand -hex 16)
  openssl x509 -req -in "$csr" -CA "$root_cert" -CAkey "$root_key" \
    -set_serial "0x$serial" -days 36525 -sha256 -extfile "$extensions" -out "$leaf"
  cp "$root_cert" "$ca"
  rm -f "$csr" "$extensions"
fi
bash scripts/test-certs.sh
openssl x509 -in "$ca" -outform DER -out certs/home-ca.der
echo 'Compare this CA SHA-256 fingerprint through a trusted channel:'
openssl x509 -in "$ca" -noout -fingerprint -sha256
echo 'CA private key stays in .local/ca, never in the firmware or downloads. Existing self-signed files are preserved.'
