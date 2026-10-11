#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Assemble fixtures/manifest.json from real/, upstream/ and generated/ recipes."""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from ooxml_zip import sha256_file  # noqa: E402
from painter_fixtures import PAINTER_RECIPES, PAINTER_ROWS_UNEXERCISED, build_painter_recipe  # noqa: E402
from tiers import RECIPES, build_recipe  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "fixtures" / "manifest.json"
SELECTION = Path(__file__).resolve().parent / "upstream_selection.json"


def tier_of(nbytes: int) -> str:
    if nbytes < 100_000:
        return "S"
    if nbytes < 1_000_000:
        return "M"
    if nbytes < 10_000_000:
        return "L"
    return "XL"


def painter_tags_for_real(fmt: str, features: list[str], path: str) -> list[str]:
    """Map real-corpus feature tags / filenames onto the 36 unexercised painter rows."""
    tags: set[str] = set()
    feat = set(features)
    name = Path(path).name.lower()
    if fmt == "xlsx":
        if "conditional-formats" in feat or "databar" in name or "conditional" in name:
            tags.add("painter:X3")
        if "drawing" in feat or "charts" in feat:
            tags.add("painter:X8")
        if "charts" in feat:
            tags.update(["painter:X9", "painter:S1"])
        tags.add("painter:X10")  # print path accepts any sheet
        if "sparkline" in name:
            tags.add("painter:X11")
        if "smartart" in name or "smartart" in feat:
            tags.update(["painter:X12", "painter:S2"])
    elif fmt == "docx":
        if "charts" in feat or "chartex" in name:
            tags.update(["painter:D4", "painter:S1"])
        if "headers-footers" in feat or "header" in name or "footer" in name or "hdr" in name:
            tags.add("painter:D5")
        if "smartart" in feat or "smartart" in name or "shp-inline" in name:
            tags.update(["painter:D11", "painter:S2"])
    elif fmt == "pptx":
        if "wordart" in feat or "warp" in name or "wordart" in name:
            tags.add("painter:P4")
        tags.add("painter:P6")  # most decks have groups; real corpus scan confirmed
        if "connector" in feat or "cxn" in name or "shp-shapes" in name or "shp-common" in name:
            tags.add("painter:P7")
        if "tables" in feat:
            tags.add("painter:P8")
        if "charts" in feat:
            tags.add("painter:P9")
        if "3d" in name or "chart-type" in name or "bar3d" in name:
            tags.update(["painter:P10", "painter:P13"])
        if "smartart" in feat:
            tags.add("painter:S2")
        if "three-d-smartart" in name or "smartart" in feat and "3d" in name:
            tags.update(["painter:P12", "painter:P13"])
        if "media" in feat or "audio" in name or "movie" in name or "embedded" in name:
            tags.add("painter:P14")
        if "ole" in name or "attachments" in name:
            tags.add("painter:P15")
        if "ink" in name:
            tags.update(["painter:P18", "painter:P19"] if "contentpart" in name else ["painter:P18"])
        if "contentpart" in name:
            tags.add("painter:P19")
        if "comment" in name or "comments" in feat:
            tags.add("painter:P23")
        if "animations" in feat or "transition" in name or "keyframes" in name:
            tags.add("painter:P24")
        tags.update(["painter:P26", "painter:P27"])  # export paths take any deck
    return sorted(tags)


def painter_tags_for_upstream(upstream_path: str, fmt: str) -> list[str]:
    p = upstream_path.lower()
    tags: set[str] = set()
    if fmt == "xlsx":
        if "sparkline" in p:
            tags.add("painter:X11")
        if "smartart" in p:
            tags.update(["painter:X12", "painter:S2"])
        if "feature" in p or "openpyxl" in p:
            tags.add("painter:X3")
        if "chart" in p:
            tags.update(["painter:X8", "painter:X9", "painter:S1"])
        tags.add("painter:X10")
    elif fmt == "docx":
        if "smartart" in p:
            tags.update(["painter:D11", "painter:S2"])
        if "chart" in p:
            tags.update(["painter:D4", "painter:S1"])
        if "header" in p or "footer" in p or "field" in p:
            tags.add("painter:D5")
    elif fmt == "pptx":
        if "wordart" in p or "text-warp" in p:
            tags.add("painter:P4")
        if "group" in p or "smartart" in p or "corpus" in p:
            tags.add("painter:P6")
        if "connector" in p or "calibri-metric" in p:
            tags.add("painter:P7")
        if "table" in p:
            tags.add("painter:P8")
        if "chart" in p and "3d" not in p and "three-d" not in p:
            tags.add("painter:P9")
        if "three-d-charts" in p or "bar3d" in p or "pie3d" in p:
            tags.update(["painter:P9", "painter:P10", "painter:P13"])
        if "three-d-smartart" in p:
            tags.update(["painter:P12", "painter:P13", "painter:S2"])
        if "smartart" in p:
            tags.add("painter:S2")
        if "media" in p or "audio" in p or "sound" in p:
            tags.add("painter:P14")
        if "ole" in p:
            tags.add("painter:P15")
        if "ink-annotation" in p:
            tags.add("painter:P18")
        if "ink-contentpart" in p:
            tags.update(["painter:P18", "painter:P19"])
        if "text-layout" in p or "equation" in p:
            tags.add("painter:P20")
        if "animation" in p or "transition" in p:
            tags.add("painter:P24")
        if "notes" in p or "table-paragraph" in p:
            tags.add("painter:P25")
        if "comment" in p:
            tags.add("painter:P23")
        tags.update(["painter:P6", "painter:P26", "painter:P27"])
    return sorted(tags)


def entry(
    *,
    id: str,
    format: str,
    path: str,
    sha256: str,
    bytes_: int,
    source: str,
    licence: str,
    features: list[str],
    redistributable: bool = True,
    on_demand: bool = False,
    generator: dict | None = None,
    upstream_path: str | None = None,
) -> dict:
    row = {
        "id": id,
        "format": format,
        "path": path,
        "sha256": sha256,
        "bytes": bytes_,
        "tier": tier_of(bytes_),
        "source": source,
        "licence": licence,
        "features": sorted(set(features)),
        "redistributable": redistributable,
        "on_demand": on_demand,
    }
    if generator:
        row["generator"] = generator
    if upstream_path:
        row["upstream_path"] = upstream_path
    return row


def build() -> dict:
    rows: list[dict] = []

    # Real Office-authored corpus (#32 / #95 / #96)
    for fmt in ("xlsx", "docx", "pptx"):
        src = ROOT / "fixtures" / "real" / fmt / "sources.json"
        for r in json.loads(src.read_text()):
            path = r["path"]
            abs_path = ROOT / path
            digest = sha256_file(abs_path)
            if digest != r["sha256"]:
                raise SystemExit(f"real sha256 mismatch: {path}")
            feats = list(r.get("features", [])) + painter_tags_for_real(fmt, r.get("features", []), path)
            rows.append(
                entry(
                    id=r["id"],
                    format=fmt,
                    path=path,
                    sha256=digest,
                    bytes_=abs_path.stat().st_size,
                    source=r.get("source_url", "real"),
                    licence=r["licence"],
                    features=feats + ["parity-corpus", "real-office"],
                    redistributable=bool(r.get("redistributable", True)),
                )
            )

    # Upstream redistributable copies
    sel = json.loads(SELECTION.read_text())
    for s in sel["selection"]:
        if not s.get("redistributable", False):
            # Record as non-committed reference only (not in path checks).
            rows.append(
                entry(
                    id="upstream-noredist-" + s["sha256"][:12],
                    format="pptx" if "pptx" in s["upstream_path"] else ("docx" if "docx" in s["upstream_path"] else "xlsx"),
                    path="",  # not committed
                    sha256=s["sha256"],
                    bytes_=0,
                    source=f"upstream:{s['upstream_path']}",
                    licence="unknown",
                    features=["upstream", "third-party", f"provenance:{s.get('provenance', '?')}"],
                    redistributable=False,
                    on_demand=False,
                    upstream_path=s["upstream_path"],
                )
            )
            continue
        local = s.get("local_path")
        if not local:
            raise SystemExit(f"missing local_path; run copy_upstream.py first: {s['upstream_path']}")
        abs_path = ROOT / local
        digest = sha256_file(abs_path)
        if digest != s["sha256"]:
            raise SystemExit(f"upstream copy sha mismatch: {local}")
        fmt = "docx" if "/docx/" in local else ("xlsx" if "/xlsx/" in local else "pptx")
        feats = ["upstream", f"provenance:{s.get('provenance', 'upstream-synthetic')}"] + painter_tags_for_upstream(
            s["upstream_path"], fmt
        )
        # Prefer the commit that introduced the file when selection records it
        # (e.g. outside-contributor fixtures reviewed for redistribution).
        if s.get("added_by_commit"):
            source = (
                f"https://github.com/ChristopherVR/ooxml/commit/{s['added_by_commit']} "
                f"({s['upstream_path']}; pin e66aaec)"
            )
        else:
            source = (
                "https://github.com/ChristopherVR/ooxml/blob/"
                f"e66aaec24adc0b68472eae1fc42aa4598e09c352/{s['upstream_path']}"
            )
        rows.append(
            entry(
                id="upstream-" + s["sha256"][:12],
                format=fmt,
                path=local,
                sha256=digest,
                bytes_=abs_path.stat().st_size,
                source=source,
                licence="Apache-2.0",
                features=feats,
                redistributable=True,
                upstream_path=s["upstream_path"],
            )
        )

    # Generated tier + painter recipes (ensure files exist for committed ones)
    all_recipes = {**RECIPES, **{k: {**v, "on_demand": v.get("on_demand", False)} for k, v in PAINTER_RECIPES.items()}}
    for rid, recipe in RECIPES.items():
        on_demand = bool(recipe.get("on_demand"))
        if on_demand:
            # Generate once to capture sha/bytes for the manifest, then leave file for verify.
            path = build_recipe(rid, force=False)
        else:
            path = build_recipe(rid, force=False)
        digest = sha256_file(path)
        rel = str(path.relative_to(ROOT))
        rows.append(
            entry(
                id=rid,
                format=recipe["format"],
                path=rel,
                sha256=digest,
                bytes_=path.stat().st_size,
                source="generated",
                licence="MIT",
                features=list(recipe.get("features", [])) + ["generated"],
                redistributable=True,
                on_demand=on_demand,
                generator={"tool": "fixtures/gen/generate.py", "id": rid, "seed": 4},
            )
        )

    for rid, recipe in PAINTER_RECIPES.items():
        path = build_painter_recipe(rid, force=False)
        digest = sha256_file(path)
        rel = str(path.relative_to(ROOT))
        rows.append(
            entry(
                id=rid,
                format=recipe["format"],
                path=rel,
                sha256=digest,
                bytes_=path.stat().st_size,
                source="generated",
                licence="MIT",
                features=list(recipe.get("features", [])) + ["generated", "painter-fixture"],
                redistributable=True,
                on_demand=False,
                generator={"tool": "fixtures/gen/generate.py", "id": rid, "seed": 4},
            )
        )

    # Sort for stability
    rows.sort(key=lambda r: (r["format"], r["id"]))

    covered = set()
    for r in rows:
        if not r["redistributable"]:
            continue
        for f in r["features"]:
            if f.startswith("painter:"):
                covered.add(f.split(":", 1)[1])
    missing = [p for p in PAINTER_ROWS_UNEXERCISED if p not in covered]

    manifest = {
        "$schema": "manifest.schema.json",
        "version": 1,
        "tiers": {"S": "<100KB", "M": "<1MB", "L": "<10MB", "XL": ">=10MB"},
        "painter_rows_required": PAINTER_ROWS_UNEXERCISED,
        "painter_rows_covered": sorted(covered),
        "painter_rows_missing": missing,
        "fixtures": rows,
    }
    return manifest


def main() -> int:
    manifest = build()
    MANIFEST.write_text(json.dumps(manifest, indent=2) + "\n")
    print(
        f"wrote {MANIFEST} entries={len(manifest['fixtures'])} "
        f"painter_covered={len(manifest['painter_rows_covered'])}/36 "
        f"missing={manifest['painter_rows_missing']}"
    )
    if manifest["painter_rows_missing"]:
        return 1
    return 0


if __name__ == "__main__":
    # Allow running as a script from fixtures/gen
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    raise SystemExit(main())
