#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Extract theme1.xml (and xl/theme/theme1.xml) from packages into a temp dir for #25."""
import sys
import zipfile
from pathlib import Path

THEME_NAMES = {
    "ppt/theme/theme1.xml",
    "xl/theme/theme1.xml",
    "word/theme/theme1.xml",
    "theme/theme1.xml",
}


def extract_from_zip(path: Path, out: Path) -> int:
    n = 0
    try:
        with zipfile.ZipFile(path) as zf:
            for name in THEME_NAMES:
                if name in zf.namelist():
                    dest = out / f"{path.stem}-{name.replace('/', '_')}"
                    dest.write_bytes(zf.read(name))
                    n += 1
                    break
    except zipfile.BadZipFile:
        return 0
    return n


def main() -> None:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else ".")
    out = Path(sys.argv[2] if len(sys.argv) > 2 else "/tmp/drawingml-themes")
    out.mkdir(parents=True, exist_ok=True)
    count = 0
    patterns = [
        "fixtures/opc/allowed/*",
        "fixtures/real/*/*",
        "upstream/src/core/pptx/__tests__/fixtures/*.pptx",
    ]
    seen: set[Path] = set()
    for pat in patterns:
        for path in root.glob(pat):
            if not path.is_file() or path in seen:
                continue
            seen.add(path)
            if path.suffix.lower() not in {".pptx", ".xlsx", ".docx", ".pptm", ".xlsm", ".docm"}:
                continue
            count += extract_from_zip(path, out)
    print(str(out))
    print(count, file=sys.stderr)


if __name__ == "__main__":
    main()
