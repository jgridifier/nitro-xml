#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Blind review packet: relabels each arm's diff as X/Y/Z/W in random order and seals the key.
# bash tools/model-compare/blind.sh <issue>
set -euo pipefail
N=$1; MC=$(cd "$(dirname "$0")" && pwd); OUT=$MC/review/issue-$N; mkdir -p "$OUT"
python3 - "$N" "$MC" "$OUT" <<'PY'
import json, random, subprocess, sys
n, mc, out = sys.argv[1:]
runs = [json.loads(l) for l in open(f'{mc}/results/runs.jsonl') if json.loads(l)['issue'] == int(n) and json.loads(l)['status'] == 'ok']
labels = ['X','Y','Z','W'][:len(runs)]; random.SystemRandom().shuffle(runs); key = {}
for lab, r in zip(labels, runs):
    diff = subprocess.run(['git','log','--format=','-p','--reverse',f"{r['base']}..{r['head']}",'--','.',':!tools/model-compare',':!*.lock'],capture_output=True,text=True).stdout
    open(f'{out}/{lab}.diff','w').write(diff); key[lab] = r['arm']
open(f'{out}/key.sealed.json','w').write(json.dumps(key))
open(f'{out}/scores.md','w').write('# Blind review #%s\n\nScore each 1-5 before opening key.sealed.json.\n\n| Label | Correctness | Scope | Licence headers | Tests (non-obvious, Catches line) | Notes |\n|---|---|---|---|---|---|\n' % n + ''.join(f'| {l} | | | | | |\n' for l in labels))
print('packet:', out, labels)
PY
