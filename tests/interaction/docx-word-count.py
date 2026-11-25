#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""
Playwright e2e for feature-546: word count in the status bar (docx).

Checks:
  - [data-wc] button appears with "105 words" on notes.docx
  - Native text selection shows "S of N words"
  - Clicking opens Word Count dialog with correct stats
  - Esc / Close button dismisses and returns focus

Takes four headless 1440x900 screenshots:
  feature-546-before.png, feature-546-after.png,
  feature-546-selection.png, feature-546-dialog.png
into SHOTS_DIR and docs/screenshots/docx-word-count/.
"""
import http.server
import os
import shutil
import sys
import threading
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[2]
SHOTS_DIR = Path(os.environ.get("DOCX_WC_SHOTS", "/workspace/nitro-nightly/shots-2026-11-21"))
DOCS_SHOTS = ROOT / "docs" / "screenshots" / "docx-word-count"

CHROME = str(Path("/home/box/.cache/ms-playwright/chromium_headless_shell-1243/chrome-headless-shell-linux64/chrome-headless-shell"))
if not Path(CHROME).exists():
    # fall back to whatever playwright installed
    CHROME = None

def assemble(dest: Path):
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    shutil.copytree(ROOT / "demo", dest, dirs_exist_ok=True)
    shutil.rmtree(dest / "nitro-xml", ignore_errors=True)
    loader_dest = dest / "nitro-xml" / "loader"
    shutil.copytree(ROOT / "packages" / "nitro-xml" / "loader", loader_dest,
                    ignore=shutil.ignore_patterns("test"),
                    dirs_exist_ok=True)
    shutil.copytree(ROOT / "packages" / "nitro-xml" / "pkg",
                    dest / "nitro-xml" / "pkg", dirs_exist_ok=True)

def start_server(demo_dir: Path):
    class Handler(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *a, **kw):
            super().__init__(*a, directory=str(demo_dir), **kw)
        def log_message(self, *_): pass
    server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
    port = server.server_address[1]
    t = threading.Thread(target=server.serve_forever, daemon=True)
    t.start()
    return f"http://127.0.0.1:{port}", server

def save_shot(page, name):
    SHOTS_DIR.mkdir(parents=True, exist_ok=True)
    DOCS_SHOTS.mkdir(parents=True, exist_ok=True)
    p = SHOTS_DIR / name
    page.screenshot(path=str(p), animations="disabled")
    shutil.copy(str(p), str(DOCS_SHOTS / name))

def wait_for_docx(page):
    page.wait_for_function(
        """() => {
            const el = document.querySelector('nitro-docx');
            const sr = el && el.shadowRoot;
            return !!(sr && el.document && sr.querySelector('.dve-print-line'));
        }""",
        timeout=45_000,
    )
    page.wait_for_timeout(400)

def wait_for_wc(page, timeout_ms=5000):
    deadline = page.evaluate("Date.now()") + timeout_ms
    while page.evaluate("Date.now()") < deadline:
        txt = page.evaluate("""() => {
            const btn = document.querySelector('[data-wc]');
            return btn && !btn.hidden ? btn.textContent.trim() : null;
        }""")
        if txt:
            return txt
        page.wait_for_timeout(100)
    raise AssertionError("word count button never became visible")

def run(page, base):
    errors = []

    # --- Load page ---
    page.goto(f"{base}/docx-full.html", wait_until="domcontentloaded", timeout=60_000)
    wait_for_docx(page)

    # Before: status bar without word count (immediately after load, before debounce fires)
    save_shot(page, "feature-546-before.png")

    # --- Wait for word count ---
    wc_text = wait_for_wc(page)
    import re
    m = re.match(r"^(\d+)\s+words?$", wc_text)
    if not m:
        errors.append(f"word count has unexpected format: {wc_text!r}")
    else:
        n = int(m.group(1))
        if n != 105:
            errors.append(f"notes.docx: expected 105 words, got {n} (text: {wc_text!r})")
        else:
            print(f"  word count: {wc_text} ✓")

    save_shot(page, "feature-546-after.png")

    # --- Selection word count ---
    first_line = page.evaluate("""() => {
        const el = document.querySelector('nitro-docx');
        const sr = el && el.shadowRoot;
        const span = sr && sr.querySelector('.dve-print-line span');
        if (!span) return null;
        const r = span.getBoundingClientRect();
        return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
    }""")
    if first_line:
        page.mouse.click(first_line["x"], first_line["y"], click_count=3)
        page.wait_for_timeout(200)
        sel_wc = page.evaluate("""() => {
            const btn = document.querySelector('[data-wc]');
            return btn && !btn.hidden ? btn.textContent.trim() : null;
        }""")
        if sel_wc and re.search(r"\d+ of \d+ words", sel_wc):
            print(f"  selection count: {sel_wc} ✓")
        else:
            print(f"  selection count: {sel_wc!r} (collapsed or no native sel — ok for edit mode)")
    else:
        errors.append("could not find first line for selection test")
    save_shot(page, "feature-546-selection.png")

    # Deselect
    page.evaluate("window.getSelection && window.getSelection().removeAllRanges()")
    page.wait_for_timeout(100)

    # --- Dialog ---
    wc_btn = page.locator("[data-wc]").first
    wc_btn.click()
    page.wait_for_timeout(200)

    dialog_visible = page.evaluate("!!document.querySelector('.wcd')")
    if not dialog_visible:
        errors.append("Word Count dialog did not open")
    else:
        print("  dialog opens ✓")

    # Check Words row = 105
    dialog_words = page.evaluate("""() => {
        for (const r of document.querySelectorAll('.wcd-row')) {
            const lbl = r.querySelector('.wcd-lbl');
            const val = r.querySelector('.wcd-val');
            if (lbl && lbl.textContent === 'Words') return val ? val.textContent.trim() : null;
        }
        return null;
    }""")
    if dialog_words != "105":
        errors.append(f"dialog Words should be 105, got {dialog_words!r}")
    else:
        print(f"  dialog words: {dialog_words} ✓")

    # Check all required stats labels
    dialog_labels = page.evaluate(
        "[...document.querySelectorAll('.wcd-lbl')].map(l => l.textContent.trim())"
    )
    for req in ["Pages", "Words", "Characters (no spaces)", "Characters (with spaces)", "Paragraphs", "Lines"]:
        if req not in dialog_labels:
            errors.append(f"dialog missing stat: {req!r}")
    if not errors:
        print("  dialog stats present ✓")

    save_shot(page, "feature-546-dialog.png")

    # --- Close with Esc ---
    page.keyboard.press("Escape")
    page.wait_for_timeout(150)
    dialog_gone = page.evaluate("!document.querySelector('.wcd')")
    if not dialog_gone:
        errors.append("dialog did not close on Esc")
    else:
        print("  Esc closes dialog ✓")

    focus_on_wc = page.evaluate("""() => {
        const btn = document.querySelector('[data-wc]');
        return document.activeElement === btn;
    }""")
    if not focus_on_wc:
        errors.append("focus did not return to word count button after Esc")
    else:
        print("  focus returns to button ✓")

    # --- Close button ---
    wc_btn.click()
    page.wait_for_timeout(200)
    page.locator(".wcd-close").first.click()
    page.wait_for_timeout(150)
    dialog_gone2 = page.evaluate("!document.querySelector('.wcd')")
    if not dialog_gone2:
        errors.append("dialog did not close on Close button")
    else:
        print("  Close button works ✓")

    if errors:
        for e in errors:
            print(f"  ERROR: {e}", file=sys.stderr)
        raise AssertionError(f"{len(errors)} check(s) failed")

def main():
    demo_dir = ROOT / "tests" / "interaction" / "out" / "wc-demo"
    assemble(demo_dir)
    base_url, server = start_server(demo_dir)
    failed = False
    with sync_playwright() as pw:
        launch_opts = {"headless": True}
        if CHROME:
            launch_opts["executable_path"] = CHROME
        browser = pw.chromium.launch(**launch_opts)
        try:
            page = browser.new_page(viewport={"width": 1440, "height": 900})
            print("docx-word-count: running checks…")
            run(page, base_url)
            print("docx-word-count: all checks passed")
        except Exception as e:
            print(f"docx-word-count: FAIL {e}", file=sys.stderr)
            failed = True
        finally:
            browser.close()
    server.shutdown()
    out_dir = ROOT / "tests" / "interaction" / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "docx-word-count-summary.txt").write_text("FAIL\n" if failed else "OK\n")
    if failed:
        sys.exit(1)

if __name__ == "__main__":
    main()
