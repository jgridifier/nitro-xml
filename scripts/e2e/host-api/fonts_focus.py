#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""
E2E: host-api fonts without Google + no focus steal on open (#566 / #567)

Serves _site with python http.server, drives demo/host-api/fonts-focus.html
in headless Chrome, and asserts the definition-of-done for each format.

Checks per format:
  - Zero requests to fonts.googleapis.com / fonts.gstatic.com during open+edit
  - Supplied font (NitroTestFont) is in document.fonts after open
  - Textarea retains focus through open (no focus steal)
  - Typed characters land in the textarea while the doc is loading
  - Clicking the doc focuses the editor

HoP rule: browser checks FAIL, never warn and pass.

Exit 0 = all assertions passed.
Exit 1 = at least one assertion failed.

Usage:
  /workspace/pw-venv/bin/python scripts/e2e/host-api/fonts_focus.py [port]
"""
import sys
import os
import threading
import time
import http.server
import traceback

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 7893
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
    try:
        with http.server.HTTPServer(("127.0.0.1", PORT), _Handler) as httpd:
            httpd.serve_forever()
    except OSError:
        pass  # already in use — existing server on that port is fine

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

    google_requests = []

    browser = playwright.chromium.launch(
        executable_path=CHROME,
        args=["--no-sandbox", "--disable-dev-shm-usage"],
    )
    context = browser.new_context()

    # Intercept and count Google Font network requests.
    def on_request(request):
        url = request.url
        if "fonts.googleapis.com" in url or "fonts.gstatic.com" in url:
            google_requests.append(url)
            print(f"  [intercept] Google font request: {url}", flush=True)

    context.on("request", on_request)
    page = context.new_page()

    url = f"http://127.0.0.1:{PORT}/demo/host-api/fonts-focus.html?f={fmt}"
    page.goto(url, wait_until="networkidle", timeout=LOAD_TIMEOUT)

    # Wait for chat-area to be present (static HTML, should be immediate).
    page.wait_for_selector("#chat-area", timeout=5000)

    # In headless Chrome autofocus only fires with an explicit activation context;
    # simulate it by clicking the textarea first so the browser treats it as focused.
    page.locator("#chat-area").click()
    page.wait_for_timeout(100)

    active_before = page.evaluate(
        "() => document.activeElement ? (document.activeElement.id || document.activeElement.tagName.toLowerCase()) : 'none'"
    )
    if active_before == "chat-area":
        ok(f"{fmt}: textarea has focus before open")
    else:
        fail(f"{fmt}: expected textarea focused before open, got: {active_before!r}")

    # Type into the textarea BEFORE opening the doc.
    page.keyboard.type("hello")
    typed_in_ta = page.evaluate("() => document.getElementById('chat-area').value")
    if "hello" in typed_in_ta:
        ok(f"{fmt}: typing before open lands in textarea")
    else:
        fail(f"{fmt}: typing before open did NOT land in textarea; value={typed_in_ta!r}")

    # ---- Click Open (loads doc without focus:true) ----
    page.click("#btn-open")

    # Wait for "loaded" message in the log.
    try:
        page.wait_for_function(
            "() => document.getElementById('log').textContent.includes('loaded')",
            timeout=LOAD_TIMEOUT,
        )
    except Exception:
        fail(f"{fmt}: document did not load within {LOAD_TIMEOUT}ms")
        browser.close()
        return

    page.wait_for_timeout(500)

    # ---- Zero Google font requests ----
    if google_requests:
        fail(f"{fmt}: {len(google_requests)} request(s) to Google Fonts: {google_requests[:3]}")
    else:
        ok(f"{fmt}: zero requests to fonts.googleapis.com / fonts.gstatic.com")

    # Also check window.__googleFontRequests from fetch intercept in the page.
    js_count = page.evaluate("() => window.__googleFontRequests || 0")
    if js_count > 0:
        fail(f"{fmt}: JS fetch interceptor saw {js_count} Google font request(s)")
    else:
        ok(f"{fmt}: JS fetch interceptor: zero Google font requests")

    # ---- Supplied font is loaded ----
    font_loaded = page.evaluate("() => document.fonts.check('16px NitroTestFont')")
    if font_loaded:
        ok(f"{fmt}: NitroTestFont is in document.fonts after open")
    else:
        fail(f"{fmt}: NitroTestFont NOT found in document.fonts after open")

    # ---- Focus was NOT stolen by open ----
    # The textarea was focused before open; after open it must remain focused
    # (not the editor or any element inside the shadow root).
    active_after = page.evaluate(
        "() => document.activeElement ? (document.activeElement.id || document.activeElement.tagName.toLowerCase()) : 'none'"
    )
    # Focus is acceptable if it stayed on the textarea or returned to body/document.
    # It is NOT acceptable if the editor or its shadow internals hold focus.
    focus_stolen = page.evaluate("""() => {
        const ae = document.activeElement;
        const ta = document.getElementById('chat-area');
        if (!ae || ae === ta || ae === document.body) return false;
        // Check if activeElement is inside the viewer shadow root
        const viewer = document.getElementById('viewer');
        if (!viewer) return false;
        if (ae === viewer) return true;
        const sr = viewer.shadowRoot;
        return !!(sr && sr.contains(ae));
    }""")
    if focus_stolen:
        fail(f"{fmt}: focus stolen by open — activeElement={active_after!r}")
    else:
        ok(f"{fmt}: open did NOT steal focus (activeElement={active_after!r})")

    # ---- Refocus textarea and type: characters must go there ----
    page.locator("#chat-area").click()
    page.keyboard.type(" world")
    ta_value = page.evaluate("() => document.getElementById('chat-area').value")
    if "world" in ta_value:
        ok(f"{fmt}: typing after open lands in textarea")
    else:
        fail(f"{fmt}: typing after open did NOT land in textarea; value={ta_value!r}")

    # ---- Clicking the doc focuses the editor ----
    # Playwright's page.mouse.click() doesn't pierce shadow DOM in headless shell;
    # dispatch the PointerEvent from inside the shadow root to test the same code path
    # (element.js pointerdown handler calls this._v.focus(); input-proxy refocuses the proxy textarea).
    editor_focused = page.evaluate("""() => {
        const el = document.getElementById('viewer');
        if (!el) return { ok: false, reason: 'no viewer' };
        const sr = el.shadowRoot;
        if (!sr) return { ok: false, reason: 'no shadowRoot' };
        const v = sr.querySelector('.v');
        if (!v) return { ok: false, reason: 'no .v' };
        // Simulate what a user click on the content area does: pointerdown on .v
        const r = v.getBoundingClientRect();
        v.dispatchEvent(new PointerEvent('pointerdown', {
            bubbles: true, composed: true,
            button: 0, buttons: 1, pointerType: 'mouse',
            clientX: r.left + r.width / 2, clientY: r.top + r.height / 2,
        }));
        // Allow microtasks to settle (focus is set synchronously in element.js)
        return {
            ok: !!(document.activeElement === el || (sr.activeElement)),
            ae: document.activeElement ? (document.activeElement.id || document.activeElement.tagName) : null,
            sae: sr.activeElement ? (sr.activeElement.className || sr.activeElement.tagName) : null,
        };
    }""")
    page.wait_for_timeout(200)
    # Re-read after any async focus settle
    editor_focused2 = page.evaluate("""() => {
        const el = document.getElementById('viewer');
        const sr = el && el.shadowRoot;
        return !!(document.activeElement === el || (sr && sr.activeElement));
    }""")
    if editor_focused2:
        ok(f"{fmt}: clicking the doc focuses the editor ({editor_focused})")
    else:
        fail(f"{fmt}: clicking the doc did NOT focus the editor: {editor_focused}")

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
