// SPDX-License-Identifier: MIT
/**
 * Deep phase tracing for React-embed mount→paint.
 * Patches Worker postMessage/onmessage and records Performance marks.
 */

function now() {
  return performance.now();
}

function mark(name, extra) {
  globalThis.__nitroBench?.mark?.(name, extra);
  try {
    performance.mark(`nx:${name}`);
  } catch {
    /* ignore */
  }
}

export function createTraceSession() {
  const phases = {
    worker_ready_ms: null,
    init_core_host_ms: null,
    fixture_fetch_ms: null,
    wd_construct_ms: null,
    open_post_ms: null,
    open_roundtrip_ms: null,
    bytes_transfer_kib: null,
    after_open_ao_ms: null,
    first_tick_ms: null,
    first_cmdbuf_bytes: null,
    first_measure_count: null,
    first_layout_count: null,
    measure_rounds: 0,
    measure_total_ms: 0,
    layout_rounds: 0,
    layout_total_ms: 0,
    frame_rounds: 0,
    frame_total_ms: 0,
    paint_wait_ms: null,
    style_recalc_probe_ms: null,
    host_load_buffer_ms: null,
    mount_to_paint_ms: null,
  };
  const ticks = [];
  const worker_msgs = [];
  const pending = new Map();
  let readyT0 = null;

  return {
    phases,
    ticks,
    worker_msgs,
    reset() {
      for (const k of Object.keys(phases)) {
        phases[k] =
          typeof phases[k] === "number" && !k.endsWith("_ms") && k.includes("rounds")
            ? 0
            : k.endsWith("_rounds") || k.endsWith("_total_ms")
              ? 0
              : null;
      }
      phases.measure_rounds = 0;
      phases.measure_total_ms = 0;
      phases.layout_rounds = 0;
      phases.layout_total_ms = 0;
      phases.frame_rounds = 0;
      phases.frame_total_ms = 0;
      ticks.length = 0;
      worker_msgs.length = 0;
      pending.clear();
      readyT0 = null;
    },
    set(key, ms) {
      phases[key] = typeof ms === "number" ? +Number(ms).toFixed(2) : ms;
    },
    onOutgoing(msg) {
      if (!msg || !msg.type) return;
      const t0 = now();
      if (msg.type === "prewarm") readyT0 = t0;
      if (msg.type === "open") {
        const n = msg.bytes?.byteLength || 0;
        phases.bytes_transfer_kib = +(n / 1024).toFixed(2);
        pending.set("opened", { t0, kind: "open", bytes: n });
        mark("open_post", { bytes: n });
      }
      if (msg.type === "take-frame") pending.set("frame", { t0, kind: "frame" });
      if (msg.type === "measure-results") pending.set("measure-acked", { t0, kind: "measure" });
      if (msg.type === "layout-results") pending.set("layout-acked", { t0, kind: "layout" });
      if (msg.type === "sa") pending.set("sa", { t0, kind: "shell", op: msg.op });
    },
    onIncoming(msg) {
      if (!msg || !msg.type) return;
      const t = now();
      if (msg.type === "ready" && readyT0 != null && phases.worker_ready_ms == null) {
        phases.worker_ready_ms = +(t - readyT0).toFixed(2);
        mark("worker_ready", { ms: phases.worker_ready_ms });
      }
      const p = pending.get(msg.type);
      if (!p) return;
      const ms = t - p.t0;
      pending.delete(msg.type);
      worker_msgs.push({ type: msg.type, ms: +ms.toFixed(2), kind: p.kind });
      if (p.kind === "open") {
        phases.open_roundtrip_ms = +ms.toFixed(2);
        mark("open_roundtrip", { ms: phases.open_roundtrip_ms });
      }
      if (p.kind === "frame") {
        phases.frame_rounds += 1;
        phases.frame_total_ms = +(phases.frame_total_ms + ms).toFixed(2);
        const cmdLen = msg.commands?.byteLength ?? null;
        if (phases.first_cmdbuf_bytes == null && cmdLen != null) {
          phases.first_cmdbuf_bytes = cmdLen;
          mark("first_cmdbuf", { bytes: cmdLen, frame_ms: +ms.toFixed(2) });
        }
        if (phases.first_measure_count == null) phases.first_measure_count = msg.measureCount | 0;
        if (phases.first_layout_count == null) phases.first_layout_count = msg.layoutCount | 0;
        ticks.push({
          n: phases.frame_rounds,
          frame_ms: +ms.toFixed(2),
          cmd_bytes: cmdLen,
          measureCount: msg.measureCount | 0,
          layoutCount: msg.layoutCount | 0,
          outlineCount: msg.outlineCount | 0,
          sheet_tail: msg.t !== false,
        });
      }
      if (p.kind === "measure") {
        phases.measure_rounds += 1;
        phases.measure_total_ms = +(phases.measure_total_ms + ms).toFixed(2);
      }
      if (p.kind === "layout") {
        phases.layout_rounds += 1;
        phases.layout_total_ms = +(phases.layout_total_ms + ms).toFixed(2);
      }
    },
    snapshot() {
      return {
        phases: { ...phases },
        ticks: ticks.map((x) => ({ ...x })),
        worker_msgs: worker_msgs.slice(),
      };
    },
  };
}

let activeSession = null;

export function getActiveTrace() {
  return activeSession;
}

export function beginTraceSession() {
  if (!activeSession) activeSession = createTraceSession();
  else activeSession.reset();
  return activeSession;
}

/** Wrap global Worker so every instance traces postMessage / onmessage. Call once. */
export function installWorkerMessageTrace() {
  if (typeof Worker === "undefined" || globalThis.__nxWorkerTraceInstalled) return;
  globalThis.__nxWorkerTraceInstalled = true;
  const Orig = Worker;
  function TracedWorker(url, opts) {
    const w = new Orig(url, opts);
    const origPost = w.postMessage.bind(w);
    w.postMessage = (msg, transfer) => {
      activeSession?.onOutgoing(msg);
      return origPost(msg, transfer);
    };
    // Observe without replacing onmessage (replacing broke WorkerDocument opens).
    w.addEventListener("message", (ev) => {
      activeSession?.onIncoming(ev?.data);
    });
    return w;
  }
  TracedWorker.prototype = Orig.prototype;
  Object.defineProperty(TracedWorker, "name", { value: "Worker" });
  globalThis.Worker = TracedWorker;
}
