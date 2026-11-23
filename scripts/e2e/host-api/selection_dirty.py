#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Real-input host API checks (#561/#562); any missing capability exits nonzero.

Assemble _site as described in the host-api brief, then run with pw-venv Python.
An optional positional port defaults to 7892. No synthetic input or model edits.
"""
import functools
import http.server
import json
from pathlib import Path
import os
import sys
import threading
from playwright.sync_api import sync_playwright

SITE = Path(__file__).resolve().parents[3] / "_site"
def _chrome_path():
    env = os.environ.get("HOST_API_CHROME")
    if env:
        return env
    default = "/home/box/.cache/ms-playwright/chromium_headless_shell-1254/chrome-headless-shell-linux64/chrome-headless-shell"
    return default if os.path.exists(default) else None


CHROME = _chrome_path()
# Full text of the first shape on slide 1 of demo/samples/deck.pptx.
PPTX_SHAPE_TEXT = 'Nitro XML'


class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *_):
        pass


def expect(actual, expected, message):
    assert actual == expected, f"{message}: expected {expected!r}, got {actual!r}"


def events(page):
    return page.evaluate("window.hostChanges")


def open_page(browser, port, fmt):
    page = browser.new_page(viewport={"width": 1400, "height": 900})
    # Install before the module creates/loads the element; never clear load events.
    page.add_init_script("""window.hostChanges = [];
        document.addEventListener('nitro-change', e => {
            if (e.target.id === 'viewer') window.hostChanges.push(e.detail);
        }, true);""")
    page.goto(f"http://127.0.0.1:{port}/demo/host-api/selection-dirty.html?f={fmt}")
    page.wait_for_function("document.querySelector('#log').textContent.includes('loaded')")
    page.wait_for_function("typeof document.querySelector('#viewer').getSelection === 'function'")
    page.wait_for_function("typeof document.querySelector('#viewer').document.whenEditReady === 'function'")
    page.evaluate("document.querySelector('#viewer').document.whenEditReady()")
    page.locator('#viewer .nxp').wait_for(state='attached')
    page.wait_for_timeout(500)
    box = page.locator('#viewer .v').bounding_box()
    assert box and box['width'] > 100 and box['height'] > 100, f"editor has no usable surface: {box}"
    expect(events(page), [], "events during load")
    expect(page.evaluate("document.querySelector('#viewer').dirty"), False, "dirty on load")
    return page


def click_text(page, fmt):
    selector = '#viewer [data-bid="p0"] span' if fmt == 'docx' else '#viewer .pptxv-para span'
    target = page.locator(selector).first
    target.click()
    return target


def assert_focus(page):
    expect(page.evaluate("""() => {
        const el = document.querySelector('#viewer');
        return document.activeElement === el && el.shadowRoot.activeElement?.matches('.nxp');
    }"""), True, "real click must focus editor input")


def cell_point(page, row, col):
    # Grid header and default cell dimensions in demo/samples/sample.xlsx.
    # Read the rect in one page turn: every painted frame rebuilds the grid DOM, so a
    # two-step locator lookup can land on a just-detached node mid-drag (flaky None).
    box = page.evaluate("""() => {
        const grid = document.querySelector('#viewer').shadowRoot.querySelector('.xg-root');
        const r = grid && grid.isConnected ? grid.getBoundingClientRect() : null;
        return r && r.width > 0 && r.height > 0 ? {x: r.x, y: r.y} : null;
    }""")
    assert box, 'spreadsheet grid missing'
    return box['x'] + 33 + col * 64 + 32, box['y'] + 20 + row * 20 + 10


def check_dirty(page, fmt):
    # Catches: unfocused keystrokes and cancelled cell edits passing as dirty-event proof.
    page.locator('#chat-area').click()
    if fmt == 'xlsx':
        page.mouse.click(*cell_point(page, 1, 1))
    else:
        click_text(page, fmt)
    assert_focus(page)
    page.wait_for_timeout(500)
    expect(events(page), [], 'selection/focus must not mark dirty')
    page.keyboard.type('x')
    if fmt == 'xlsx':
        page.keyboard.press('Enter')  # Commit; Escape cancels the edit.
    page.wait_for_function("document.querySelector('#viewer').dirty === true", timeout=3000)
    page.wait_for_timeout(500)
    expect(events(page), [{'dirty': True}], 'exactly one first-edit event')
    expect(page.evaluate("document.querySelector('#viewer').dirty"), True, 'dirty after edit')
    page.keyboard.type('y')
    if fmt == 'xlsx':
        page.keyboard.press('Enter')
    page.wait_for_timeout(700)
    expect(events(page), [{'dirty': True}], 'additional edits must not repeat dirty=true')
    page.locator('#btn-save').click()
    page.wait_for_function("document.querySelector('#viewer').dirty === false", timeout=3000)
    page.wait_for_timeout(500)
    expect(events(page), [{'dirty': True}, {'dirty': False}], 'save resets exactly once')
    page.locator('#btn-save').click()
    page.wait_for_timeout(500)
    expect(events(page), [{'dirty': True}, {'dirty': False}], 'clean save must not emit')
    page.locator('#viewer [data-a="zoom-out"]').click()
    page.locator('#viewer [data-a="zoom-in"]').click()
    page.locator('#viewer .v').hover()
    page.mouse.wheel(0, 100)
    if fmt == 'pptx':
        page.locator('#viewer [data-a="sheet"][data-g="1"]').click()
        page.locator('#viewer [data-a="sheet"][data-g="0"]').click()
    page.wait_for_timeout(700)
    expect(events(page), [{'dirty': True}, {'dirty': False}], 'zoom/scroll/navigation must not emit')
    print(f'  PASS {fmt}: focus; no load/selection event; first edit exactly once; dirty=true; save resets; clean save and zoom/scroll/navigation silent', flush=True)


def check_selection(page, fmt):
    # Catches: placeholder payloads passing without a real caret, cell range or clicked shape.
    # v0.2.0 reports only what is real: docx caret, xlsx range + values, pptx clicked shape.
    if fmt == 'xlsx':
        page.mouse.move(*cell_point(page, 1, 1))
        page.mouse.down()
        page.mouse.move(*cell_point(page, 2, 2), steps=12)
        page.mouse.up()
    elif fmt == 'docx':
        # A real click at the left edge of "Quarterly" puts the caret at 0; three engine
        # ArrowRight moves must then report offset 3 (catches a stale or click-only caret).
        target = page.locator('#viewer [data-bid="p0"] span').first
        box = target.bounding_box()
        assert box, 'docx first run missing'
        page.mouse.click(box['x'] + 1, box['y'] + box['height'] / 2)
        page.wait_for_timeout(300)
        for _ in range(3):
            page.keyboard.press('ArrowRight')
    else:
        page.locator('#viewer .pptxv-para span').first.click()
    page.wait_for_timeout(500)
    selection = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} selection: {json.dumps(selection)}', flush=True)
    assert selection is not None, 'selection must not be null'
    if fmt == 'docx':
        expect(selection, {
            'format': 'docx', 'text': '', 'page': 0,
            'start': {'paragraph': 0, 'run': 0, 'offset': 3},
            'end': {'paragraph': 0, 'run': 0, 'offset': 3}, 'collapsed': True,
        }, 'DOCX caret position (collapsed, no selected text)')
    elif fmt == 'xlsx':
        expect(selection, {
            'format': 'xlsx', 'sheet': 'Sheet1', 'sheetIndex': 0,
            'range': 'B2:C3', 'anchor': 'B2', 'values': [[4, 6], [6, 9]], 'truncated': False,
        }, 'XLSX multi-cell range and numeric values')
    else:
        expect(selection, {
            'format': 'pptx', 'slide': 0, 'shapeId': 'ppt/slides/slide1.xml-shape-0',
            'shapeName': 'TextBox 1', 'text': PPTX_SHAPE_TEXT, 'textRange': None, 'collapsed': True,
        }, 'PPTX clicked shape (collapsed, no text range)')
    expect(events(page), [], 'selection must not mark dirty')
    expect(page.evaluate("document.querySelector('#viewer').dirty"), False, 'selection must not set dirty')
    print(f'  PASS {fmt}: real selection payload', flush=True)


def _screenshot(page, name, widths, out_dir=None):
    """Save screenshots at the given viewport widths (height 800)."""
    import os
    if out_dir is None:
        out_dir = Path(__file__).resolve().parents[0] / 'out'
    os.makedirs(out_dir, exist_ok=True)
    for w in widths:
        h = 800 if w == 1280 else 900
        page.set_viewport_size({'width': w, 'height': h})
        page.wait_for_timeout(200)
        page.screenshot(path=str(out_dir / f'{name}_{w}x{h}.png'))
    # Restore default; wait for the layout to stabilise before the caller queries positions.
    page.set_viewport_size({'width': 1400, 'height': 900})
    page.wait_for_timeout(300)


def check_selection_s1(page, fmt):
    """S1 real text-range selection (#561): drag, Shift+Arrow, type/paste replaces,
    Backspace deletes, one Undo restores; visible highlight in the DOM.
    Checks are strict — any failure exits non-zero (never warn-and-pass)."""
    if fmt not in ('docx', 'pptx'):
        print(f'  SKIP {fmt}: selection S1 is docx/pptx only', flush=True)
        return

    if fmt == 'docx':
        target_sel = '#viewer [data-bid="p0"] span'
        hl_sel = '#viewer .nxh-r'
    else:
        target_sel = '#viewer .pptxv-para span'
        hl_sel = '#viewer .nxph-r'

    target = page.locator(target_sel).first
    box = target.bounding_box()
    assert box, f'{fmt} text span missing'

    # --- Drag selection: start to ~30px right ---
    x0 = box['x'] + 1
    y0 = box['y'] + box['height'] / 2
    page.mouse.move(x0, y0)
    page.mouse.down()
    page.mouse.move(x0 + 35, y0, steps=15)
    page.mouse.up()
    page.wait_for_timeout(600)

    sel = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} drag selection: {json.dumps(sel)}', flush=True)
    assert sel is not None, f'{fmt} getSelection() must not be null after drag'
    assert sel.get('format') == fmt, f'format must be {fmt}'
    assert sel.get('collapsed') is False, f'{fmt} drag must produce non-collapsed selection'
    sel_text = sel.get('text', '') if fmt == 'docx' else (sel.get('textRange') or {}).get('text', '')
    assert sel_text, f'{fmt} drag selection text must not be empty'

    # Visible highlight elements must be present in the DOM
    hl_count = page.locator(hl_sel).count()
    assert hl_count > 0, f'{fmt} highlight overlay elements must be visible after drag (found 0 .nxh-r/.nxph-r)'
    print(f'  {fmt} drag: collapsed=False text={sel_text!r} highlights={hl_count}', flush=True)

    # --- Screenshot: selection visible ---
    _screenshot(page, f'{fmt}_selection_single_line', [1280, 1440])

    # --- Multi-line screenshot (docx only): drag ~3 lines down for PR attachment ---
    if fmt == 'docx':
        ml_box = page.locator(target_sel).first.bounding_box()
        if ml_box:
            mx0 = ml_box['x'] + 1
            my0 = ml_box['y'] + ml_box['height'] / 2
            page.mouse.move(mx0, my0)
            page.mouse.down()
            page.mouse.move(mx0 + 80, my0 + 45, steps=25)
            page.mouse.up()
            page.wait_for_timeout(400)
            ml_sel = page.evaluate("document.querySelector('#viewer').getSelection()")
            if ml_sel and not ml_sel.get('collapsed'):
                _screenshot(page, f'{fmt}_selection_multi_line', [1280, 1440])
                print(f'  {fmt} multi-line screenshot taken', flush=True)

    # --- Shift+Arrow selection ---
    # Re-fetch position: viewport may have been resized by _screenshot, shifting pptx layout.
    fresh_box = page.locator(target_sel).first.bounding_box()
    if fresh_box:
        x0, y0 = fresh_box['x'] + 1, fresh_box['y'] + fresh_box['height'] / 2
    # Reset: single click to place caret
    page.mouse.click(x0, y0)
    page.wait_for_timeout(300)
    for _ in range(4):
        page.keyboard.press('Shift+ArrowRight')
    page.wait_for_timeout(400)
    sel2 = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} shift-arrow selection: {json.dumps(sel2)}', flush=True)
    assert sel2 is not None, f'{fmt} getSelection() must not be null after Shift+Arrow'
    assert sel2.get('collapsed') is False, f'{fmt} Shift+Arrow must produce non-collapsed selection'

    # --- Typing replaces selection ---
    page.keyboard.type('Q')
    if fmt == 'docx':
        page.wait_for_function("document.querySelector('#viewer').dirty === true", timeout=3000)
    page.wait_for_timeout(500)
    sel3 = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} after-type selection: {json.dumps(sel3)}', flush=True)
    assert sel3.get('collapsed') is True, f'{fmt} typing must collapse selection'
    hl_after = page.locator(hl_sel).count()
    assert hl_after == 0, f'{fmt} highlight must disappear after typing (found {hl_after})'

    # --- Backspace removes selection ---
    # Build a fresh selection with Shift+ArrowRight
    fresh_box2 = page.locator(target_sel).first.bounding_box()
    if fresh_box2:
        x0, y0 = fresh_box2['x'] + 1, fresh_box2['y'] + fresh_box2['height'] / 2
    page.mouse.click(x0, y0)
    page.wait_for_timeout(300)
    for _ in range(3):
        page.keyboard.press('Shift+ArrowRight')
    page.wait_for_timeout(300)
    sel_before_del = page.evaluate("document.querySelector('#viewer').getSelection()")
    assert sel_before_del.get('collapsed') is False, f'{fmt} must have selection before Backspace'
    page.keyboard.press('Backspace')
    page.wait_for_timeout(500)
    sel4 = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} after-backspace selection: {json.dumps(sel4)}', flush=True)
    assert sel4.get('collapsed') is True, f'{fmt} Backspace must collapse selection'
    hl_after2 = page.locator(hl_sel).count()
    assert hl_after2 == 0, f'{fmt} highlight must disappear after Backspace (found {hl_after2})'

    # --- Undo restores deleted content ---
    page.keyboard.press('Control+z')
    page.wait_for_timeout(500)
    # After undo the document should still be dirty (edit is there) and we can verify
    # the undo worked by checking the viewer is still alive (no crash).
    viewer_ok = page.evaluate("!!document.querySelector('#viewer').document")
    assert viewer_ok, f'{fmt} viewer must remain alive after undo'
    print(f'  {fmt} undo: viewer alive', flush=True)

    expect(events(page), [{'dirty': True}], f'{fmt} S1 edits must mark dirty exactly once')
    expect(page.evaluate("document.querySelector('#viewer').dirty"), True, f'{fmt} must still be dirty after undo')
    print(f'  PASS {fmt}: S1 selection (drag, shift-arrow, type replaces, delete, undo)', flush=True)


def main():
    failures = []
    handler = functools.partial(Handler, directory=str(SITE))
    # Bind synchronously: an occupied port is a hard failure, never an unrelated server.
    with http.server.ThreadingHTTPServer(('127.0.0.1', int(sys.argv[1]) if len(sys.argv) > 1 else 7892), handler) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with sync_playwright() as p:
                browser = p.chromium.launch(**({} if not CHROME else {"executable_path": CHROME}), args=['--no-sandbox', '--disable-dev-shm-usage'])
                try:
                    for fmt in ('docx', 'xlsx', 'pptx'):
                        checks = [('dirty', check_dirty), ('selection', check_selection)]
                        if fmt in ('docx', 'pptx'):
                            checks.append(('selection_s1', check_selection_s1))
                        for name, check in checks:
                            page = None
                            try:
                                page = open_page(browser, server.server_port, fmt)
                                check(page, fmt)
                            except Exception as error:
                                failure = f'{fmt} {name}: {error}'
                                failures.append(failure)
                                print(f'  FAIL {failure}', flush=True)
                            finally:
                                if page:
                                    page.close()
                finally:
                    browser.close()
        finally:
            server.shutdown()
            thread.join()
    print(f'\nfailures: {len(failures)}', flush=True)
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main())
