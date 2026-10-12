#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Fuzz corpus smoke: rebuild cargo-fuzz targets and replay every seed once.
# Part of #30 (parser hardening). Does not run a long fuzz campaign — CI should
# stay fast. Agents never edit .github/workflows/; ask Jared to wire this job
# into required checks when ready.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

FUZZ_DIR="$ROOT/rs/fuzz"
SEED_DIR="$FUZZ_DIR/seed"
CORPUS_DIR="$FUZZ_DIR/corpus"
FIXTURES_FUZZ="$ROOT/fixtures/fuzz"

TARGETS=(xml_parse zip_open opc_open xlsx_load docx_load pptx_load safe_href)

if ! command -v cargo-fuzz >/dev/null 2>&1; then
  echo "cargo-fuzz not installed; install with: cargo install cargo-fuzz"
  exit 1
fi

if ! rustup toolchain list | grep -q '^nightly'; then
  echo "nightly toolchain required for cargo-fuzz"
  exit 1
fi

sync_corpus() {
  local target="$1"
  local dest="$CORPUS_DIR/$target"
  mkdir -p "$dest"
  # Curated seeds checked into the repo.
  if [[ -d "$SEED_DIR/$target" ]]; then
    cp -a "$SEED_DIR/$target"/. "$dest"/
  fi
  # Limit / crash fixtures shared across related targets.
  case "$target" in
    xml_parse)
      [[ -d "$FIXTURES_FUZZ/xml" ]] && cp -a "$FIXTURES_FUZZ/xml"/. "$dest"/ || true
      ;;
    zip_open)
      [[ -d "$FIXTURES_FUZZ/zip" ]] && cp -a "$FIXTURES_FUZZ/zip"/. "$dest"/ || true
      ;;
    opc_open)
      [[ -d "$FIXTURES_FUZZ/zip" ]] && cp -a "$FIXTURES_FUZZ/zip"/. "$dest"/ || true
      [[ -d "$FIXTURES_FUZZ/opc" ]] && cp -a "$FIXTURES_FUZZ/opc"/. "$dest"/ || true
      ;;
    xlsx_load)
      # Legitimate high-compression workbooks (seeded); skip copying whole fixtures/fuzz/xlsx
      # if seeds already include the curated ones.
      [[ -d "$FIXTURES_FUZZ/xlsx" ]] && cp -a "$FIXTURES_FUZZ/xlsx"/. "$dest"/ || true
      ;;
    docx_load)
      # Do not auto-copy fixtures/fuzz/docx here: repeated-paragraphs-* inflate to
      # 7–21 MiB XML and blow RSS when the full docx reader parses them. Those files
      # are exercised via zip_open / opc_open seeds and nitro-xml-opc unit tests.
      ;;
    pptx_load)
      [[ -d "$FIXTURES_FUZZ/pptx" ]] && cp -a "$FIXTURES_FUZZ/pptx"/. "$dest"/ || true
      ;;
    safe_href)
      [[ -d "$FIXTURES_FUZZ/safe-href" ]] && cp -a "$FIXTURES_FUZZ/safe-href"/. "$dest"/ || true
      ;;
  esac
}

echo "## fuzz-smoke"
echo "targets: ${TARGETS[*]}"

cd "$FUZZ_DIR"
for target in "${TARGETS[@]}"; do
  sync_corpus "$target"
  count="$(find "$CORPUS_DIR/$target" -type f | wc -l | tr -d ' ')"
  echo "### $target ($count corpus files)"
  cargo fuzz build "$target"
  # -runs=0: execute every corpus input once and exit (libFuzzer).
  cargo fuzz run "$target" -- -runs=0
done

msg="fuzz-smoke: all ${#TARGETS[@]} targets replayed corpus without crash"
echo "$msg"
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  echo "$msg" >>"$GITHUB_STEP_SUMMARY"
fi
