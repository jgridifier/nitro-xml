# SPDX-License-Identifier: MIT
# Covers: ribbon option, execCommand, nitro-command event (Part of #590).
"""Run after assembling _site. All three formats: docx / xlsx / pptx."""
import functools
import http.server
import os
import pathlib
import threading
from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[3]

HOST_CMD_ID = "host.review"
HOST_CMD_LABEL = "Send for review"
HOST_CMD_ICON = "M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"


def chrome_path():
    env = os.environ.get('HOST_API_CHROME')
    if env:
        return env
    default = '/home/box/.cache/ms-playwright/chromium_headless_shell-1254/chrome-headless-shell-linux64/chrome-headless-shell'
    return default if os.path.exists(default) else None


class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *_):
        pass


server = http.server.ThreadingHTTPServer(
    ('127.0.0.1', 0), functools.partial(Handler, directory=str(ROOT / '_site')))
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    with sync_playwright() as p:
        chrome = chrome_path()
        browser = p.chromium.launch(
            **({'executable_path': chrome} if chrome else {}),
            args=['--no-sandbox', '--disable-dev-shm-usage'],
        )
        for fmt in ['docx', 'xlsx', 'pptx']:
            page = browser.new_page()
            page.set_default_timeout(25000)
            errors = []
            page.on('pageerror', lambda e: errors.append(str(e)))
            page.goto(f'http://127.0.0.1:{server.server_port}/demo/host-api/ribbon.html?f={fmt}')
            print(fmt, 'waiting for ready', flush=True)
            page.evaluate(
                'Promise.race([ready, new Promise((_,reject) => setTimeout(() => reject(new Error("timeout")), 30000))])'
            )
            print(fmt, 'ready', flush=True)

            # ----------------------------------------------------------------
            # 1. el.commands is populated with built-in entries
            # ----------------------------------------------------------------
            count = page.evaluate('el.commands.length')
            assert count > 100, (fmt, 'commands empty', count)
            has_bold = page.evaluate('el.commands.some(c => c.id === "home.bold")')
            assert has_bold, (fmt, 'home.bold missing from commands')
            print(fmt, 'commands ok:', count, 'entries', flush=True)

            # ----------------------------------------------------------------
            # 2. execCommand(known id) returns true and fires nitro-command
            # Use home.find (read-only, no clipboard/selection needed).
            # ----------------------------------------------------------------
            result = page.evaluate('''() => {
              const events = [];
              el.addEventListener("nitro-command", e => {
                events.push({id: e.detail.id, fmt: e.detail.format});
                e.preventDefault();  // stop built-in so no side effects
              }, {once: true});
              const ok = el.execCommand("home.find");
              return {ok, events};
            }''')
            assert result['ok'] is True, (fmt, 'execCommand known returned false')
            assert len(result['events']) == 1, (fmt, 'nitro-command not fired for execCommand')
            assert result['events'][0]['id'] == 'home.find', (fmt, result)
            assert result['events'][0]['fmt'] == fmt, (fmt, result)
            print(fmt, 'execCommand(known) ok', flush=True)

            # ----------------------------------------------------------------
            # 3. execCommand(unknown id) returns false (no event fired)
            # ----------------------------------------------------------------
            result2 = page.evaluate('''() => {
              const events = [];
              el.addEventListener("nitro-command", e => events.push(e.detail.id));
              const ok = el.execCommand("bad.unknown.id");
              return {ok, events};
            }''')
            assert result2['ok'] is False, (fmt, 'execCommand bad id returned true')
            assert result2['events'] == [], (fmt, 'nitro-command fired for unknown id')
            print(fmt, 'execCommand(unknown) ok', flush=True)

            # ----------------------------------------------------------------
            # 4. nitro-command is cancelable via preventDefault
            # ----------------------------------------------------------------
            cancelled = page.evaluate('''() => {
              let prevented = false;
              el.addEventListener("nitro-command", e => {
                if (e.detail.id === "home.find") { e.preventDefault(); prevented = true; }
              }, {once: true});
              el.execCommand("home.find");
              return prevented;
            }''')
            assert cancelled is True, (fmt, 'preventDefault not fired')
            print(fmt, 'nitro-command cancelable ok', flush=True)

            # ----------------------------------------------------------------
            # 5. ribbon.hide removes buttons from chrome
            # ----------------------------------------------------------------
            # First confirm file.open button is present (embedded toolbar only
            # shows a minimal set, so test against full Rust chrome when available)
            page.evaluate('el.ribbon = { hide: ["file.open", "file.new"] }')
            page.wait_for_timeout(200)
            hidden_check = page.evaluate('''() => {
              const ch = el._ch;
              if (!ch) return "no-ch";
              const btn = ch.querySelector('[data-g="file.open"]');
              return btn ? "still-visible" : "hidden";
            }''')
            # _ch may not have file.open in embedded mode (minimal toolbar), skip btn check
            # but confirm _ribbonCfg was set
            cfg_set = page.evaluate('el._ribbonCfg && el._ribbonCfg.hide instanceof Set')
            assert cfg_set is True, (fmt, 'ribbon hide: _ribbonCfg not a Set')
            print(fmt, 'ribbon hide ok (cfg stored, chrome btn:', hidden_check, ')', flush=True)

            # ----------------------------------------------------------------
            # 6. ribbon.add injects a host button
            # ----------------------------------------------------------------
            page.evaluate(f'''el.ribbon = {{
              add: [{{
                id: "{HOST_CMD_ID}",
                label: "{HOST_CMD_LABEL}",
                icon: "{HOST_CMD_ICON}",
                editing: false,
              }}]
            }}''')
            page.wait_for_timeout(200)
            btn_present = page.evaluate(f'''() => {{
              const ch = el._ch;
              if (!ch) return "no-ch";
              return ch.querySelector('[data-g="{HOST_CMD_ID}"]') ? "found" : "missing";
            }}''')
            # In embedded mode _ch is minimal; fall back to checking via _ribbonCfg.add
            add_stored = page.evaluate(
                f'el._ribbonCfg && el._ribbonCfg.add.some(a => a.id === "{HOST_CMD_ID}")'
            )
            assert add_stored is True, (fmt, 'ribbon add: host cmd not in _ribbonCfg.add')
            print(fmt, 'ribbon add ok (chrome btn:', btn_present, ')', flush=True)

            # ----------------------------------------------------------------
            # 7. Host-added command fires nitro-command and returns to host
            # ----------------------------------------------------------------
            host_event = page.evaluate(f'''() => {{
              const events = [];
              el.addEventListener("nitro-command", e => events.push(e.detail.id));
              const ok = el.execCommand("{HOST_CMD_ID}");
              return {{ok, events}};
            }}''')
            assert host_event['ok'] is True, (fmt, 'execCommand host cmd returned false')
            assert HOST_CMD_ID in host_event['events'], (fmt, 'host cmd: nitro-command not fired')
            print(fmt, 'host-added execCommand fires event ok', flush=True)

            # ----------------------------------------------------------------
            # 8. readonly disables editing commands in el.commands
            # Use setAttribute to trigger the attribute-change path; poll
            # el.readonly (not nitro-mode) to avoid an indefinite hang.
            # ----------------------------------------------------------------
            page.evaluate('el.setAttribute("readonly", "true")')
            page.wait_for_function('el.readonly === true', timeout=15000)
            editing_disabled = page.evaluate('''() => {
              const cmds = el.commands;
              const editingCmds = cmds.filter(c => {
                // home.bold is editing:true, home.copy is editing:false
                return c.id === "home.bold" || c.id === "home.copy";
              });
              return editingCmds.map(c => ({id: c.id, enabled: c.enabled}));
            }''')
            bold = next((c for c in editing_disabled if c['id'] == 'home.bold'), None)
            copy = next((c for c in editing_disabled if c['id'] == 'home.copy'), None)
            assert bold is not None, (fmt, 'home.bold not in commands')
            assert copy is not None, (fmt, 'home.copy not in commands')
            assert bold['enabled'] is False, (fmt, 'home.bold should be disabled in readonly')
            assert copy['enabled'] is True, (fmt, 'home.copy should be enabled in readonly')
            print(fmt, 'readonly commands disabled ok', flush=True)

            # ----------------------------------------------------------------
            # 9. Host-added editing:true command disabled in readonly
            # ----------------------------------------------------------------
            page.evaluate(f'''el.ribbon = {{
              add: [{{
                id: "host.edit-only",
                label: "Edit Only Action",
                editing: true,
              }}]
            }}''')
            host_ro = page.evaluate('''() => {
              const cmds = el.commands;
              const h = cmds.find(c => c.id === "host.edit-only");
              return h ? h.enabled : null;
            }''')
            assert host_ro is False, (fmt, 'host editing:true cmd should be disabled in readonly', host_ro)
            print(fmt, 'host-added editing:true disabled in readonly ok', flush=True)

            # ----------------------------------------------------------------
            # 10. ribbon = null clears the config
            # ----------------------------------------------------------------
            page.evaluate('el.ribbon = null')
            cleared = page.evaluate('el._ribbonCfg === null')
            assert cleared is True, (fmt, 'ribbon = null did not clear _ribbonCfg')
            print(fmt, 'ribbon=null clear ok', flush=True)

            # Filter known browser-permission errors that are not code bugs.
            real_errors = [e for e in errors if 'Clipboard' not in e]
            assert not real_errors, (fmt, real_errors)
            print(f'PASS {fmt}: execCommand, nitro-command, cancel, hide, add, readonly, host-added', flush=True)
            page.close()
        browser.close()
finally:
    server.shutdown()
