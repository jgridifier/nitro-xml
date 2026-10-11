#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Compare DrawingML gate JSON (preset paths with numeric tolerance)."""
import json
import re
import sys

FLOAT_RE = re.compile(r"-?\d+\.\d+|-?\d+\.|-?\d+")


def canon(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"))


def norm_path(s: str, tol: float = 0.01) -> str:
    def repl(m: re.Match[str]) -> str:
        v = float(m.group(0))
        r = round(v / tol) * tol
        if abs(r) < tol / 2:
            r = 0.0
        if r == int(r):
            return str(int(r))
        return f"{r:.3f}".rstrip("0").rstrip(".")

    return FLOAT_RE.sub(repl, s)


def norm_fill(obj):
    if not isinstance(obj, dict):
        return obj
    out = dict(obj)
    if out.get("kind") == "solid" and "color" in out:
        out["color"] = norm_color(out["color"])
    if out.get("kind") == "pattern":
        if "foreground" in out:
            out["foreground"] = norm_color(out.get("foreground"))
        if "background" in out:
            out["background"] = norm_color(out.get("background"))
    if out.get("kind") == "gradient" and "sourceXml" in out:
        out = {k: v for k, v in out.items() if k != "sourceXml"}
    if "angle" in out and out["angle"] is not None:
        out["angle"] = float(out["angle"])
    if "stops" in out and isinstance(out["stops"], list):
        out["stops"] = [
            {
                **s,
                "position": float(s.get("position", 0)),
                "color": norm_color(s.get("color")),
            }
            for s in out["stops"]
        ]
    return out


def norm_color(c):
    if not isinstance(c, dict):
        return c
    out = {k: v for k, v in c.items() if k != "transforms" or v}
    if "transforms" not in out:
        out["transforms"] = []
    return out


def norm_resolved(d):
    if not isinstance(d, dict):
        return d
    return {
        "hex": d.get("hex"),
        "alpha": float(d.get("alpha", 1)),
        "unapplied": d.get("unapplied") or [],
    }


def norm_theme_slots(slots):
    if not isinstance(slots, dict):
        return slots
    return {
        k: {
            "color": norm_color(v.get("color")),
            "legacy": norm_resolved(v.get("legacy")),
            "document": norm_resolved(v.get("document")),
            "base": norm_resolved(v.get("base")),
        }
        for k, v in slots.items()
    }


def norm_line(obj):
    if obj is None:
        return None
    if not isinstance(obj, dict):
        return obj
    out = dict(obj)
    if "fill" in out:
        out["fill"] = norm_fill(out["fill"])
    return out


def preset_equal(ref: dict, got: dict) -> bool:
    if ref.get("fillNone") != got.get("fillNone"):
        return False
    if norm_path(ref.get("svgPath", "")) != norm_path(got.get("svgPath", "")):
        return False
    rp = ref.get("paths") or []
    gp = got.get("paths") or []
    if len(rp) != len(gp):
        return False
    for a, b in zip(rp, gp):
        if a.get("fill") != b.get("fill") or a.get("stroke") != b.get("stroke"):
            return False
        if norm_path(a.get("d", "")) != norm_path(b.get("d", "")):
            return False
    tr = ref.get("textRect")
    tg = got.get("textRect")
    if tr is None and tg is None:
        return True
    if tr is None or tg is None:
        return False
    for k in "l", "t", "r", "b":
        if abs(float(tr[k]) - float(tg[k])) > 0.01:
            return False
    return True


def color_equal(ref: dict, got: dict) -> bool:
    return (
        ref.get("hex") == got.get("hex")
        and ref.get("alpha") == got.get("alpha")
        and ref.get("unapplied") == got.get("unapplied")
    )


def load_json_arg(arg: str) -> dict:
    if arg.startswith("@"):
        text = open(arg[1:], encoding="utf-8").read()
    else:
        text = arg
    return json.loads(text)


def main() -> None:
    mode = sys.argv[1]
    ref = load_json_arg(sys.argv[2])
    got = load_json_arg(sys.argv[3])
    if mode == "preset":
        ok = preset_equal(ref, got)
    elif mode == "color":
        ok = color_equal(ref, got)
    elif mode == "fill":
        ok = canon(norm_fill(ref.get("fill"))) == canon(norm_fill(got.get("fill"))) and canon(
            norm_line(ref.get("line"))
        ) == canon(norm_line(got.get("line")))
    elif mode == "theme":
        ok = canon(norm_theme_slots(ref)) == canon(norm_theme_slots(got))
    else:
        ok = ref == got
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
