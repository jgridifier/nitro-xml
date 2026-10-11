#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Fixed colour-transform matrix for the #25 gate (stdout: JSONL rows)."""
import json
import sys

TRANSFORMS = [
    [],
    [{"name": "lumMod", "value": "75000"}],
    [{"name": "lumOff", "value": "20000"}],
    [{"name": "tint", "value": "50000"}],
    [{"name": "shade", "value": "50000"}],
    [{"name": "satMod", "value": "120000"}],
    [{"name": "alpha", "value": "50000"}],
    [{"name": "alphaMod", "value": "80000"}],
    [{"name": "comp", "value": ""}],
    [{"name": "inv", "value": ""}],
    [{"name": "gray", "value": ""}],
    [{"name": "hueMod", "value": "120000"}],
    [
        {"name": "lumMod", "value": "80000"},
        {"name": "tint", "value": "40000"},
        {"name": "alpha", "value": "90000"},
    ],
]

BASES = [
    {"id": "srgb_red", "color": {"kind": "srgb", "value": "FF0000", "transforms": []}},
    {"id": "srgb_gray", "color": {"kind": "srgb", "value": "808080", "transforms": []}},
    {"id": "srgb_white", "color": {"kind": "srgb", "value": "FFFFFF", "transforms": []}},
    {
        "id": "scheme_accent1",
        "color": {"kind": "scheme", "value": "accent1", "transforms": []},
        "theme": {
            "accent1": "4472C4",
            "dk1": "000000",
            "lt1": "FFFFFF",
            "dk2": "44546A",
            "lt2": "E7E6E6",
        },
    },
    {
        "id": "scheme_tx1",
        "color": {"kind": "scheme", "value": "tx1", "transforms": []},
        "theme": {"bg1": "FFFFFF", "tx1": "000000", "bg2": "E7E6E6", "tx2": "44546A"},
    },
]

ORDERS = ["legacy", "document"]


def main() -> None:
    for base in BASES:
        for ti, transforms in enumerate(TRANSFORMS):
            color = dict(base["color"])
            color["transforms"] = transforms
            for order in ORDERS:
                row = {
                    "id": f"{base['id']}/t{ti}/{order}",
                    "color": color,
                    "theme": base.get("theme", {}),
                    "transform_order": order,
                }
                print(json.dumps(row, separators=(",", ":")))


if __name__ == "__main__":
    main()
