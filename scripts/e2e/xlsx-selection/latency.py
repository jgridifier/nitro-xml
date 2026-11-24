#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Playwright pointer-to-paint latency check for #612 on a 100k x 20 workbook."""
import functools
import http.server
import os
import sys
import tempfile
import threading
import time
import zipfile
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[3]
SITE = ROOT / "_site"


def _chrome_path():
    env = os.environ.get("HOST_API_CHROME")
    if env:
        return env
    for cand in (
        "/home/box/.cache/ms-playwright/chromium_headless_shell-1255/chrome-headless-shell-linux64/chrome-headless-shell",
        "/home/box/.cache/ms-playwright/chromium_headless_shell-1254/chrome-headless-shell-linux64/chrome-headless-shell",
    ):
        if os.path.exists(cand):
            return cand
    return None


def make_100k_xlsx(path: Path):
    cols = [chr(65 + i) for i in range(20)]  # A..T
    rows_xml = []
    for r in range(1, 100_001):
        cells = "".join(f'<c r="{c}{r}"><v>{(r * 20 + i) % 1000}</v></c>' for i, c in enumerate(cols))
        rows_xml.append(f'<row r="{r}">{cells}</row>')
    sheet_xml = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
        '<sheetViews><sheetView workbookViewId="0"/></sheetViews>'
        '<dimension ref="A1:T100000"/>'
        f'<sheetData>{"".join(rows_xml)}</sheetData>'
        '</worksheet>'
    )
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as z:
        z.writestr(
            "[Content_Types].xml",
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
            '<Default Extension="rels" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.relationships+xml"/>'
            '<Default Extension="xml" ContentType="application/xml"/>'
            '<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>'
            '<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
            "</Types>",
        )
        z.writestr(
            "_rels/.rels",
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>'
            "</Relationships>",
        )
        z.writestr(
            "xl/workbook.xml",
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
            'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
            '<sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>'
            "</workbook>",
        )
        z.writestr(
            "xl/_rels/workbook.xml.rels",
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>'
            "</Relationships>",
        )
        z.writestr("xl/worksheets/sheet1.xml", sheet_xml)


class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *_):
        pass


def percentile(sorted_vals, pct):
    if not sorted_vals:
        return 0.0
    idx = min(len(sorted_vals) - 1, int(pct * (len(sorted_vals) - 1)))
    return sorted_vals[idx]


def run_once(browser, port, xlsx_path, rate):
    page = browser.new_page(viewport={"width": 1440, "height": 900})
    try:
        cdp = page.context.new_cdp_session(page)
        if rate > 1:
            cdp.send("Emulation.setCPUThrottlingRate", {"rate": rate})
        page.goto(f"http://127.0.0.1:{port}/demo/xlsx-full.html")
        page.wait_for_function("() => document.querySelector('.ox input[type=file]') !== null", timeout=20000)
        page.wait_for_function("() => document.querySelector('.over')?.hidden === true", timeout=20000)
        page.locator('.ox input[type="file"]').set_input_files(str(xlsx_path))
        page.wait_for_function(
            """() => {
                const el = document.querySelector('nitro-xlsx');
                const name = document.querySelector('[data-doc]')?.textContent || '';
                return name.includes('gen_100k') && document.querySelector('.over')?.hidden === true && !!el?.shadowRoot?.querySelector('.xg-c');
            }""",
            timeout=60000,
        )
        page.wait_for_timeout(500)

        grid = page.evaluate("""() => {
            const sr = document.querySelector('nitro-xlsx').shadowRoot;
            const v = sr.querySelector('.v');
            const g = sr.querySelector('.xg-root').getBoundingClientRect();
            const c = sr.querySelector('.xg-corner').getBoundingClientRect();
            window.__lat = { samples: [], pending: null };
            v.addEventListener('pointermove', () => {
                if (window.__lat.pending === null) window.__lat.pending = performance.now();
            }, true);
            const target = sr.querySelector('.nx-sel-layer') || sr.querySelector('.dom');
            const mo = new MutationObserver(() => {
                if (window.__lat.pending === null) return;
                const overlay = sr.querySelector('.nx-sel-outline') || sr.querySelector('.xg-outline');
                if (overlay) {
                    const t0 = window.__lat.pending;
                    window.__lat.pending = null;
                    window.__lat.samples.push(performance.now() - t0);
                }
            });
            mo.observe(target, { childList: true, subtree: true, attributes: true, attributeFilter: ['style'] });
            return { gx: g.x, gy: g.y, hw: c.width, hh: c.height, gw: g.width, gh: g.height };
        }""")

        sx = grid["gx"] + grid["hw"] + 90
        sy = grid["gy"] + grid["hh"] + 30
        ex = grid["gx"] + grid["hw"] + 360
        ey = grid["gy"] + grid["gh"] + 60
        page.mouse.move(sx, sy)
        page.mouse.down()
        steps = 65
        for i in range(1, steps + 1):
            nx = sx + (ex - sx) * i / steps
            ny = sy + (ey - sy) * i / steps
            page.mouse.move(nx, ny)
            page.wait_for_timeout(16)
        page.mouse.up()
        page.wait_for_timeout(300)

        samples = page.evaluate("() => window.__lat.samples")
        samples.sort()
        assert len(samples) >= 30, f"expected >=30 measured drag frames at {rate}x, got {len(samples)}"
        p50 = percentile(samples, 0.50)
        p95 = percentile(samples, 0.95)
        mx = samples[-1]
        print(f"AFTER {rate}x: count={len(samples)} p50={p50:.2f}ms p95={p95:.2f}ms max={mx:.2f}ms", flush=True)
        return p50, p95, mx
    finally:
        page.close()


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 4602
    assert SITE.is_dir(), f"missing _site at {SITE}"
    with tempfile.TemporaryDirectory() as tmp:
        xlsx_path = Path(tmp) / "gen_100k_20.xlsx"
        t0 = time.time()
        make_100k_xlsx(xlsx_path)
        print(f"Generated 100k x 20 workbook ({xlsx_path.stat().st_size} B) in {time.time() - t0:.2f}s", flush=True)

        handler = functools.partial(Handler, directory=str(SITE))
        with http.server.ThreadingHTTPServer(("127.0.0.1", port), handler) as server:
            th = threading.Thread(target=server.serve_forever, daemon=True)
            th.start()
            try:
                chrome = _chrome_path()
                with sync_playwright() as p:
                    browser = p.chromium.launch(
                        **({} if not chrome else {"executable_path": chrome}),
                        args=["--no-sandbox", "--disable-dev-shm-usage"],
                    )
                    try:
                        _, p95_1x, _ = run_once(browser, port, xlsx_path, 1)
                        _, p95_4x, _ = run_once(browser, port, xlsx_path, 4)
                    finally:
                        browser.close()
            finally:
                server.shutdown()
                th.join()

    failures = []
    if p95_1x > 16.0:
        failures.append(f"1x p95 {p95_1x:.2f}ms > 16.0ms")
    if p95_4x > 16.0:
        failures.append(f"4x p95 {p95_4x:.2f}ms > 16.0ms")
    if failures:
        print("FAIL:", "; ".join(failures), flush=True)
        return 1
    print("PASS: pointer-to-paint p95 <= 16 ms at 1x and 4x", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
