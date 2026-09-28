#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
# NANACOIN_BOARD (s3 default for desktop bundles, s2) selects the bundle
# directory and the bank's certificate.
bash nanacoin_rs/scripts/dev-certs.sh "${NANACOIN_BOARD:-s3}"
(cd nanacoin_ui && npm run build)
node nanacoin_rs/scripts/bundle-web.mjs
