# SPDX-License-Identifier: MIT
import argparse, glob, json, os, re, subprocess
p = argparse.ArgumentParser()
for a in ['arm','issue','base','base-run','wt','log','wall','rounds','first-try','pass','status']: p.add_argument('--'+a)
a = p.parse_args()
def git(*x): return subprocess.run(['git','-C',a.wt,*x],capture_output=True,text=True).stdout
num = git('diff','--numstat',a.base_run,'HEAD')
loc = 0
for l in num.splitlines():
    ad, de, f = l.split('\t')
    if ad == '-' or f.startswith(('fixtures/','tools/model-compare/')) or f.endswith('.lock'): continue
    loc += int(ad)
diff = git('diff','-U3',a.base_run,'HEAD','--','*.rs')
tests = len(re.findall(r'^\+\s*#\[(?:test|proptest|tokio::test)', diff, re.M)) + len(re.findall(r'^\+\s*proptest!\s*\{', diff, re.M))
# Rejected: an added #[test] whose function body does not start with a `// Catches:` line (AGENTS.md §0).
rejected = 0
for m in re.finditer(r'^\+\s*#\[test\]\s*\n(?:\+.*\n)*?\+\s*fn [^\n]*\{\s*\n\+\s*(.*)', diff, re.M):
    if not m.group(1).strip().startswith('// Catches:'): rejected += 1
usage = {'cost_usd': 0.0, 'input_tokens': 0, 'output_tokens': 0, 'cache_read_tokens': 0, 'by_model': {}}
for f in sorted(glob.glob(f'{a.log}/agent-*.json')):
    try: d = json.load(open(f))
    except Exception: continue
    usage['cost_usd'] += d.get('total_cost_usd') or 0
    for m, u in (d.get('modelUsage') or {}).items():
        bm = usage['by_model'].setdefault(m, {'in':0,'out':0,'cache_read':0,'cost':0.0})
        bm['in'] += u.get('inputTokens',0) + u.get('cacheCreationInputTokens',0); bm['out'] += u.get('outputTokens',0)
        bm['cache_read'] += u.get('cacheReadInputTokens',0); bm['cost'] += u.get('costUSD',0)
        usage['input_tokens'] += u.get('inputTokens',0) + u.get('cacheCreationInputTokens',0); usage['output_tokens'] += u.get('outputTokens',0); usage['cache_read_tokens'] += u.get('cacheReadInputTokens',0)
nim = {'requests': 0, 'http_503': 0, 'errors': 0}
for f in sorted(glob.glob(f'{a.log}/agent-*.jsonl')):
    for line in open(f, errors='replace'):
        if '"tokens"' in line:
            try:
                ev = json.loads(line); t = (ev.get('part') or {}).get('tokens') or {}
                usage['input_tokens'] += t.get('input',0); usage['output_tokens'] += t.get('output',0); usage['cache_read_tokens'] += (t.get('cache') or {}).get('read',0)
                nim['requests'] += 1
            except Exception: pass
        if 'usage' in line and '"input_tokens"' in line:
            try:
                u = json.loads(line).get('usage') or {}
                usage['input_tokens'] += u.get('input_tokens',0); usage['output_tokens'] += u.get('output_tokens',0); usage['cache_read_tokens'] += u.get('cached_input_tokens',0)
            except Exception: pass
for f in sorted(glob.glob(f'{a.log}/agent-*.err')):
    t = open(f, errors='replace').read()
    nim['http_503'] += len(re.findall(r'\b503\b', t)); nim['errors'] += len(re.findall(r'level=ERROR', t))
lat = [float(x) for f in glob.glob(f'{a.log}/agent-*.err') for x in re.findall(r'duration[=:]\s*(\d+(?:\.\d+)?)', open(f, errors='replace').read())]
out = dict(issue=int(a.issue), arm=a.arm, base=a.base, status=a.status, first_try_pass=a.first_try=='true', final_pass=a.__dict__['pass']=='true',
           review_rounds=int(a.rounds), loc_added=loc, tests_added=tests, rejected_tests=rejected, wall_s=int(a.wall), usage=usage,
           branch=git('rev-parse','--abbrev-ref','HEAD').strip(), head=git('rev-parse','HEAD').strip())
if a.arm.startswith('D'): out['nim'] = dict(nim, latency_samples_ms=len(lat), latency_median_ms=(sorted(lat)[len(lat)//2] if lat else None))
print(json.dumps(out))
