#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Copy redistributable upstream fixtures by sha256 into fixtures/upstream/."""

from __future__ import annotations

import json
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from ooxml_zip import sha256_file  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
SELECTION = Path(__file__).resolve().parent / "upstream_selection.json"
DEST = ROOT / "fixtures" / "upstream"
UPSTREAM = ROOT / "upstream"


def short_name(upstream_path: str, sha256: str) -> str:
    """Stable unique name: 12-hex prefix avoids smartart-gallery collisions."""
    return f"{sha256[:12]}_{Path(upstream_path).name}"


def format_of(upstream_path: str) -> str:
    p = upstream_path.lower()
    if "/docx/" in p or p.endswith((".docx", ".docm", ".dotx", ".dotm")):
        return "docx"
    if "/xlsx/" in p or p.endswith((".xlsx", ".xlsm", ".xltx", ".xltm")):
        return "xlsx"
    if "pptx" in p or p.endswith((".pptx", ".pptm", ".potx", ".potm")):
        return "pptx"
    raise ValueError(f"unknown format for {upstream_path}")


def main() -> int:
    data = json.loads(SELECTION.read_text())
    copied = 0
    skipped = 0
    errors = 0
    for entry in data["selection"]:
        if not entry.get("redistributable", False):
            skipped += 1
            continue
        src = UPSTREAM / entry["upstream_path"]
        if not src.is_file():
            print(f"missing upstream file: {entry['upstream_path']}", file=sys.stderr)
            errors += 1
            continue
        digest = sha256_file(src)
        if digest != entry["sha256"]:
            print(f"sha256 mismatch for {entry['upstream_path']}: {digest} != {entry['sha256']}", file=sys.stderr)
            errors += 1
            continue
        fmt = format_of(entry["upstream_path"])
        dest = DEST / fmt / short_name(entry["upstream_path"], digest)
        dest.parent.mkdir(parents=True, exist_ok=True)
        if not dest.exists() or sha256_file(dest) != digest:
            shutil.copy2(src, dest)
        entry["local_path"] = str(dest.relative_to(ROOT))
        copied += 1
    # Persist local_path annotations for the manifest builder.
    SELECTION.write_text(json.dumps(data, indent=1) + "\n")
    print(f"copied={copied} skipped_non_redist={skipped} errors={errors}")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
