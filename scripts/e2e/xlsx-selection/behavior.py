#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Playwright behaviour checks + after-screenshots for #612."""
import functools
import http.server
import io
import json
import os
import re
import sys
import threading
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[3]
SITE = ROOT / "_site"
SHOTS = Path("/workspace/nitro-nightly/shots-agy-612")


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


class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *_):
        pass


def open_xlsx(browser, port):
    page = browser.new_page(viewport={"width": 1440, "height": 900})
    page.goto(f"http://127.0.0.1:{port}/demo/xlsx-full.html")
    page.wait_for_function(
        "() => { const el = document.querySelector('nitro-xlsx'); return el && el.shadowRoot && el.shadowRoot.querySelector('.xg-c') && el._nxSel; }",
        timeout=20000,
    )
    page.wait_for_function("() => document.querySelector('.over')?.hidden === true", timeout=20000)
    page.wait_for_timeout(400)
    return page


def grid_geom(page):
    return page.evaluate("""() => {
        const sr = document.querySelector('nitro-xlsx').shadowRoot;
        const g = sr.querySelector('.xg-root').getBoundingClientRect();
        const c = sr.querySelector('.xg-corner').getBoundingClientRect();
        return { gx: g.x, gy: g.y, gw: g.width, gh: g.height, hw: c.width, hh: c.height };
    }""")


def cell_xy(g, row, col):
    return g["gx"] + g["hw"] + col * 64 + 32, g["gy"] + g["hh"] + row * 20 + 10


def col_hdr_xy(g, col):
    return g["gx"] + g["hw"] + col * 64 + 32, g["gy"] + g["hh"] / 2


def row_hdr_xy(g, row):
    return g["gx"] + g["hw"] / 2, g["gy"] + g["hh"] + row * 20 + 10


def name_box(page):
    return page.evaluate("() => document.querySelector('nitro-xlsx').shadowRoot.querySelector('.xfb-name')?.textContent || ''")


def check_all(browser, port, save_shots):
    if save_shots:
        SHOTS.mkdir(parents=True, exist_ok=True)

    # 1. Shaded range + active cell white + Name Box + size tip + ribbon <= 2 rows
    page = open_xlsx(browser, port)
    try:
        g = grid_geom(page)
        x0, y0 = cell_xy(g, 11, 1)  # B12
        x1, y1 = cell_xy(g, 18, 4)  # E19
        page.mouse.move(x0, y0)
        page.mouse.down()
        page.mouse.move(x1, y1, steps=12)
        page.wait_for_timeout(80)
        tip_text = page.evaluate("""() => {
            const t = document.querySelector('nitro-xlsx').shadowRoot.querySelector('.nx-sel-tip');
            return t && !t.hidden ? t.textContent : '';
        }""")
        assert tip_text == "8R × 4C", f"expected size tip '8R × 4C', got {tip_text!r}"
        page.mouse.up()
        page.wait_for_timeout(150)

        assert name_box(page) == "B12:E19", f"expected Name Box 'B12:E19', got {name_box(page)!r}"
        shade_info = page.evaluate("""() => {
            const sr = document.querySelector('nitro-xlsx').shadowRoot;
            const shades = [...sr.querySelectorAll('.nx-sel-shade')].map(n => n.getBoundingClientRect());
            const active = sr.querySelector('.nx-sel-active')?.getBoundingClientRect();
            const outline = sr.querySelector('.nx-sel-outline')?.getBoundingClientRect();
            const overlapsActive = active ? shades.some(s =>
                s.left < active.right - 1 && s.right > active.left + 1 &&
                s.top < active.bottom - 1 && s.bottom > active.top + 1
            ) : true;
            return { shadeCount: shades.length, hasOutline: !!outline, hasActive: !!active, overlapsActive };
        }""")
        assert shade_info["shadeCount"] >= 2, f"expected >=2 shade rects around active cell, got {shade_info}"
        assert shade_info["hasOutline"] and shade_info["hasActive"], f"missing outline/active: {shade_info}"
        assert not shade_info["overlapsActive"], f"shade must leave active cell white: {shade_info}"
        if save_shots:
            page.screenshot(path=str(SHOTS / "after-b12-e19.png"))
        print("  PASS: shaded range B12:E19 with active cell white, Name Box, and size tip '8R × 4C'", flush=True)

        # 2. Ctrl+click + Ctrl+drag second range G14:H17 with size tip visible
        gx0, gy0 = cell_xy(g, 13, 6)  # G14
        gx1, gy1 = cell_xy(g, 16, 7)  # H17
        page.keyboard.down("Control")
        page.mouse.move(gx0, gy0)
        page.mouse.down()
        page.mouse.move(gx1, gy1, steps=10)
        page.wait_for_timeout(80)
        tip2 = page.evaluate("""() => {
            const t = document.querySelector('nitro-xlsx').shadowRoot.querySelector('.nx-sel-tip');
            return t && !t.hidden ? t.textContent : '';
        }""")
        assert tip2 == "4R × 2C", f"expected second-range tip '4R × 2C', got {tip2!r}"
        if save_shots:
            page.screenshot(path=str(SHOTS / "after-multi-range-tip.png"))
        page.mouse.up()
        page.keyboard.up("Control")
        page.wait_for_timeout(200)

        sel = page.evaluate("() => document.querySelector('nitro-xlsx').getSelection()")
        assert sel["range"] == "B12:E19,G14:H17", f"expected multi-range 'B12:E19,G14:H17', got {sel['range']!r}"
        assert len(sel["ranges"]) == 2, f"expected 2 ranges in getSelection(), got {len(sel['ranges'])}"
        assert name_box(page) == "B12:E19,G14:H17", f"expected multi-range Name Box, got {name_box(page)!r}"

        # Ribbon group row check (<= 2 rows)
        ribbon_rows = page.evaluate("""() => {
            const groups = [...document.querySelectorAll('.ox .rg')];
            const bad = [];
            for (const rg of groups) {
                const label = rg.querySelector('.gl')?.textContent || '';
                const rows = rg.querySelectorAll('.col2 .r1').length;
                if (rows > 2) bad.push({ label, rows });
            }
            return bad;
        }""")
        assert not ribbon_rows, f"ribbon groups exceeded 2 rows: {ribbon_rows}"

        # Non-rectangular multi-range Copy shows clear non-silent notice
        page.locator('.ox .rbn button[aria-label="Copy"]').click()
        page.wait_for_timeout(150)
        toast = page.evaluate("""() => {
            const t = document.querySelector('nitro-xlsx').shadowRoot.querySelector('.nx-sel-toast');
            return t && !t.hidden ? t.textContent : '';
        }""")
        assert "multiple selections" in toast.lower(), f"expected non-silent copy notice, got {toast!r}"

        # Bold + percent format apply to every range
        page.locator('.ox .rbn button[aria-label="Bold"]').click()
        page.wait_for_timeout(300)
        page.locator('.ox .rbn button[aria-label="Percent style"]').click()
        page.wait_for_timeout(300)
        saved_bytes = bytes(page.evaluate("async () => Array.from(await document.querySelector('nitro-xlsx').save())"))
        with zipfile.ZipFile(io.BytesIO(saved_bytes)) as zf:
            styles_xml = zf.read("xl/styles.xml").decode("utf-8")
            sheet1_xml = zf.read("xl/worksheets/sheet1.xml").decode("utf-8")
        for cell_ref in ("B12", "E19", "G14", "H17"):
            m = re.search(rf'<c r="{cell_ref}"[^>]*s="(\d+)"', sheet1_xml)
            assert m, f"expected style on {cell_ref} in saved sheet1.xml"
            assert m.group(1) != "0", f"expected non-default Bold+Percent style on {cell_ref}, got s={m.group(1)}"
        assert 'numFmtId="9"' in styles_xml and "<b/>" in styles_xml, "expected Bold (<b/>) and Percent (numFmtId=9) in saved styles.xml"

        # Delete clears every range (verify both painted DOM and saved workbook)
        page.keyboard.press("Delete")
        page.wait_for_timeout(500)
        dom_vals = page.evaluate("""() => {
            const sr = document.querySelector('nitro-xlsx').shadowRoot;
            return [...sr.querySelectorAll('.xg-c')].filter(c => {
                const l = parseFloat(c.style.left), t = parseFloat(c.style.top);
                const in1 = l >= 63 && l <= 257 && t >= 219 && t <= 361; // B12:E19
                const in2 = l >= 383 && l <= 449 && t >= 259 && t <= 321; // G14:H17
                return (in1 || in2) && (c.textContent || '').trim() !== '';
            }).map(c => c.textContent);
        }""")
        assert not dom_vals, f"expected B12:E19 and G14:H17 DOM cells to be cleared, got {dom_vals}"
        saved_after_del = bytes(page.evaluate("async () => Array.from(await document.querySelector('nitro-xlsx').save())"))
        with zipfile.ZipFile(io.BytesIO(saved_after_del)) as zf:
            sheet_after_del = zf.read("xl/worksheets/sheet1.xml").decode("utf-8")
        root_xml = ET.fromstring(sheet_after_del)
        ns = {"s": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
        by_ref = {c.attrib.get("r"): c for c in root_xml.findall(".//s:c", ns)}
        for cell_ref in ("B12", "E19", "G14", "H17"):
            c_el = by_ref.get(cell_ref)
            v_el = c_el.find("s:v", ns) if c_el is not None else None
            assert v_el is None, f"expected {cell_ref} value cleared in saved sheet1.xml, got {ET.tostring(c_el, encoding='unicode')}"
        print("  PASS: Ctrl+click multi-range, getSelection(), copy notice, Bold/Percent, Delete clears all ranges", flush=True)
    finally:
        page.close()

    # 3. Header selection, Shift+click header, header drag, corner button, Ctrl+A
    page = open_xlsx(browser, port)
    try:
        g = grid_geom(page)
        page.mouse.click(*col_hdr_xy(g, 2))  # Col C
        page.wait_for_timeout(120)
        assert name_box(page) == "C:C", f"expected 'C:C', got {name_box(page)!r}"
        if save_shots:
            page.screenshot(path=str(SHOTS / "after-col-header.png"))

        page.keyboard.down("Shift")
        page.mouse.click(*col_hdr_xy(g, 4))  # Shift+click Col E -> C:E
        page.keyboard.up("Shift")
        page.wait_for_timeout(120)
        assert name_box(page) == "C:E", f"expected 'C:E' after Shift+click, got {name_box(page)!r}"

        # Drag across headers B..D
        bx, by = col_hdr_xy(g, 1)
        dx, dy = col_hdr_xy(g, 3)
        page.mouse.move(bx, by)
        page.mouse.down()
        page.mouse.move(dx, dy, steps=6)
        page.mouse.up()
        page.wait_for_timeout(120)
        assert name_box(page) == "B:D", f"expected 'B:D' after col header drag, got {name_box(page)!r}"

        # Row header click + Shift+click
        page.mouse.click(*row_hdr_xy(g, 2))  # Row 3
        page.wait_for_timeout(120)
        assert name_box(page) == "3:3", f"expected '3:3', got {name_box(page)!r}"
        page.keyboard.down("Shift")
        page.mouse.click(*row_hdr_xy(g, 5))  # Row 6 -> 3:6
        page.keyboard.up("Shift")
        page.wait_for_timeout(120)
        assert name_box(page) == "3:6", f"expected '3:6' after Shift+click row, got {name_box(page)!r}"

        # Corner button select all
        page.mouse.click(g["gx"] + g["hw"] / 2, g["gy"] + g["hh"] / 2)
        page.wait_for_timeout(120)
        assert name_box(page) == "1:1048576", f"expected '1:1048576' from corner click, got {name_box(page)!r}"

        # Click empty cell U1 (col 20) and Ctrl+A -> select all
        page.mouse.click(*cell_xy(g, 0, 1))  # B1 (in region)
        page.wait_for_timeout(120)
        page.keyboard.press("Control+a")
        page.wait_for_timeout(200)
        nb_after_1 = name_box(page)
        page.keyboard.press("Control+a")
        page.wait_for_timeout(200)
        nb_after_2 = name_box(page)
        assert nb_after_2 == "1:1048576" or nb_after_1 == "1:1048576", f"expected Ctrl+A select all, got {nb_after_1!r} -> {nb_after_2!r}"
        print("  PASS: column/row header click, drag, Shift+click, corner button, Ctrl+A", flush=True)

        # 4. Edge auto-scroll
        page.mouse.click(*cell_xy(g, 2, 1))  # B3
        page.wait_for_timeout(100)
        st0 = page.evaluate("() => document.querySelector('nitro-xlsx').getPosition().scrollTop")
        page.mouse.move(*cell_xy(g, 2, 1))
        page.mouse.down()
        # Hold below bottom edge of the grid
        page.mouse.move(g["gx"] + g["hw"] + 90, g["gy"] + g["gh"] + 50, steps=6)
        page.wait_for_timeout(500)
        st1 = page.evaluate("() => document.querySelector('nitro-xlsx').getPosition().scrollTop")
        assert st1 > st0, f"expected edge auto-scroll to increase scrollTop ({st0} -> {st1})"
        # Move back inside and confirm scrolling stops
        page.mouse.move(g["gx"] + g["hw"] + 90, g["gy"] + g["hh"] + 60, steps=4)
        page.wait_for_timeout(150)
        st2 = page.evaluate("() => document.querySelector('nitro-xlsx').getPosition().scrollTop")
        page.wait_for_timeout(350)
        st3 = page.evaluate("() => document.querySelector('nitro-xlsx').getPosition().scrollTop")
        page.mouse.up()
        assert st3 == st2, f"expected auto-scroll to stop when pointer returned inside ({st2} -> {st3})"
        print(f"  PASS: edge auto-scroll ({st0} -> {st1}, stopped at {st3})", flush=True)
    finally:
        page.close()


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 4603
    save_shots = "--screenshots" in sys.argv
    assert SITE.is_dir(), f"missing _site at {SITE}"
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
                    check_all(browser, port, save_shots)
                finally:
                    browser.close()
        finally:
            server.shutdown()
            th.join()
    print("PASS: all #612 behaviour checks", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
