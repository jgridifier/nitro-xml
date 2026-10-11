#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Emit fixtures/opc/safe-href.jsonl from upstream safe-href.test.ts input cases."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "fixtures" / "opc" / "safe-href.jsonl"

JS = "javascript"
ch = chr


def rows() -> list[dict]:
    openable = [
        "https://example.com",
        "http://example.com",
        "HTTPS://EXAMPLE.COM",
        "mailto:user@example.com",
        "  https://example.com  ",
        f"https://example.com/{JS}/docs",
        "https://example.com/a b",
    ]
    store_only = [
        "ftp://files.example.com",
        "file:///C:/Reports/q1.xlsx",
        "FILE://server/share/a.xlsx",
        "#Sheet2!A1",
        "#'My Sheet'!B3",
        "Sheet2!A1",
        "/page/about",
        "../reports/q1.xlsx",
        "q1.xlsx",
        "\\\\server\\share\\q1.xlsx",
        "C:\\Reports\\q1.xlsx",
        "//example.com/x",
    ]
    scripts = [
        f"{JS}:alert(1)",
        f"{JS.upper()}:alert(1)",
        "JaVaScRiPt:alert(1)",
        f"  {JS}:alert(1)",
        f"{ch(1)}{JS}:alert(1)",
        "data:text/html,<h1>XSS</h1>",
        "DATA:text/html;base64,PHNjcmlwdD4=",
        "vbscript:MsgBox('XSS')",
        "mhtml:file://C:/test.mht",
        f"x-{JS}:alert(1)",
        "livescript:alert(1)",
        "ms-its:c:/x.chm::/a.htm",
        "blob:https://example.com/uuid",
        f"java{ch(0x200b)}script:alert(1)",
        f"java{ch(0x200c)}script:alert(1)",
        f"java{ch(0x200d)}script:alert(1)",
        f"{ch(0xfeff)}{JS}:alert(1)",
        f"java{ch(0)}script:alert(1)",
        "java\tscript:alert(1)",
        "java\nscript:alert(1)",
        "java\rscript:alert(1)",
        "java script:alert(1)",
        f"java{ch(0xad)}script:alert(1)",
        f"java{ch(0x2060)}script:alert(1)",
        f"{ch(0x202e)}{JS}:alert(1)",
        "da\tta:text/html,x",
    ]
    obfuscated_ok_scheme = ["ht\ttp://example.com", f"ht{ch(0x200b)}tps://example.com"]
    control = [f"https://example.com/{ch(0)}", "https://a\nb", f"#Sheet1{ch(7)}!A1"]
    unsupported = ["tel:+123", "ms-word:ofe|u|https://x", "chrome://settings", "about:blank"]
    empty = ["", "   ", ch(0x200b)]

    out: list[dict] = []
    for i, href in enumerate(openable):
        out.append({"id": f"open-{i}", "href": href})
    for i, href in enumerate(store_only):
        out.append({"id": f"store-{i}", "href": href})
    for i, href in enumerate(scripts):
        out.append({"id": f"script-{i}", "href": href})
    for i, href in enumerate(obfuscated_ok_scheme):
        out.append({"id": f"obfuscated-{i}", "href": href})
    for i, href in enumerate(control):
        out.append({"id": f"control-{i}", "href": href})
    for i, href in enumerate(unsupported):
        out.append({"id": f"unsupported-{i}", "href": href})
    for i, href in enumerate(empty):
        out.append({"id": f"empty-{i}", "href": href})
    out.append({"id": "null-0", "href": None})
    # Fuzz-style spliced script schemes (fixed seeds, not a property test).
    for i, (scheme, gaps) in enumerate(
        [
            ("javascript", ["", "\t", "", "", "", "", "", "", "", "", ""]),
            ("data", ["", "\u200b", "", "", ""]),
            ("vbscript", [" ", "", "", "", "", "", "", ""]),
        ]
    ):
        letters = []
        for idx, letter in enumerate(scheme):
            gap = gaps[idx] if idx < len(gaps) else ""
            letters.append(f"{letter}{gap}")
        href = f"{''.join(letters)}:x"
        out.append({"id": f"fuzz-{i}", "href": href})
    return out


def main() -> None:
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with OUT.open("w", encoding="utf-8") as f:
        for row in rows():
            f.write(json.dumps(row, ensure_ascii=False) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
