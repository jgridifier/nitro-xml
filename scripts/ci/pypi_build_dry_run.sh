#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PY="$ROOT/python"

if ! command -v maturin >/dev/null 2>&1; then
  python3 -m pip install --user 'maturin>=1.4,<2.0' 'pytest>=7.0'
  export PATH="${HOME}/.local/bin:${PATH}"
fi

cd "$PY"
VENV="$PY/.venv"
if [[ ! -d "$VENV" ]]; then
  python3 -m venv "$VENV"
  "$VENV/bin/pip" install 'maturin>=1.4,<2.0' 'pytest>=7.0'
fi
# shellcheck source=/dev/null
source "$VENV/bin/activate"
maturin develop --release
pytest
