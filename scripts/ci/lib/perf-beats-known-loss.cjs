// SPDX-License-Identifier: MIT
// Documented M1 peak_mem known-loss exceptions for perf_beats_ts / perf.sh.
// Cells stay measured and printed as LOSS; they do not block enforce readiness
// and do not fail PERF_GATE_MODE=enforce. Any other LOSS / TIE/LOSS still fails.
// Decision: https://github.com/jgridifier/nitro-xml/issues/30#issuecomment-6341154384

"use strict";

const KNOWN_LOSS_DECISION =
  "https://github.com/jgridifier/nitro-xml/issues/30#issuecomment-6341154384";

/** Exact format|tier|metric cells Jared accepted as M1 compile-floor LOSS. */
const KNOWN_LOSS_CELLS = Object.freeze([
  Object.freeze({ format: "xlsx", tier: "S", metric: "peak_mem" }),
  Object.freeze({ format: "docx", tier: "S", metric: "peak_mem" }),
  Object.freeze({ format: "docx", tier: "M", metric: "peak_mem" }),
]);

const KEYS = new Set(
  KNOWN_LOSS_CELLS.map((c) => `${c.format}|${c.tier}|${c.metric}`),
);

function isKnownLossCell(format, tier, metric) {
  return KEYS.has(`${format}|${tier}|${metric}`);
}

/**
 * Classify a non-WIN cell for gate exit / readiness.
 * Known-loss peak_mem cells remain measured LOSS but are not blocking.
 */
function classifyMeasuredLoss(format, tier, metric, verdict) {
  const label = `${format} ${tier} ${metric} ${verdict}`;
  const known = isKnownLossCell(format, tier, metric);
  return {
    label,
    knownLoss: known,
    blocking: !known,
  };
}

module.exports = {
  KNOWN_LOSS_DECISION,
  KNOWN_LOSS_CELLS,
  isKnownLossCell,
  classifyMeasuredLoss,
};
