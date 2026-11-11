<!-- SPDX-License-Identifier: MIT -->
# Packed-package consumer

Run `bash scripts/ci/consumer_smoke.sh` from the repository root after building
WASM and installing workspace dependencies. It copies this tiny host into a fresh
temporary directory, installs both npm tarballs (no workspace links or source
aliases), checks the published TypeScript declarations under Node ESM, builds Vite for production and opens/edits/saves all three formats in
Chromium with one worker. `vite.config.js` sets `worker.format: "es"` because
the engine worker uses dynamic imports. Open XML SDK and LibreOffice must be installed via
`scripts/ci/setup-validators.sh`; missing validators fail the check.

`NITRO_CONSUMER_OUT=/absolute/path` retains saved files and pack manifests.
`NITRO_HANDCHECK=1` runs ten cases instead of three and includes original files,
edit commands and an Office review README. Neither mode publishes anything.

The declaration check includes `ESNext.Disposable` for the generated wasm-bindgen
`Symbol.dispose` types (TypeScript 5.2+); it does not use `skipLibCheck`.
