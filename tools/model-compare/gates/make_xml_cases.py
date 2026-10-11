# SPDX-License-Identifier: MIT
# Extracts every XML part of the real corpus and writes crafted edge cases for the #12 gate.
import os, sys, zipfile, glob
out = sys.argv[1]; os.makedirs(out, exist_ok=True); paths = []
for f in sorted(glob.glob('fixtures/real/*/*.*')):
    if f.endswith(('.md', '.json')): continue
    try: z = zipfile.ZipFile(f)
    except Exception: continue
    for n in z.namelist():
        if n.endswith(('.xml', '.rels', '.vml')) and not n.endswith('/'):
            p = os.path.join(out, 'corpus', os.path.basename(f) + '__' + n.replace('/', '__'))
            os.makedirs(os.path.dirname(p), exist_ok=True); open(p, 'wb').write(z.read(n)); paths.append(p)
cases = {
 'doctype.xml': '<?xml version="1.0"?><!DOCTYPE a [<!ENTITY x "y">]><a>&x;</a>',
 'entity-only.xml': '<a><!ENTITY x "y"></a>',
 'unknown-entity.xml': '<a>&nbsp;</a>',
 'numeric-refs.xml': '<a b="&#x41;&#66;&amp;&lt;&gt;&quot;&apos;">&#x1F600;&#169;</a>',
 'mismatched.xml': '<a><b></a></b>',
 'undeclared-prefix.xml': '<a><x:b/></a>',
 'cdata-comment-pi.xml': '<a>one<![CDATA[<two>]]><!-- c --><?pi x?>three</a>',
 'attr-ws.xml': '<a b="x\ty\nz\r\nw"  c = \'q\'/>',
 'crlf-text.xml': '<a>l1\r\nl2\rl3</a>',
 'bom.xml': '\ufeff<?xml version="1.0" encoding="UTF-8"?><a/>',
 'ns-default-and-prefixed.xml': '<r xmlns="urn:d" xmlns:p="urn:p"><p:c p:x="1" y="2"><d/></p:c></r>',
 'xml-space.xml': '<a xml:space="preserve">  </a>',
 'dup-attr.xml': '<a b="1" b="2"/>',
 'no-root.xml': '<?xml version="1.0"?>',
 'empty.xml': '',
 'trailing-garbage.xml': '<a/><b/>',
 'lt-in-attr.xml': '<a b="<"/>',
 'deep.xml': '<a>' * 2000 + '</a>' * 2000,
}
for name, text in cases.items():
    p = os.path.join(out, 'crafted', name); os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, 'wb').write(text.encode('utf-8')); paths.append(p)
for p in sorted(glob.glob('fixtures/xml/**/*.xml', recursive=True)): paths.append(p)
open(os.path.join(out, 'list.txt'), 'w').write('\n'.join(paths) + '\n')
print(len(paths))
