#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Installs pinned validators into a cached tool dir (#48):
#   1. .NET SDK via dotnet-install.sh → $NITRO_TOOLS_DIR/dotnet
#   2. tools/ooxml-validate (Open XML SDK OpenXmlValidator, Office2019)
#      published to $NITRO_TOOLS_DIR/ooxml-validate/
#   3. LibreOffice headless from apt (libreoffice-core-nogui)
#
# Versions live only in scripts/ci/versions.env. GitHub Actions already caches
# ~/.nitro-tools keyed on versions.env + tools/** (see ci.yml roundtrip job).
# Agents never edit .github/workflows/.
#
# Definition of done for #48: this script no longer prints NOT IMPLEMENTED.
# Ask Jared to add nothing new — the `roundtrip` job already calls this script;
# ensure `roundtrip` stays in branch-protection required checks.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=versions.env
. scripts/ci/versions.env

TOOLS="${NITRO_TOOLS_DIR:-$HOME/.nitro-tools}"
DOTNET_ROOT="$TOOLS/dotnet"
VALIDATE_OUT="$TOOLS/ooxml-validate"
STAMP="$TOOLS/.validators-stamp"
EXPECTED_STAMP="${DOTNET_SDK_VERSION}|${OPENXML_SDK_VERSION}|${LIBREOFFICE_APT_PACKAGE}"

export NITRO_TOOLS_DIR="$TOOLS"
mkdir -p "$TOOLS"

echo "setup-validators: tool dir $TOOLS"

need_dotnet_build=1
if [ -f "$STAMP" ] && [ "$(cat "$STAMP")" = "$EXPECTED_STAMP" ]; then
  if [ -x "$VALIDATE_OUT/ooxml-validate" ] || [ -f "$VALIDATE_OUT/ooxml-validate.dll" ]; then
    need_dotnet_build=0
    echo "setup-validators: cache hit (stamp $EXPECTED_STAMP)"
  fi
fi

if [ "$need_dotnet_build" -eq 1 ]; then
  if [ ! -x "$DOTNET_ROOT/dotnet" ]; then
    echo "setup-validators: installing .NET SDK ${DOTNET_SDK_VERSION}"
    curl -fsSL https://dot.net/v1/dotnet-install.sh -o /tmp/dotnet-install.sh
    bash /tmp/dotnet-install.sh --version "$DOTNET_SDK_VERSION" --install-dir "$DOTNET_ROOT"
  else
    echo "setup-validators: reusing .NET SDK at $DOTNET_ROOT"
  fi
  export DOTNET_ROOT
  export PATH="$DOTNET_ROOT:$PATH"
  export DOTNET_NOLOGO=1
  export DOTNET_CLI_TELEMETRY_OPTOUT=1

  echo "setup-validators: publishing tools/ooxml-validate (OpenXml ${OPENXML_SDK_VERSION})"
  rm -rf "$VALIDATE_OUT"
  mkdir -p "$VALIDATE_OUT"
  (
    cd "$ROOT/tools/ooxml-validate"
    # Restore with the pinned package version from versions.env.
    "$DOTNET_ROOT/dotnet" publish \
      -c Release \
      -o "$VALIDATE_OUT" \
      -p:OpenXmlSdkVersion="$OPENXML_SDK_VERSION" \
      --verbosity quiet
  )
  # Prefer a native apphost when present; otherwise `dotnet ooxml-validate.dll`.
  if [ -f "$VALIDATE_OUT/ooxml-validate" ]; then
    chmod +x "$VALIDATE_OUT/ooxml-validate"
  fi
  printf '%s\n' "$EXPECTED_STAMP" > "$STAMP"
else
  export DOTNET_ROOT
  export PATH="$DOTNET_ROOT:$PATH"
fi

if ! command -v soffice >/dev/null 2>&1 && ! command -v libreoffice >/dev/null 2>&1; then
  echo "setup-validators: installing ${LIBREOFFICE_APT_PACKAGE} (headless)"
  if [ "$(id -u)" -eq 0 ]; then
    apt-get update -qq
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
      "$LIBREOFFICE_APT_PACKAGE" libreoffice-calc-nogui libreoffice-writer-nogui libreoffice-impress-nogui \
      >/dev/null
  else
    sudo apt-get update -qq
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
      "$LIBREOFFICE_APT_PACKAGE" libreoffice-calc-nogui libreoffice-writer-nogui libreoffice-impress-nogui \
      >/dev/null
  fi
else
  echo "setup-validators: LibreOffice already on PATH"
fi

# --- smoke: Open XML SDK validator ---
SMOKE_XLSX="$ROOT/fixtures/opc/allowed/minimal.xlsx"
if [ ! -f "$SMOKE_XLSX" ]; then
  echo "setup-validators: missing smoke fixture $SMOKE_XLSX" >&2
  exit 1
fi
if [ -x "$VALIDATE_OUT/ooxml-validate" ]; then
  VALIDATE_CMD=("$VALIDATE_OUT/ooxml-validate")
else
  VALIDATE_CMD=(dotnet "$VALIDATE_OUT/ooxml-validate.dll")
fi
echo "setup-validators: smoke ooxml-validate on minimal.xlsx"
SMOKE_JSON=$("${VALIDATE_CMD[@]}" "$SMOKE_XLSX")
python3 -c 'import json,sys; json.loads(sys.argv[1])' "$SMOKE_JSON"

# --- smoke: LibreOffice headless ---
SOFFICE=$(command -v soffice || command -v libreoffice || true)
if [ -z "$SOFFICE" ]; then
  echo "setup-validators: soffice/libreoffice not found after install" >&2
  exit 1
fi
echo "setup-validators: smoke LibreOffice --version"
"$SOFFICE" --headless --version >/dev/null

echo "setup-validators: OK"
echo "  ooxml-validate → $VALIDATE_OUT"
echo "  soffice        → $SOFFICE"
echo "  NITRO_TOOLS_DIR=$TOOLS"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "## setup-validators (#48)"
    echo "- stamp: \`$EXPECTED_STAMP\`"
    echo "- ooxml-validate: \`$VALIDATE_OUT\`"
    echo "- soffice: \`$SOFFICE\`"
  } >> "$GITHUB_STEP_SUMMARY"
fi
exit 0
