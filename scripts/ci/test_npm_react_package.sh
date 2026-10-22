#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Catches: npm package exports / React binding regressing SSR safety or the
# eager loader brotli ceiling (must stay ≤ 17408 B).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

# Catches: package.json dropping ESM exports or provenance publishConfig.
node --input-type=module -e '
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
const pkg = JSON.parse(readFileSync("packages/nitro-xml/package.json","utf8"));
assert.equal(pkg.type, "module");
assert.ok(pkg.exports["."]);
assert.ok(pkg.exports["./loader"]);
assert.ok(pkg.exports["./wasm"]);
assert.equal(pkg.publishConfig?.provenance, true);
const react = JSON.parse(readFileSync("packages/nitro-xml-react/package.json","utf8"));
assert.equal(react.name, "@nitro-xml/react");
assert.ok(react.peerDependencies["nitro-xml"]);
assert.equal(react.sideEffects, false);
'

cd "$ROOT/packages/nitro-xml-react"
npm run test

cd "$ROOT"
bash scripts/ci/bundle_budget.sh

echo "test_npm_react_package: ok"
