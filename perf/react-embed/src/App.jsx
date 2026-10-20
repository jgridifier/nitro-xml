// SPDX-License-Identifier: MIT
import { useCallback, useEffect, useRef, useState, StrictMode } from "react";
import { NitroHost } from "./NitroHost.jsx";
import {
  openInElement,
  editAndRepaint,
  cellTypeSpec,
  typeKey,
  boldEdit,
  percentile,
  fixtures,
} from "./bench-api.js";

function params() {
  const q = new URLSearchParams(location.search);
  return {
    scenario: q.get("scenario") || "startup",
    format: q.get("format") || "xlsx",
    tier: q.get("tier") || "S",
    strict: q.get("strict") === "1",
    samples: Number(q.get("samples") || 20),
    smoke: q.get("smoke") === "1",
  };
}

async function ensureDefined(kind = "embed") {
  const {
    defineNitroXlsx,
    defineNitroDocx,
    defineNitroPptx,
    initCore,
    prewarmWorker,
  } = await import("nitro-xml/loader");
  const t0 = performance.now();
  if (kind === "startup") {
    // Host-side cold init (session.js path / explicit initCore callers).
    await initCore();
  } else {
    // React embed path: compile + Module-seeded worker prewarm only — no host
    // instantiate (WorkerDocument owns the single wasm instance).
    const { preload } = await import("nitro-xml/loader/lazy/preload.js");
    await preload();
  }
  const init_ms = performance.now() - t0;
  defineNitroXlsx();
  defineNitroDocx();
  defineNitroPptx();
  prewarmWorker();
  return { init_ms };
}

function finish(result) {
  window.__nitroBench?.finish({
    ok: true,
    ...result,
    wasm: window.__nitroBench.state.wasm,
    worker: window.__nitroBench.state.worker,
    marks: window.__nitroBench.state.marks,
  });
}

function fail(err) {
  console.error(err);
  window.__nitroBench?.fail(err);
}

async function runEdits(el, kind, n) {
  const samples = [];
  for (let i = 0; i < n; i++) {
    if (kind === "cell") {
      samples.push(await editAndRepaint(el, cellTypeSpec(i)));
    } else if (kind === "docx" || kind === "pptx") {
      // Real host path: one keystroke via editorKey (applyEdit office path is
      // not what React embeds call for typing).
      samples.push(await editAndRepaint(el, (e) => typeKey(e, "x")));
    } else if (kind === "bold") {
      samples.push(await editAndRepaint(el, boldEdit));
    } else {
      throw new Error(`unknown edit kind ${kind}`);
    }
  }
  return {
    n: samples.length,
    p50_ms: percentile(samples, 50),
    p95_ms: percentile(samples, 95),
    samples_ms: samples.map((x) => +x.toFixed(3)),
  };
}

function ScenarioRunner({ cfg }) {
  const [format, setFormat] = useState(cfg.format);
  const [mountKey, setMountKey] = useState(0);
  const [mounted, setMounted] = useState(true);
  const hostRef = useRef(null);
  const ran = useRef(false);
  const mountTimes = useRef([]);

  const onReady = useCallback((el) => {
    mountTimes.current.push(performance.now());
    window.__nitroBench?.mark("host_ready", { format: el.tagName });
  }, []);

  useEffect(() => {
    if (ran.current) return;
    ran.current = true;
    (async () => {
      try {
        const { scenario, format: fmt, tier, samples, smoke } = cfg;
        window.__nitroBench?.mark("scenario_start", { scenario, format: fmt, tier });

        if (scenario === "startup") {
          const { init_ms } = await ensureDefined("startup");
          finish({
            scenario,
            init_core_ms: init_ms,
            note: "cold initCore + define tags + prewarmWorker",
          });
          return;
        }

        const { init_ms } = await ensureDefined("embed");

        const waitEl = async () => {
          const deadline = performance.now() + 10000;
          while (performance.now() < deadline) {
            const el = hostRef.current?.element;
            if (el) return el;
            await new Promise((r) => setTimeout(r, 10));
          }
          throw new Error("host element not ready");
        };

        if (scenario === "first-paint") {
          const el = await waitEl();
          const paint = await openInElement(el, fmt, tier);
          finish({ scenario, format: fmt, tier, init_core_ms: init_ms, ...paint });
          return;
        }

        if (scenario === "remount") {
          const el = await waitEl();
          const first = await openInElement(el, fmt, tier);
          setMounted(false);
          await new Promise((r) => setTimeout(r, 50));
          setMounted(true);
          setMountKey((k) => k + 1);
          await new Promise((r) => setTimeout(r, 50));
          const el2 = await waitEl();
          const second = await openInElement(el2, fmt, tier);
          finish({
            scenario,
            format: fmt,
            tier,
            init_core_ms: init_ms,
            first,
            second,
            remount_delta_ms: second.mount_to_paint_ms - first.mount_to_paint_ms,
          });
          return;
        }

        if (scenario === "strict-double-mount") {
          // With StrictMode the effect mounts twice; record ready timestamps.
          await new Promise((r) => setTimeout(r, 100));
          const el = await waitEl();
          const paint = await openInElement(el, fmt, tier);
          finish({
            scenario,
            format: fmt,
            tier,
            init_core_ms: init_ms,
            host_ready_count: mountTimes.current.length,
            host_ready_gaps_ms: mountTimes.current.slice(1).map((t, i) => t - mountTimes.current[i]),
            ...paint,
          });
          return;
        }

        if (scenario.startsWith("edit-")) {
          const el = await waitEl();
          const editTier = tier || "S";
          await openInElement(el, fmt, editTier);
          const n = smoke ? 3 : samples || 50;
          let kind = "cell";
          if (scenario === "edit-docx-text") kind = "docx";
          if (scenario === "edit-pptx-text") kind = "pptx";
          if (scenario === "edit-bold") kind = "bold";
          if (scenario === "edit-cell") kind = "cell";
          const edit = await runEdits(el, kind, n);
          finish({
            scenario,
            format: fmt,
            tier: editTier,
            init_core_ms: init_ms,
            edit,
            heap_after: performance.memory ? performance.memory.usedJSHeapSize : null,
          });
          return;
        }

        finish({ scenario, error: `unknown scenario ${scenario}`, init_core_ms: init_ms });
      } catch (e) {
        fail(e);
      }
    })();
  }, [cfg]);

  // Keep format in sync for host
  useEffect(() => {
    setFormat(cfg.format);
  }, [cfg.format]);

  if (cfg.scenario === "startup") {
    return <div data-scenario="startup">startup (no viewer)</div>;
  }

  if (!mounted) return <div data-unmounted="1" />;

  return (
    <NitroHost
      key={`${format}-${mountKey}`}
      ref={hostRef}
      format={format}
      onReady={onReady}
    />
  );
}

export default function App() {
  const cfg = params();
  const body = <ScenarioRunner cfg={cfg} />;
  if (cfg.strict || cfg.scenario === "strict-double-mount") {
    return <StrictMode>{body}</StrictMode>;
  }
  return body;
}

// silence unused import warning in some bundlers
void fixtures;
