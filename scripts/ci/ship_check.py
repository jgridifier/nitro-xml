# SPDX-License-Identifier: MIT
"""Fails if a shipped artefact references the upstream TS engine, ProseMirror or Lit (PRODUCT_SPEC.md §5:
"CI fails if a shipped artefact imports it"). Checks, per package: package.json dependency fields, every
file in the `npm pack --dry-run` list (text content), and the import section of every .wasm in it."""
import json, os, re, subprocess, sys

FORBIDDEN = [
    ('ooxml-core', re.compile(r'ooxml-core')),
    ('ooxml-ui', re.compile(r'ooxml-ui')),
    ('upstream/', re.compile(r'(?<![\w-])upstream/')),
    ('ProseMirror', re.compile(r'prosemirror', re.I)),
    ('Lit', re.compile(r'''(?:from\s*|import\s*\(\s*|require\s*\(\s*)['"](?:lit|lit-html|lit-element|@lit/[^'"]*)['"]''')),
]
DEP_FIELDS = ('dependencies', 'peerDependencies', 'optionalDependencies', 'bundleDependencies', 'bundledDependencies')
LIT_DEPS = re.compile(r'^(lit|lit-html|lit-element|@lit/.*)$')

def leb(b, i):
    r = s = 0
    while True:
        x = b[i]; i += 1; r |= (x & 0x7F) << s; s += 7
        if x < 0x80: return r, i

def wasm_imports(path):
    b = open(path, 'rb').read()
    if b[:4] != b'\0asm': return []
    i, out = 8, []
    while i < len(b):
        sid = b[i]; size, i = leb(b, i + 1); end = i + size
        if sid == 2:
            n, j = leb(b, i)
            for _ in range(n):
                ml, j = leb(b, j); mod = b[j:j + ml].decode(errors='replace'); j += ml
                fl, j = leb(b, j); fld = b[j:j + fl].decode(errors='replace'); j += fl
                kind = b[j]; j += 1
                if kind == 0: _, j = leb(b, j)
                elif kind == 1: j += 1; flags, j = leb(b, j); _, j = leb(b, j); j = leb(b, j)[1] if flags & 1 else j
                elif kind == 2: flags, j = leb(b, j); _, j = leb(b, j); j = leb(b, j)[1] if flags & 1 else j
                elif kind == 3: j += 2
                elif kind == 4: j += 1; _, j = leb(b, j)
                out.append(f'{mod}::{fld}')
        i = end
    return out

def check(pkg):
    bad = []
    meta = json.load(open(os.path.join(pkg, 'package.json')))
    for field in DEP_FIELDS:
        deps = meta.get(field) or {}
        for name in (deps if isinstance(deps, dict) else deps):
            spec = deps[name] if isinstance(deps, dict) else ''
            if name in ('ooxml-core', 'ooxml-ui') or 'prosemirror' in name.lower() or LIT_DEPS.match(name) or 'upstream/' in str(spec) or 'ChristopherVR/ooxml' in str(spec):
                bad.append(f'package.json {field}: {name} {spec}'.strip())
    packed = subprocess.run(['npm', 'pack', '--dry-run', '--json', '--ignore-scripts'], cwd=pkg, capture_output=True, text=True)
    if packed.returncode != 0:
        bad.append(f'npm pack --dry-run failed: {packed.stderr.strip()[:200]}'); return bad
    files = [f['path'] for f in json.loads(packed.stdout)[0]['files']]
    for rel in files:
        path = os.path.join(pkg, rel)
        if rel.endswith('.wasm'):
            for imp in wasm_imports(path):
                for label, rx in FORBIDDEN[:4]:
                    if rx.search(imp): bad.append(f'{rel}: wasm import {imp} references {label}')
            continue
        if rel in ('LICENSE', 'LICENSE-THIRD-PARTY', 'NOTICE', 'README.md', 'package.json'): continue  # licence texts credit upstream by name
        text = open(path, encoding='utf-8', errors='replace').read()
        for label, rx in FORBIDDEN:
            m = rx.search(text)
            if m: bad.append(f'{rel}: references {label} ({m.group(0)!r})')
    return bad

if __name__ == '__main__':
    fail = False
    for pkg in sys.argv[1:]:
        problems = check(pkg)
        for p in problems: print(f'ship-check: {pkg}: {p}', file=sys.stderr)
        print(f'ship-check: {pkg}: {"FAIL" if problems else "ok"} ')
        fail |= bool(problems)
    sys.exit(1 if fail else 0)
