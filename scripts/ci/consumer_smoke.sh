#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Catches: npm artifacts that work via repository aliases but fail in a real consumer.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD
WORK=$(mktemp -d /tmp/nitro-consumer.XXXXXX)
trap 'rm -rf "$WORK"' EXIT
export NITRO_CONSUMER_OUT="${NITRO_CONSUMER_OUT:-$ROOT/out/consumer-smoke}"
mkdir -p "$NITRO_CONSUMER_OUT" "$WORK/app/public/fixtures" "$WORK/packs"
node scripts/ci/npm_pack_check.mjs "$NITRO_CONSUMER_OUT"
(cd packages/nitro-xml && npm pack --pack-destination "$WORK/packs" --json > "$WORK/core-pack.json")
(cd packages/nitro-xml-react && npm pack --pack-destination "$WORK/packs" --json > "$WORK/react-pack.json")
cp examples/consumer-smoke/* "$WORK/app/"
node scripts/ci/consumer_cases.mjs "$WORK/app" "$NITRO_CONSUMER_OUT"
(cd "$WORK/app" && npm install --no-fund --no-audit "$WORK/packs/nitro-xml-0.1.0.tgz" "$WORK/packs/nitro-xml-react-0.1.0.tgz" && npm run build && npx playwright install chromium && npm test -- --max-failures=1)
node scripts/ci/consumer_validate.mjs "$NITRO_CONSUMER_OUT"
echo "consumer-smoke: PASS (packed packages, production Vite, real React save, Open XML SDK + LibreOffice)"
