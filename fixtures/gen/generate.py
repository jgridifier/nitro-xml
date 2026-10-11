#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""CLI: generate committed and on-demand fixture packages (fixed seed=4)."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from painter_fixtures import PAINTER_RECIPES, build_painter_recipe  # noqa: E402
from tiers import RECIPES, build_recipe  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--id", action="append", dest="ids", help="Recipe id (repeatable)")
    ap.add_argument("--committed-only", action="store_true", help="Skip on_demand L/XL recipes")
    ap.add_argument("--on-demand-only", action="store_true", help="Only on_demand recipes")
    ap.add_argument("--force", action="store_true", help="Rebuild even if the file exists")
    ap.add_argument("--all", action="store_true", help="Build every recipe")
    args = ap.parse_args()

    ids: list[str] = []
    if args.ids:
        ids = args.ids
    elif args.all or args.committed_only or args.on_demand_only:
        ids = list(RECIPES) + list(PAINTER_RECIPES)
    else:
        ap.error("pass --all, --committed-only, --on-demand-only, or --id")

    built = 0
    for rid in ids:
        if rid in RECIPES:
            on_demand = bool(RECIPES[rid].get("on_demand"))
            if args.committed_only and on_demand:
                continue
            if args.on_demand_only and not on_demand:
                continue
            path = build_recipe(rid, force=args.force)
            print(f"{rid} -> {path} ({path.stat().st_size} bytes)")
            built += 1
        elif rid in PAINTER_RECIPES:
            if args.on_demand_only:
                continue
            path = build_painter_recipe(rid, force=args.force)
            print(f"{rid} -> {path} ({path.stat().st_size} bytes)")
            built += 1
        else:
            print(f"unknown recipe id: {rid}", file=sys.stderr)
            return 1
    print(f"built={built}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
