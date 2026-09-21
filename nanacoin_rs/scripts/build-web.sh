#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
bash nanacoin_rs/scripts/dev-certs.sh
(cd nanacoin_ui && npm run build)
node nanacoin_rs/scripts/bundle-web.mjs
