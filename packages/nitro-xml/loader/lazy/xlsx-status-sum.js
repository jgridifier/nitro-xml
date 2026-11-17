// SPDX-License-Identifier: MIT
// Selection stats (Sum / Average / Count) in the Excel status bar — issue #540.
export function mountSelStats(el, span, getChrome) {
  if (!span) return;
  let timer = 0;
  let lastKey = '';
  let watched = null;

  async function update() {
    const ch = getChrome();
    if (!ch) { span.hidden = true; return; }
    const nameEl = ch.querySelector('.xfb-name');
    const ref = nameEl ? nameEl.textContent.trim() : '';
    const tabBtn = ch.querySelector('.t button[aria-current=true]');
    const sheet = tabBtn ? (tabBtn.dataset.g | 0) : 0;
    const n = cellCount(ref);
    const key = `${sheet}\0${ref}`;
    if (key === lastKey) return;
    lastKey = key;

    if (n < 2 || n > 100000) { span.hidden = true; return; }

    const doc = el._d;
    if (!doc) { span.hidden = true; return; }

    let tsv = '';
    try {
      if (doc.handle) {
        tsv = doc.handle.cellText(sheet, ref);
      } else if (doc._request && doc._worker) {
        const msg = await doc._request('cell-text', () =>
          doc._worker.postMessage({ type: 'cell-text', sheet, range: ref })
        );
        tsv = (msg && msg.result) || '';
      } else {
        span.hidden = true;
        return;
      }
    } catch { span.hidden = true; return; }

    const cells = tsv.split(/[\t\n]/).filter(s => s);
    let sum = 0, cnt = 0;
    for (const c of cells) {
      const v = parseVal(c);
      if (v !== null) { sum += v; cnt++; }
    }

    if (cnt < 2) { span.hidden = true; return; }
    span.textContent = `Average: ${fmt(sum / cnt)}  Count: ${cnt}  Sum: ${fmt(sum)}`;
    span.hidden = false;
  }

  function schedule() { clearTimeout(timer); timer = setTimeout(update, 60); }

  const mo = new MutationObserver(schedule);

  function connect() {
    const c = getChrome();
    if (c && c !== watched) {
      if (watched) mo.disconnect();
      watched = c;
      mo.observe(c, { childList: true, subtree: true, characterData: true });
    }
    if (c) schedule();
  }

  setInterval(connect, 400);
  connect();
}

function cellCount(ref) {
  const i = ref.indexOf(':');
  if (i < 0) return ref ? 1 : 0;
  const p = parseRef(ref.slice(0, i));
  const q = parseRef(ref.slice(i + 1));
  if (!p || !q) return 0;
  return (Math.abs(q.r - p.r) + 1) * (Math.abs(q.c - p.c) + 1);
}

function parseRef(s) {
  const m = /^([A-Z]+)(\d+)$/i.exec(s.trim());
  if (!m) return null;
  let c = 0;
  const col = m[1].toUpperCase();
  for (let i = 0; i < col.length; i++) c = c * 26 + col.charCodeAt(i) - 64;
  return { r: +m[2] - 1, c: c - 1 };
}

function parseVal(t) {
  t = t.trim();
  if (!t) return null;
  const n = +t;
  if (!isNaN(n)) return n;
  const f = parseFloat(t.replace(/,/g, '').replace(/^[^\d\-.]+/, ''));
  return isNaN(f) ? null : f;
}

function fmt(n) {
  return isFinite(n) ? String(parseFloat(n.toPrecision(10))) : String(n);
}
