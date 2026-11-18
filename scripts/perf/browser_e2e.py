#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""
Browser end-to-end timing: navigation to first DOM content paint in Chromium.

Measures, per fixture × format (xlsx/docx/pptx), time from navigationStart to the
moment the custom element's shadow DOM first shows content. This includes WASM fetch,
compile, and instantiate (fresh browser context per run = no HTTP cache).

Requires the _site demo tree assembled (see scripts/perf/compare_wasm_ts.sh or the
local-demo instructions in CLAUDE.md / brief).

Usage (from repo root):
  /workspace/pw-venv/bin/python scripts/perf/browser_e2e.py \\
      [--formats xlsx,docx,pptx] [--tiers S,M,L,XL] [--runs 7] \\
      [--bootstrap 2000] [--json] [--out docs/perf/browser-e2e.json]

The script serves _site itself; no external server needed.
"""
import argparse
import http.server
import json
import math
import os
import shutil
import socket
import sys
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
CHROMIUM = (
    "/home/box/.cache/ms-playwright/chromium_headless_shell-1254"
    "/chrome-headless-shell-linux64/chrome-headless-shell"
)
SEED = 20261019

FIXTURES = {
    ("xlsx", "S"):  "fixtures/generated/xlsx/cells_1k.xlsx",
    ("xlsx", "M"):  "fixtures/generated/xlsx/cells_100k.xlsx",
    ("xlsx", "L"):  "fixtures/generated/xlsx/cells_1m.xlsx",
    ("xlsx", "XL"): "fixtures/generated/xlsx/xl_a.xlsx",
    ("docx", "S"):  "fixtures/generated/docx/pages_10.docx",
    ("docx", "M"):  "fixtures/generated/docx/pages_100.docx",
    ("docx", "L"):  "fixtures/generated/docx/pages_500.docx",
    ("docx", "XL"): "fixtures/generated/docx/xl_a.docx",
    ("pptx", "S"):  "fixtures/generated/pptx/slides_10.pptx",
    ("pptx", "M"):  "fixtures/generated/pptx/slides_50.pptx",
    ("pptx", "L"):  "fixtures/generated/pptx/slides_200.pptx",
    ("pptx", "XL"): "fixtures/generated/pptx/xl_a.pptx",
}


def _find_free_port() -> int:
    s = socket.socket()
    s.bind(("", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def _build_site(site: Path) -> None:
    """Assemble _site for the demo server without rebuilding WASM."""
    demo_dst = site / "demo"
    loader_dst = demo_dst / "nitro-xml" / "loader"
    pkg_dst = demo_dst / "nitro-xml" / "pkg"
    for d in [loader_dst, pkg_dst]:
        d.mkdir(parents=True, exist_ok=True)

    # Copy demo/ → _site/demo/
    for item in (ROOT / "demo").iterdir():
        dst = demo_dst / item.name
        if item.is_dir():
            if dst.exists():
                shutil.rmtree(dst)
            shutil.copytree(item, dst)
        else:
            shutil.copy2(item, dst)

    # Copy loader/ → _site/demo/nitro-xml/loader/ (skip test/)
    for item in (ROOT / "packages/nitro-xml/loader").iterdir():
        if item.name == "test":
            continue
        dst = loader_dst / item.name
        if item.is_dir():
            if dst.exists():
                shutil.rmtree(dst)
            shutil.copytree(item, dst)
        else:
            shutil.copy2(item, dst)

    # Copy pkg/ → _site/demo/nitro-xml/pkg/
    for item in (ROOT / "packages/nitro-xml/pkg").iterdir():
        shutil.copy2(item, pkg_dst / item.name)

    # Perf harness page
    shutil.copy2(ROOT / "perf/browser-e2e.html", demo_dst / "perf-e2e.html")

    # Fixtures → _site/fixtures/generated/ (matches URL paths in FIXTURES dict)
    fixtures_dst = site / "fixtures" / "generated"
    if fixtures_dst.exists():
        shutil.rmtree(fixtures_dst)
    shutil.copytree(ROOT / "fixtures/generated", fixtures_dst)


class _Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, directory=None, **kwargs):
        super().__init__(*args, directory=str(directory), **kwargs)

    def log_message(self, *args):  # silence request logs
        pass

    def guess_type(self, path):
        if str(path).endswith(".wasm"):
            return "application/wasm"
        return super().guess_type(path)


def _serve(directory: Path, port: int):
    handler = lambda *a, **kw: _Handler(*a, directory=directory, **kw)
    server = http.server.HTTPServer(("127.0.0.1", port), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


# --- Statistics helpers (match JS harness: Mulberry32 PRNG + unpaired bootstrap CI) ---

def _median(xs):
    s = sorted(xs)
    n = len(s)
    return s[n // 2] if n % 2 else (s[n // 2 - 1] + s[n // 2]) / 2


def _percentile(xs, p):
    s = sorted(xs)
    n = len(s)
    i = (n - 1) * p
    lo, hi = int(i), math.ceil(i)
    return s[lo] if lo == hi else s[lo] * (hi - i) + s[hi] * (i - lo)


def _stdev(xs):
    if len(xs) < 2:
        return float("nan")
    m = sum(xs) / len(xs)
    return math.sqrt(sum((x - m) ** 2 for x in xs) / (len(xs) - 1))


def _mulberry32(seed):
    a = seed & 0xFFFFFFFF
    def _next():
        nonlocal a
        a = (a + 0x6D2B79F5) & 0xFFFFFFFF
        t = a ^ (a >> 15)
        t = (t * (t | 1)) & 0xFFFFFFFF
        t ^= (t + ((t ^ (t >> 7)) * (t | 61))) & 0xFFFFFFFF
        return ((t ^ (t >> 14)) & 0xFFFFFFFF) / 4294967296
    return _next


def _bootstrap_ci(samples, B, seed):
    n = len(samples)
    if n < 2:
        return (float("nan"), float("nan"))
    rand = _mulberry32(seed)
    medians = []
    for _ in range(B):
        resamp = [samples[int(rand() * n)] for _ in range(n)]
        medians.append(_median(resamp))
    medians.sort()
    return (_percentile(medians, 0.025), _percentile(medians, 0.975))


# --- Measurement ---

def _measure_fixture(fmt, tier, fixture_rel, port, runs, pw):
    """
    Run *runs* browser e2e measurements for one fixture.
    One fresh browser instance (clean WASM code cache) per run.
    Returns list of elapsed_ms values.
    """
    url = (
        f"http://127.0.0.1:{port}/demo/perf-e2e.html"
        f"?f={fmt}&src=/{fixture_rel}"
    )
    samples = []
    for i in range(runs):
        # Fresh browser per run = clean V8 WASM code cache.
        try:
            browser = pw.chromium.launch(
                executable_path=CHROMIUM,
                args=["--no-sandbox", "--disable-dev-shm-usage"],
            )
            ctx = browser.new_context()
            page = ctx.new_page()
            try:
                page.goto(url, timeout=90_000, wait_until="domcontentloaded")
                page.wait_for_function(
                    "window.__perfPaintMs !== undefined",
                    timeout=90_000,
                )
                val = page.evaluate("window.__perfPaintMs")
                if isinstance(val, (int, float)) and val > 0:
                    samples.append(float(val))
                else:
                    err = page.evaluate("window.__perfError")
                    print(
                        f"  WARN {fmt}-{tier} run {i}: paint=null err={err}",
                        file=sys.stderr,
                    )
            except Exception as e:
                print(f"  WARN {fmt}-{tier} run {i}: {e}", file=sys.stderr)
            finally:
                ctx.close()
                browser.close()
        except Exception as e:
            print(f"  WARN {fmt}-{tier} run {i} browser launch: {e}", file=sys.stderr)
    return samples


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--formats", default="xlsx,docx,pptx")
    ap.add_argument("--tiers", default="S,M,L,XL")
    ap.add_argument("--runs", type=int, default=7,
                    help="Runs per fixture (N≥5 required for HoP sign-off)")
    ap.add_argument("--bootstrap", type=int, default=2000)
    ap.add_argument("--out", default="docs/perf/browser-e2e.json")
    ap.add_argument("--json", action="store_true",
                    help="Print JSON to stdout in addition to writing --out")
    ap.add_argument("--skip-site-build", action="store_true",
                    help="Reuse existing _site (must already be assembled)")
    args = ap.parse_args()

    formats = [f.strip() for f in args.formats.split(",")]
    tiers   = [t.strip() for t in args.tiers.split(",")]
    runs    = args.runs

    site = ROOT / "_site"

    if args.skip_site_build and (site / "demo" / "nitro-xml" / "pkg" / "nitro_xml_wasm_bg.wasm").exists():
        print("browser_e2e.py: reusing existing _site", file=sys.stderr)
        # Refresh harness page and fixtures even when skipping full build.
        shutil.copy2(ROOT / "perf/browser-e2e.html", site / "demo" / "perf-e2e.html")
        if not (site / "fixtures" / "generated").exists():
            (site / "fixtures" / "generated").parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(ROOT / "fixtures/generated", site / "fixtures" / "generated")
    else:
        print("browser_e2e.py: assembling _site...", file=sys.stderr)
        _build_site(site)

    port = _find_free_port()
    server = _serve(site, port)
    print(f"browser_e2e.py: serving at http://127.0.0.1:{port}", file=sys.stderr)

    from playwright.sync_api import sync_playwright

    rows = []
    with sync_playwright() as pw:
        for fmt in formats:
            for tier in tiers:
                key = (fmt, tier)
                if key not in FIXTURES:
                    continue
                fixture_rel = FIXTURES[key]
                if not (ROOT / fixture_rel).exists():
                    print(f"  SKIP {fmt}-{tier}: fixture not found", file=sys.stderr)
                    rows.append({
                        "format": fmt, "tier": tier, "status": "FIXTURE_MISSING",
                        "fixture": fixture_rel, "samples_ms": [],
                    })
                    continue

                print(f"  {fmt}-{tier} ({runs} runs)...", file=sys.stderr)
                t0 = time.time()
                samples = _measure_fixture(fmt, tier, fixture_rel, port, runs, pw)
                wall = time.time() - t0

                if not samples:
                    print(f"    FAILED (0 samples in {wall:.1f}s)", file=sys.stderr)
                    rows.append({
                        "format": fmt, "tier": tier, "status": "FAILED",
                        "fixture": fixture_rel, "samples_ms": [],
                    })
                    continue

                med    = _median(samples)
                p95    = _percentile(samples, 0.95)
                sd     = _stdev(samples)
                spread = max(samples) - min(samples)
                seed_i = SEED + (formats.index(fmt) * len(tiers) + tiers.index(tier)) * 97
                ci_lo, ci_hi = _bootstrap_ci(samples, args.bootstrap, seed_i)

                print(
                    f"    n={len(samples)} median={med:.0f}ms p95={p95:.0f}ms "
                    f"stdev={sd:.0f}ms spread={spread:.0f}ms  ({wall:.0f}s wall)",
                    file=sys.stderr,
                )
                rows.append({
                    "format": fmt,
                    "tier": tier,
                    "status": "OK",
                    "fixture": fixture_rel,
                    "n": len(samples),
                    "median_ms": round(med, 1),
                    "p95_ms":    round(p95, 1),
                    "stdev_ms":  round(sd, 1),
                    "spread_ms": round(spread, 1),
                    "ci95_lo_ms": round(ci_lo, 1),
                    "ci95_hi_ms": round(ci_hi, 1),
                    "samples_ms": [round(x, 1) for x in samples],
                })

    server.shutdown()

    out_obj = {
        "schema": 1,
        "metric": "browser_e2e_first_paint_ms",
        "description": (
            "Time from navigationStart to first DOM content paint in Chromium headless shell, "
            "measured inside the page with performance.now(). Includes WASM fetch, compile, "
            "and instantiate. Fresh browser instance per run (no HTTP cache, no V8 code cache). "
            "Start: performance.now() at module script start (≈navigationStart+HTML-parse). "
            "End: shadow-DOM paint selector detected "
            "(.dve-print-page / .xg-root / [aria-roledescription=slide] / .dom childElementCount>0)."
        ),
        "chromium_bin": os.path.basename(CHROMIUM),
        "runs_per_fixture": runs,
        "bootstrap": args.bootstrap,
        "seed": SEED,
        "rows": rows,
    }

    out_path = ROOT / args.out
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(out_obj, indent=2) + "\n")
    print(f"browser_e2e.py: written {out_path}", file=sys.stderr)

    if args.json:
        print(json.dumps(out_obj, indent=2))


if __name__ == "__main__":
    main()
