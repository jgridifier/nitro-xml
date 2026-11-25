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
    """Click Get selection so the payload panel is populated, then save screenshots."""
    if out_dir is None:
        out_dir = Path(__file__).resolve().parents[0] / 'out'
    os.makedirs(out_dir, exist_ok=True)
    # HoP review #4: click "Get selection" before each screenshot so #sel-out shows the payload.
    page.locator('#btn-sel').click()
    page.wait_for_function("document.querySelector('#sel-out').textContent.trim() !== '(none)'", timeout=3000)
    page.wait_for_timeout(150)
    for w in widths:
        h = 800 if w == 1280 else 900
        page.set_viewport_size({'width': w, 'height': h})
        page.wait_for_timeout(200)
        page.screenshot(path=str(out_dir / f'{name}_{w}x{h}.png'))
    # Restore default; wait for the layout to stabilise before the caller queries positions.
    page.set_viewport_size({'width': 1400, 'height': 900})
    page.wait_for_timeout(500)


def _span_box(page, sel, idx=0):
    """Read bounding rect of the idx-th matching span inside #viewer shadowRoot in one turn."""
    inner = sel.replace('#viewer ', '')
    box = page.evaluate("""([inner, idx]) => {
        const sr = document.querySelector('#viewer').shadowRoot;
        const nodes = sr ? [...sr.querySelectorAll(inner)] : [];
        const el = idx < 0 ? nodes[nodes.length + idx] : nodes[idx];
        if (!el || !el.isConnected) return null;
        const r = el.getBoundingClientRect();
        return r.width > 0 && r.height > 0 ? {x: r.left, y: r.top, width: r.width, height: r.height} : null;
    }""", [inner, idx])
    assert box, f'span missing for {sel}[{idx}]'
    return box


def _docx_para_text(page, bid):
    return page.evaluate("""(bid) => {
        const sr = document.querySelector('#viewer').shadowRoot;
        let out = '';
        for (const b of sr.querySelectorAll(`[data-bid="${bid}"]`)) {
            for (const s of b.querySelectorAll('.dve-print-line > span')) out += s.textContent || '';
        }
        return out;
    }""", bid)


def _pptx_shape_text(page):
    return page.evaluate("""() => {
        const sr = document.querySelector('#viewer').shadowRoot;
        const sh = sr.querySelector('[data-pptx-kind="text"]');
        if (!sh) return '';
        return [...sh.querySelectorAll('.pptxv-para')].map(p =>
            [...p.querySelectorAll('span')].map(s => s.textContent || '').join('')
        ).join('\\n');
    }""")


def _caret_visible(page, fmt):
    sel = '.nxc' if fmt == 'docx' else '.nxpc'
    return page.evaluate("""(sel) => {
        const bar = document.querySelector('#viewer').shadowRoot.querySelector(sel);
        return !!(bar && bar.style.display === 'block');
    }""", sel)


def _hl_rects(page, hl_sel):
    return page.evaluate("""(sel) => {
        const sr = document.querySelector('#viewer').shadowRoot;
        const inner = sel.replace(/^#viewer\\s+/, '');
        return [...sr.querySelectorAll(inner)].map(el => {
            const r = el.getBoundingClientRect();
            return {left: r.left, top: r.top, right: r.right, bottom: r.bottom, width: r.width, height: r.height};
        });
    }""", hl_sel)


def _assert_no_overlap_highlights(rects, context_label):
    assert len(rects) > 0, f'{context_label}: expected >= 1 highlight rect'
    for i in range(len(rects)):
        for j in range(i + 1, len(rects)):
            a, b = rects[i], rects[j]
            ox = min(a['right'], b['right']) - max(a['left'], b['left'])
            oy = min(a['bottom'], b['bottom']) - max(a['top'], b['top'])
            assert not (ox > 1.0 and oy > 1.0), (
                f'{context_label}: overlapping highlight rects {a} and {b} (two-shade bug)'
            )


def check_selection_s1(page, fmt):
    """S1 real text-range selection (#561) + HoP 11-23 strict asserts:
    drag, multi-line render/payload, backwards selection, Shift+click, dblclick,
    triple-click, cross-run formatting, Shift+Arrow + type exact text + Backspace
    exact text, paste replaces selection, and single Undo restores exact text + payload."""
    if fmt not in ('docx', 'pptx'):
        print(f'  SKIP {fmt}: selection S1 is docx/pptx only', flush=True)
        return

    page.context.grant_permissions(['clipboard-read', 'clipboard-write'])

    if fmt == 'docx':
        target_sel = '#viewer [data-bid="p0"] span'
        hl_sel = '#viewer .nxh-r'
    else:
        target_sel = '#viewer [data-element-id="ppt/slides/slide1.xml-shape-0"] .pptxv-para span'
        hl_sel = '#viewer .nxph-r'

    box = _span_box(page, target_sel, 0)

    # --- 1. Drag selection: start to ~35px right ---
    x0 = box['x'] + 1
    y0 = box['y'] + box['height'] / 2
    page.mouse.move(x0, y0)
    page.mouse.down()
    page.mouse.move(x0 + 35, y0, steps=15)
    page.mouse.up()
    page.wait_for_timeout(500)

    sel = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} drag selection: {json.dumps(sel)}', flush=True)
    assert sel is not None, f'{fmt} getSelection() must not be null after drag'
    assert sel.get('format') == fmt, f'format must be {fmt}'
    assert sel.get('collapsed') is False, f'{fmt} drag must produce non-collapsed selection'
    sel_text = sel.get('text', '') if fmt == 'docx' else (sel.get('textRange') or {}).get('text', '')
    assert sel_text, f'{fmt} drag selection text must not be empty'
    assert not _caret_visible(page, fmt), f'{fmt} caret bar must be hidden during non-collapsed selection'
    _assert_no_overlap_highlights(_hl_rects(page, hl_sel), f'{fmt} single-line drag')

    # --- Screenshot: single-line selection with Get selection populated ---
    _screenshot(page, f'{fmt}_selection_single_line', [1280, 1440])

    # --- 2. Multi-line selection (docx only): spans both paragraphs, one highlight per line, no caret bar ---
    if fmt == 'docx':
        ml_box = _span_box(page, target_sel, 0)
        p1_span = _span_box(page, '#viewer [data-bid="p1"] span', 2)
        mx0 = ml_box['x'] + 1
        my0 = ml_box['y'] + ml_box['height'] / 2
        mx1 = p1_span['x'] + p1_span['width']
        my1 = p1_span['y'] + p1_span['height'] / 2
        page.mouse.move(mx0, my0)
        page.mouse.down()
        page.mouse.move(mx1, my1, steps=25)
        page.mouse.up()
        page.wait_for_timeout(500)
        ml_sel = page.evaluate("document.querySelector('#viewer').getSelection()")
        print(f'  docx multi-line selection: {json.dumps(ml_sel)}', flush=True)
        assert ml_sel and ml_sel.get('collapsed') is False, 'docx multi-line must be non-collapsed'
        assert ml_sel['start']['paragraph'] == 0 and ml_sel['end']['paragraph'] == 1, (
            f"docx multi-line must span paragraphs 0..1, got {ml_sel}"
        )
        assert 'Quarterly field notes' in ml_sel['text'] and '\nPrepared' in ml_sel['text'], (
            f"docx multi-line text mismatch: {ml_sel['text']!r}"
        )
        assert not _caret_visible(page, 'docx'), 'docx caret bar must be hidden in multi-line selection'
        ml_rects = _hl_rects(page, hl_sel)
        expect(len(ml_rects), 2, 'docx multi-line selection across 2 lines must draw exactly 2 even highlight boxes')
        _assert_no_overlap_highlights(ml_rects, 'docx multi-line')
        _screenshot(page, 'docx_selection_multi_line', [1280, 1440])
        print(f'  docx multi-line screenshot taken (rects={len(ml_rects)})', flush=True)

    # --- 3. Backwards selection (payload start/end in document order) + screenshots ---
    if fmt == 'docx':
        p0_last = _span_box(page, target_sel, -1)
        p0_mid = _span_box(page, target_sel, 2)
        bx0 = p0_last['x'] + p0_last['width'] - 2
        by0 = p0_last['y'] + p0_last['height'] / 2
        bx1 = p0_mid['x'] + 2
        by1 = p0_mid['y'] + p0_mid['height'] / 2
    else:
        sp_last = _span_box(page, target_sel, -1)
        sp_first = _span_box(page, target_sel, 0)
        bx0 = sp_last['x'] + sp_last['width'] - 2
        by0 = sp_last['y'] + sp_last['height'] / 2
        bx1 = sp_first['x'] + 40
        by1 = sp_first['y'] + sp_first['height'] / 2
    page.mouse.move(bx0, by0)
    page.mouse.down()
    page.mouse.move(bx1, by1, steps=15)
    page.mouse.up()
    page.wait_for_timeout(500)
    bsel = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} backwards selection: {json.dumps(bsel)}', flush=True)
    assert bsel and bsel.get('collapsed') is False, f'{fmt} backwards selection must be non-collapsed'
    if fmt == 'docx':
        s, e = bsel['start'], bsel['end']
        assert (s['paragraph'], s['offset']) < (e['paragraph'], e['offset']), (
            f'docx backwards start/end must be in document order: {bsel}'
        )
        assert s['offset'] > 0, f'docx backwards mid-line start.offset must be > 0: {bsel}'
        assert len(bsel['text']) > 5, f'docx backwards text too short: {bsel["text"]!r}'
    else:
        tr = bsel['textRange']
        assert tr and tr['start']['offset'] < tr['end']['offset'], (
            f'pptx backwards start/end must be in document order: {bsel}'
        )
        assert tr['start']['offset'] > 0, f'pptx backwards mid-line start.offset must be > 0: {bsel}'
        assert len(tr['text']) >= 3, f'pptx backwards text too short: {tr!r}'
    assert not _caret_visible(page, fmt), f'{fmt} caret bar must be hidden during backwards selection'
    _screenshot(page, f'{fmt}_selection_backwards', [1280, 1440])

    # --- 4. Shift+click extend ---
    p_first = _span_box(page, target_sel, 0)
    p_last = _span_box(page, target_sel, -1)
    page.mouse.click(p_first['x'] + 1, p_first['y'] + p_first['height'] / 2)
    page.wait_for_timeout(300)
    page.keyboard.down('Shift')
    page.mouse.click(p_last['x'] + p_last['width'] / 2, p_last['y'] + p_last['height'] / 2)
    page.keyboard.up('Shift')
    page.wait_for_timeout(400)
    sc_sel = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} shift+click selection: {json.dumps(sc_sel)}', flush=True)
    assert sc_sel and sc_sel.get('collapsed') is False, f'{fmt} Shift+click must extend selection'
    if fmt == 'docx':
        assert sc_sel['start']['offset'] == 0 and sc_sel['end']['offset'] > 10, f'docx Shift+click range: {sc_sel}'
        assert 'Quarterly field' in sc_sel['text'], f'docx Shift+click text: {sc_sel["text"]!r}'
    else:
        assert sc_sel['textRange']['start']['offset'] == 0 and sc_sel['textRange']['end']['offset'] > 5, (
            f'pptx Shift+click range: {sc_sel}'
        )

    # --- 5. Double-click selects word ---
    dbl_box = _span_box(page, target_sel, 0)
    page.mouse.dblclick(dbl_box['x'] + dbl_box['width'] / 2, dbl_box['y'] + dbl_box['height'] / 2)
    page.wait_for_timeout(400)
    dc_sel = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} double-click selection: {json.dumps(dc_sel)}', flush=True)
    assert dc_sel and dc_sel.get('collapsed') is False, f'{fmt} double-click must select word'
    if fmt == 'docx':
        expect(dc_sel['text'], 'Quarterly', 'docx double-click word')
        expect(dc_sel['start']['offset'], 0, 'docx dblclick start')
        expect(dc_sel['end']['offset'], 9, 'docx dblclick end')
    else:
        expect(dc_sel['textRange']['text'], 'Nitro', 'pptx double-click word')
        expect(dc_sel['textRange']['start']['offset'], 0, 'pptx dblclick start')
        expect(dc_sel['textRange']['end']['offset'], 5, 'pptx dblclick end')

    # --- 6. Triple-click selects paragraph ---
    tri_box = _span_box(page, target_sel, 0)
    page.mouse.click(tri_box['x'] + tri_box['width'] / 2, tri_box['y'] + tri_box['height'] / 2, click_count=3)
    page.wait_for_timeout(400)
    tc_sel = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} triple-click selection: {json.dumps(tc_sel)}', flush=True)
    assert tc_sel and tc_sel.get('collapsed') is False, f'{fmt} triple-click must select paragraph'
    if fmt == 'docx':
        expect(tc_sel['text'], 'Quarterly field notes', 'docx triple-click paragraph')
        expect(tc_sel['start']['offset'], 0, 'docx triple-click start')
        expect(tc_sel['end']['offset'], len('Quarterly field notes'), 'docx triple-click end')
    else:
        expect(tc_sel['textRange']['text'], 'Nitro XML', 'pptx triple-click paragraph')
        expect(tc_sel['textRange']['start']['offset'], 0, 'pptx triple-click start')
        expect(tc_sel['textRange']['end']['offset'], len('Nitro XML'), 'pptx triple-click end')

    # --- 7. Selection crossing runs with different formatting (docx p10: bold + plain + italic + underlined) ---
    if fmt == 'docx':
        page.evaluate("""() => {
            const sr = document.querySelector('#viewer').shadowRoot;
            const p10 = sr.querySelector('[data-bid="p10"]');
            p10 && p10.scrollIntoView({block: 'center'});
        }""")
        page.wait_for_timeout(150)
        p10_first = _span_box(page, '#viewer [data-bid="p10"] span', 0)
        p10_under = page.evaluate("""() => {
            const sr = document.querySelector('#viewer').shadowRoot;
            const el = [...sr.querySelectorAll('[data-bid="p10"] span')].find(s => s.textContent.includes('underlined'));
            if (!el) return null;
            const r = el.getBoundingClientRect();
            return {x: r.left, y: r.top, width: r.width, height: r.height};
        }""")
        assert p10_under, 'docx p10 underlined span missing'
        page.mouse.move(p10_first['x'] + 1, p10_first['y'] + p10_first['height'] / 2)
        page.mouse.down()
        page.mouse.move(p10_under['x'] + p10_under['width'] - 2, p10_under['y'] + p10_under['height'] / 2, steps=15)
        page.mouse.up()
        page.wait_for_timeout(500)
        xr_sel = page.evaluate("document.querySelector('#viewer').getSelection()")
        print(f'  docx cross-run selection: {json.dumps(xr_sel)}', flush=True)
        assert xr_sel and xr_sel.get('collapsed') is False, 'docx cross-run selection must be non-collapsed'
        assert 'Bold, italic and underlined' in xr_sel['text'], f'docx cross-run text mismatch: {xr_sel["text"]!r}'
        _assert_no_overlap_highlights(_hl_rects(page, hl_sel), 'docx cross-run')
        page.evaluate("""() => {
            const dom = document.querySelector('#viewer').shadowRoot.querySelector('.dom');
            if (dom) dom.scrollTop = 0;
        }""")
        page.wait_for_timeout(150)

    expect(events(page), [], f'{fmt} selection gestures must not mark dirty')
    expect(page.evaluate("document.querySelector('#viewer').dirty"), False, f'{fmt} must be clean before typing')

    # --- 8. Shift+Arrow selection -> Type 'Q' (exact text) -> Undo (exact pre-replace text + selection restored) ---
    fresh_box = _span_box(page, target_sel, 0)
    x0, y0 = fresh_box['x'] + 1, fresh_box['y'] + fresh_box['height'] / 2
    page.mouse.click(x0, y0)
    page.wait_for_timeout(300)
    for _ in range(4):
        page.keyboard.press('Shift+ArrowRight')
    page.wait_for_timeout(400)
    sel2 = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} shift-arrow selection: {json.dumps(sel2)}', flush=True)
    assert sel2 is not None and sel2.get('collapsed') is False, f'{fmt} Shift+Arrow must select'
    if fmt == 'docx':
        expect(sel2['text'], 'Quar', 'docx Shift+Arrow x4 text')
    else:
        expect(sel2['textRange']['text'], 'Nitr', 'pptx Shift+Arrow x4 text')

    # Type 'Q' -> exact replaced text
    page.keyboard.type('Q')
    page.wait_for_function("document.querySelector('#viewer').dirty === true", timeout=3000)
    page.wait_for_timeout(400)
    sel3 = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} after-type selection: {json.dumps(sel3)}', flush=True)
    assert sel3.get('collapsed') is True, f'{fmt} typing must collapse selection'
    assert page.locator(hl_sel).count() == 0, f'{fmt} highlight must disappear after typing'
    if fmt == 'docx':
        expect(_docx_para_text(page, 'p0'), 'Qterly field notes', 'docx exact text after replace with Q')
    else:
        expect(_pptx_shape_text(page), 'Qo XML', 'pptx exact text after replace with Q')
        expect(sel3['text'], 'Qo XML', 'pptx getSelection().text after replace with Q')

    # Single Undo -> exact pre-replace text AND selection payload equals sel2
    page.keyboard.press('Control+z')
    page.wait_for_timeout(500)
    if fmt == 'docx':
        expect(_docx_para_text(page, 'p0'), 'Quarterly field notes', 'docx exact pre-replace text after Undo')
    else:
        expect(_pptx_shape_text(page), 'Nitro XML', 'pptx exact pre-replace text after Undo')
    sel_undo = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} after-undo selection: {json.dumps(sel_undo)}', flush=True)
    expect(sel_undo, sel2, f'{fmt} getSelection() after Undo must equal original pre-replace selection')
    assert page.locator(hl_sel).count() > 0, f'{fmt} highlight must be restored after Undo'

    # --- 9. Re-type 'Q' then Backspace -> exact text ('terly field notes' / 'o XML') ---
    page.keyboard.type('Q')
    page.wait_for_timeout(400)
    if fmt == 'docx':
        expect(_docx_para_text(page, 'p0'), 'Qterly field notes', 'docx exact text after re-type Q')
    else:
        expect(_pptx_shape_text(page), 'Qo XML', 'pptx exact text after re-type Q')
    page.keyboard.press('Backspace')
    page.wait_for_timeout(400)
    sel4 = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} after-backspace selection: {json.dumps(sel4)}', flush=True)
    assert sel4.get('collapsed') is True, f'{fmt} Backspace must collapse selection'
    assert page.locator(hl_sel).count() == 0, f'{fmt} highlight must disappear after Backspace'
    if fmt == 'docx':
        expect(_docx_para_text(page, 'p0'), 'terly field notes', 'docx exact text after Backspace')
    else:
        expect(_pptx_shape_text(page), 'o XML', 'pptx exact text after Backspace')

    # --- 10. Paste replaces selection ---
    # Reset back to original text with two undos (undo Backspace, undo Q) or double-click current first word
    page.keyboard.press('Control+z')
    page.wait_for_timeout(300)
    page.keyboard.press('Control+z')
    page.wait_for_timeout(400)
    paste_box = _span_box(page, target_sel, 0)
    page.mouse.dblclick(paste_box['x'] + paste_box['width'] / 2, paste_box['y'] + paste_box['height'] / 2)
    page.wait_for_timeout(400)
    page.evaluate("navigator.clipboard.writeText('Demo')")
    page.keyboard.press('Control+v')
    page.wait_for_timeout(600)
    if fmt == 'docx':
        expect(_docx_para_text(page, 'p0'), 'Demo field notes', 'docx exact text after paste replaces selection')
    else:
        expect(_pptx_shape_text(page), 'Demo XML', 'pptx exact text after paste replaces selection')

    # Verify dirty tracking stayed consistent across replace -> undo-to-clean -> re-edit -> paste.
    ev_list = events(page)
    assert ev_list and ev_list[0] == {'dirty': True} and ev_list[-1] == {'dirty': True}, (
        f'{fmt} S1 dirty transitions unexpected: {ev_list}'
    )
    expect(page.evaluate("document.querySelector('#viewer').dirty"), True, f'{fmt} must still be dirty')
    print(f'  PASS {fmt}: S1 selection + HoP strict asserts', flush=True)


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
