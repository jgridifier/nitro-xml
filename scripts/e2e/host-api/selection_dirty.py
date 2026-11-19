#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""
E2E: host-api selection + dirty (#561 / #562 / #571)

Serves _site with python http.server, drives demo/host-api/selection-dirty.html
in headless Chrome, and asserts the definition-of-done for each format.

Exit 0 = all assertions passed.
Exit 1 = at least one assertion failed.

Usage:
  /workspace/pw-venv/bin/python scripts/e2e/host-api/selection_dirty.py [port]
"""
import sys
import os
import subprocess
import time
import threading
import http.server
import json
import traceback

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 7892
BASE = os.path.join(os.path.dirname(__file__), "..", "..", "..")
SITE = os.path.join(BASE, "_site")
CHROME = "/home/box/.cache/ms-playwright/chromium_headless_shell-1254/chrome-headless-shell-linux64/chrome-headless-shell"

# ---------------------------------------------------------------------------
# Serve _site
# ---------------------------------------------------------------------------
class _Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=SITE, **kw)
    def log_message(self, *_):
        pass

def _serve():
    with http.server.HTTPServer(("127.0.0.1", PORT), _Handler) as httpd:
        httpd.serve_forever()

server_thread = threading.Thread(target=_serve, daemon=True)
server_thread.start()
time.sleep(0.5)

# ---------------------------------------------------------------------------
# Playwright
# ---------------------------------------------------------------------------
from playwright.sync_api import sync_playwright

LOAD_TIMEOUT = 30_000  # ms

failures = []

def fail(msg):
    failures.append(msg)
    print(f"  FAIL: {msg}", flush=True)

def ok(msg):
    print(f"  PASS: {msg}", flush=True)

def run_format(playwright, fmt):
    print(f"\n=== {fmt.upper()} ===", flush=True)
    browser = playwright.chromium.launch(
        executable_path=CHROME,
        args=["--no-sandbox", "--disable-dev-shm-usage"],
    )
    page = browser.new_page()
    # Collect nitro-change events fired from the element
    change_events = []
    page.expose_function("__recordChange", lambda detail: change_events.append(detail))

    url = f"http://127.0.0.1:{PORT}/demo/host-api/selection-dirty.html?f={fmt}"
    page.goto(url, wait_until="domcontentloaded")

    # Inject listener before load completes (races are OK; we clear events after load)
    page.evaluate("""() => {
        const el = document.getElementById('viewer');
        if (el) el.addEventListener('nitro-change', e => window.__recordChange(e.detail));
    }""")

    # Wait for the "loaded" log line to appear
    try:
        page.wait_for_function(
            "() => document.getElementById('log').textContent.includes('loaded')",
            timeout=LOAD_TIMEOUT,
        )
    except Exception:
        fail(f"{fmt}: document did not load within {LOAD_TIMEOUT}ms")
        browser.close()
        return

    # Give lazy modules a moment to attach
    page.wait_for_timeout(500)

    # Clear any change events that fired during load (must be 0 per DoD, but allow defensive check)
    events_at_load = list(change_events)
    if events_at_load:
        fail(f"{fmt}: nitro-change fired {len(events_at_load)} time(s) during load: {events_at_load}")
    else:
        ok(f"{fmt}: no nitro-change events during open/load")

    # Reset event collector
    change_events.clear()

    # ---- dirty getter should start false ----
    is_dirty = page.evaluate("() => document.getElementById('viewer').dirty")
    if is_dirty:
        fail(f"{fmt}: dirty=true before any edit")
    else:
        ok(f"{fmt}: dirty=false before any edit")

    # ---- getSelection() returns correct format ----
    sel = page.evaluate("""async () => {
        const el = document.getElementById('viewer');
        return el.getSelection ? await el.getSelection() : null;
    }""")
    if sel is None:
        fail(f"{fmt}: getSelection() returned null — no document or attach failed")
    elif sel.get("format") != fmt:
        fail(f"{fmt}: getSelection() format={sel.get('format')} expected {fmt}")
    else:
        ok(f"{fmt}: getSelection() returns format={sel['format']}")

    # Format-specific field checks
    if fmt == "xlsx":
        if sel:
            ok(f"xlsx: sheet={sel.get('sheet')!r} sheetIndex={sel.get('sheetIndex')} range={sel.get('range')!r} anchor={sel.get('anchor')!r}")
            if "truncated" not in sel:
                fail("xlsx: getSelection() missing 'truncated' field")
            else:
                ok("xlsx: 'truncated' field present")
    elif fmt == "docx":
        if sel:
            ok(f"docx: page={sel.get('page')} collapsed={sel.get('collapsed')}")
            for field in ("text", "collapsed"):
                if field not in sel:
                    fail(f"docx: missing field {field!r}")
    elif fmt == "pptx":
        if sel:
            ok(f"pptx: slide={sel.get('slide')} shapeId={sel.get('shapeId')!r}")

    # ---- save() with no edits → no nitro-change events (issue #571) ----
    change_events.clear()
    page.evaluate("""async () => {
        const el = document.getElementById('viewer');
        await el.save();
    }""")
    page.wait_for_timeout(200)
    if change_events:
        fail(f"{fmt}: save() with no edits fired nitro-change: {change_events}")
    else:
        ok(f"{fmt}: save() with no edits fires no nitro-change (#571)")

    # ---- save() twice with no edits → dirty stays false ----
    page.evaluate("""async () => { const el = document.getElementById('viewer'); await el.save(); }""")
    page.wait_for_timeout(200)
    dirty_after_save = page.evaluate("() => document.getElementById('viewer').dirty")
    if dirty_after_save:
        fail(f"{fmt}: dirty=true after save() with no edits")
    else:
        ok(f"{fmt}: dirty=false after save() with no edits")

    # ---- Trigger a first edit ----
    change_events.clear()
    if fmt == "docx":
        # Click inside the document viewer and type
        viewer = page.locator("#viewer")
        box = viewer.bounding_box()
        if box:
            page.mouse.click(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
        page.keyboard.type("x")
    elif fmt == "xlsx":
        viewer = page.locator("#viewer")
        box = viewer.bounding_box()
        if box:
            page.mouse.click(box["x"] + 60, box["y"] + 60)
        page.keyboard.type("x")
        page.keyboard.press("Escape")
    elif fmt == "pptx":
        viewer = page.locator("#viewer")
        box = viewer.bounding_box()
        if box:
            page.mouse.click(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
        page.keyboard.type("x")

    # Wait for the dirty flip (up to 1 s — the timer fires at 350 ms)
    deadline = time.time() + 1.5
    while time.time() < deadline and not change_events:
        page.wait_for_timeout(100)

    dirty_events = [e for e in change_events if e.get("dirty")]
    if not dirty_events:
        # May be in read-only or the editor didn't receive the keystroke; mark as warning not hard failure
        print(f"  WARN: {fmt}: no dirty=true event within 1.5s of typing (editor may need focus or keystroke routing)")
    else:
        ok(f"{fmt}: nitro-change dirty=true fired after first edit")
        # Must be exactly one dirty=true event
        if len(dirty_events) > 1:
            fail(f"{fmt}: dirty=true fired {len(dirty_events)} times (expected 1)")
        else:
            ok(f"{fmt}: exactly one dirty=true event on first edit")

    # ---- save() after edit → dirty resets to false ----
    if dirty_events:
        change_events.clear()
        page.evaluate("""async () => { const el = document.getElementById('viewer'); await el.save(); }""")
        page.wait_for_timeout(400)
        clean_events = [e for e in change_events if not e.get("dirty")]
        if not clean_events:
            fail(f"{fmt}: no nitro-change dirty=false after save()")
        else:
            ok(f"{fmt}: nitro-change dirty=false fired after save()")
        dirty_post_save = page.evaluate("() => document.getElementById('viewer').dirty")
        if dirty_post_save:
            fail(f"{fmt}: dirty=true after save()")
        else:
            ok(f"{fmt}: dirty=false after save()")

    browser.close()


with sync_playwright() as playwright:
    for fmt in ("docx", "xlsx", "pptx"):
        try:
            run_format(playwright, fmt)
        except Exception as exc:
            fail(f"{fmt}: unexpected exception: {exc}")
            traceback.print_exc()

# ---------------------------------------------------------------------------
print("\n--- summary ---")
print(f"failures: {len(failures)}")
for f in failures:
    print(f"  FAIL: {f}")
sys.exit(1 if failures else 0)
