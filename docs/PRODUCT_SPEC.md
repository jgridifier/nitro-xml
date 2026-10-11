# Nitro XML: product spec (v3, full Rust/WASM)

**Repo** `jgridifier/nitro-xml` · **Licence** MIT; files ported from upstream carry Apache-2.0. The repo keeps upstream's LICENSE text and NOTICE, and PROVENANCE.md states the changes per ported file. The README should say this in one line. · **Basis** PLAN.md and issues.json, plus this spec, which wins wherever they disagree. Earlier versions: `PRODUCT_SPEC_v1_readonly.md`, `PRODUCT_SPEC_v2_final.md`.

**Design lock (Jared):** "This is a full Rust/WASM project. If something doesn't exist we will build it." The aim is the fastest, most efficient, highest-quality Rust/WASM implementation of upstream, for use inside a large competitive organisation.
- **Everything that ships is Rust/WASM.** That covers the package, model, commands, history and writer, **and** the renderers, editor UI (ribbons, chrome, dialogs), text input, selection and caret.
- **The only JS is a minimal loader.** It instantiates the wasm and forwards browser events, and it has its own gated size budget.
- **Not shipped:** ProseMirror, the upstream Lit UI and the TS painters.
- **The TS engine is test-only:** correctness oracle, perf baseline and pixel reference. It is never shipped and there is no runtime fallback.
- No bandaids: one model, one engine, one code path.

## 1. Users, job, non-goals
- **Primary user: embedding developers inside a large organisation, starting with Jared's apps.**
  - One npm package (`nitro-xml`): a component per format with a `readonly` mode, plus a headless API (`open → handle`, queries, commands, `save`, `free`).
  - It must work under strict enterprise CSP and without COOP/COEP (§3).
- **Secondary: the public demo and the scoreboard dashboard on Pages.**
- **Core job:** open, view, edit and save .xlsx, .docx and .pptx **entirely in the browser** (bytes never leave the page). That means:
  - output faithful to the TS reference
  - lossless round trips
  - beating TS and the best alternatives on speed and memory (§5)
- **Renderer requirements.** The technology choice is a joint decision with Jared (Q1). Whatever is chosen must deliver:
  - real selectable and copyable text
  - screen-reader accessibility (an accessible tree or ARIA mirror)
  - full keyboard navigation
  - IME composition
  - bidi/RTL
  - font fallback and text shaping
  - crisp high-DPI output
  - print-quality fidelity to the TS reference within the §5 pixel thresholds
- **When WASM cannot run** (unsupported browser, CSP without `'wasm-unsafe-eval'`, instantiate failure), the component shows a clear "unsupported browser" message. Nothing degrades silently.
- **Non-goals:**
  - macro execution (never); VBA and unknown parts are preserved byte for byte
  - Visio, Teams and MCP
  - framework bindings beyond the web component and vanilla
- **ASK:**
  - **IN alongside .docx/.xlsx/.pptx (2026-10-11):** macro-enabled, template and macro-enabled template variants .xlsm/.docm/.pptm, .xltx/.dotx/.potx and .xltm/.dotm/.potm (same package format; macros never run; vbaProject.bin preserved byte for byte). R0 (#33) was signed off by Head of Product on 2026-10-11; #23 and #27 additionally wait on the post-apply read-back spec and the emf-converter/mtx-decompressor Rust replacements.
  - **OUT (Jared, 2026-10-11: Word, Excel and PowerPoint only for now):** legacy .doc/.xls/.ppt, encrypted files, CSV and every other upstream format or extra. These show an "unsupported format" message.
  - real-time collaboration (default OUT)
  - PDF/image export (default OUT)

## 2. Scope and milestones (Excel → Word → PowerPoint in both)
**M1, viewer.** Stages 0–2 (scaffold, perf baseline, OPC), then:

| Stage | Scope |
|---|---|
| **R0** | **Renderer: port upstream's approach as-is (Jared, 2026-10-11).** The Rust renderers reproduce what upstream's painters do today (same rendering technology and structure, now driven from Rust/WASM), so parity with the TS reference is direct. No DOM-vs-canvas spike now. Jared redesigns and iterates on the renderers after the conversion is done. R0 records the upstream painter inventory per format, the Rust module mapping, the worker/main-thread split this implies, and the loader size budget |
| **3** | xlsx: model plus a Rust grid renderer (sheet tabs, frozen panes, conditional formats, charts) |
| **4** | docx: model, pagination and a Rust page renderer |
| **5** | pptx: model, DrawingML and a Rust slide renderer covering **all** element kinds. There is no per-slide fallback, and anything unrendered is a failing parity item |
| **6** | Viewer shell in Rust: zoom, navigation, find, copy, hyperlinks, diagnostics |
| **7** | Hardening, fuzzing, crash recovery and the M1 report. **This is the M1 gate** |

**M2, editor** (after the M1 gate and Q3):

| Stage | Scope |
|---|---|
| **E0** | Editor harnesses (round trip, edit scripts, undo, validators) and the TS-editor perf baseline, plus the Rust editor chrome framework (ribbon, dialogs, commands, shortcuts) |
| **E-text** | **Rust text-editing surface:** caret, selection, IME, bidi, clipboard, spellcheck hook, accessibility, mobile keyboards. A single-line part lands before E1 (cell editor and formula bar); the rich multi-paragraph part lands before E3 |
| **E1** | xlsx value and format edits, sheet operations, undo/redo, lossless save. Dependents are marked stale and `fullCalcOnLoad` is set on save |
| **E2** | xlsx formula engine in Rust, with recalculation |
| **E3** | docx editing (docx review features are ASK, default IN) |
| **E4** | pptx editing (speaker notes and thumbnails are ASK, default IN) |
| **E5** | Write-path hardening and the final report |

Already done or decided upstream of this spec:
- workflow skeletons and the AGENTS.md no-touch rule
- batched measurement moved to #14 (R0 must re-decide it if shaping moves into wasm)
- the #19 engine API for find, copy, hyperlinks and sheet tabs
- the issue for sourcing permissively licensed Excel-authored fixtures

## 3. Browsers and devices
- **Targets:** the last 2 major versions of Chrome, Edge, Firefox and Safari, including iOS/iPadOS. Layouts are checked at 375 px, at DPR 1 and 2.
- **WASM:** MVP plus bulk memory, sign-extension, reference types and BigInt, in one build. SIMD (128-bit) is on by default for hot loops (XML scanning, number parsing, column ops); it is in every target browser, so it is still one build.
- **CSP:** `'wasm-unsafe-eval'` is **required** (there is no fallback). The README and the error message say so.
- **Threads:** recommend **none by default.** SharedArrayBuffer needs COOP/COEP, which large-organisation embedders often cannot set. Keep single-threaded wasm plus one worker per document. A threaded build is opt-in only if Jared wants it (Q2). Only the perf harness serves COOP/COEP.
- **Measurement:** perf in Chromium. WebKit and Firefox get nightly functional, visual, accessibility and IME smoke tests.

## 4. Why Excel first
- The shortest path to a full open-to-first-paint path in Rust.
- The likeliest win: the streaming columnar parse plus the virtualised grid.
- The grid is the best first target for the renderer port.
- The xlsx edit commands are discrete, so they are the cleanest first write path.
- docx needs pagination plus the rich E-text surface. pptx is the largest area (about 146k production lines of core).

## 5. Acceptance gates (automated in CI unless marked)
**Viewer gates (M1):**
- **Parity:** zero view-IR diffs outside `known-diffs.json`, and each entry needs a reason.
- **Visual pixelmatch** against TS-rendered references (test-only), at DPR 1 and 2, plus a print-to-PDF render:
  - xlsx: ≤0.1% per viewport
  - docx: ≤0.5% per page, with the same page count
  - pptx: ≤0.5% per slide
- **Unsupported-content rate:** every coverage gap is a failing parity item. The rate may only ratchet down, and it must be zero for the format at its stage gate.
- **Perf gate (plan §4.1):**
  - **Every tier (S, M, L, XL), every format, at 1x and 4x CPU: WASM beats TS** on:
    - time to first paint (including wasm fetch, compile and instantiate)
    - cold parse
    - scroll/view-query latency
    - peak memory (JS heap plus wasm linear memory)
  - "Beats" means the median is lower and the 95% bootstrap CI of the WASM/TS ratio sits entirely below 1.0. A tie is a fail.
  - **Timing rule:** every commit posts numbers. Until a format has a full open-to-first-paint path in Rust, the gate is regression-versus-main only. From that commit on, the "beats TS at every tier" gate applies to that format and is **never relaxed**.
  - **No fallback of any kind.** If WASM loses on any tier, that is a red gate to fix by design: streaming compile, a cached compiled module, a pre-warmed worker, and a small core module with per-format code loaded on demand.
  - Stretch targets are set as ratios once BASELINE.md is frozen.
  - **Data path (design rules the gate enforces):**
    - one streaming pass from zip to columnar model, with no intermediate DOM tree
    - lazy per-sheet, per-page and per-slide parsing
    - no per-cell JS↔WASM calls; the loader only forwards events and batches
  - No long task over 200 ms.
- **Bundle and startup:** hard budget. The initial payload (wasm plus loader JS, brotli) is smaller than TS's for the same format.
  - There is a per-format KB ceiling, fixed at R0, that can only ratchet down. The ceiling covers fonts and shaping data.
  - Release builds use `opt-level=3`, LTO, `codegen-units=1` and `wasm-opt -O3`.
  - Size and time-to-interactive are both gated. The editor module loads only on edit.
- **Loader JS budget:** a separate KB ceiling for hand-written plus generated glue JS, fixed at R0 and ratchet-down only.
- **Renderer and editor-surface gates** (from the stage that introduces each surface):
  - **Accessibility:** axe-core reports zero serious or critical violations. A screen-reader smoke script compares a CDP `Accessibility.getFullAXTree` snapshot against the expected reading order and roles. NVDA and VoiceOver are a manual spot check per stage.
  - **IME:** CJK composition scripts via CDP `Input.imeSetComposition` and `Input.insertText` (composition start, update, commit and cancel) give the expected model text and caret.
  - **Bidi:** Arabic and Hebrew fixtures (mixed-direction runs, RTL paragraphs and sheets) pass the visual threshold, and logical-order copy is correct.
  - **Selection and caret parity:** scripted selections give the same copied text as the TS reference, and caret rectangles are within 1 CSS px.
- **TS's role:** test-only (oracle, baseline, pixel reference). It is never in a shipped bundle, and CI fails if a shipped artefact imports it.

**Public scoreboard (Jared's explicit ask).** A Developer owns the scoreboard harness issue.
- **Entrants.** The Developer verifies licence, upkeep and a fair setup before pinning each one.

| Format | Entrants |
|---|---|
| xlsx | SheetJS, ExcelJS (JS/TS); calamine, umya-spreadsheet (Rust) |
| docx | docx-preview, mammoth.js (JS); docx-rs (Rust) |
| pptx | PPTXjs (JS) |
| Full suite | the LibreOffice WASM build |
| Always | upstream ooxml TS |

- **Races:** parse-only, render-to-first-paint, and open-edit-save. Each project enters only the races it supports.
- **Fairness rules:**
  - pinned versions
  - same fixtures, machine and browser
  - each competitor in its documented fastest configuration
  - methodology and harness are public
  - losses are published as prominently as wins
- **Equivalence, so nobody wins by doing less:**
  - The parse race requires the same extracted output (all cell values and types, all text runs), checked against a reference.
  - The render race requires passing the visual threshold against the TS reference.
  - Entries that do not produce equivalent output are shown as "not comparable" and are not ranked.
- Results go on the Pages dashboard, published from every main commit.

**Editor gates (M2).** The TS editor stays the oracle for model results.

**(a) Lossless round trip.** Open → save with no edits must give a semantically equal package, on the full corpus.
- **Check:**
  - same part set and content types
  - relationships equal on (Id, Type, Target, TargetMode)
  - binary parts byte-equal
  - XML parts equal after canonicalisation: C14N, prefixes resolved to namespaces, attribute order ignored, whitespace ignored except under `xml:space="preserve"`
  - unknown parts, `extLst` and `mc:AlternateContent` present and unchanged
- **Tooling (pinned, installed by repo script, never by workflow YAML):** the `roundtrip` job only calls `scripts/ci/roundtrip.sh`, which first runs `scripts/ci/setup-validators.sh`. That script installs:
  1. a pinned .NET SDK via the official `dotnet-install.sh` into a cached tool dir, plus a small in-repo console tool `tools/ooxml-validate` built on the **Open XML SDK** (`DocumentFormat.OpenXml`, pinned version, `OpenXmlValidator` targeting `FileFormatVersions.Office2019`) that emits a JSON error set per file
  2. pinned **LibreOffice** headless (`soffice --headless --convert-to`) from apt

  Versions live in `scripts/ci/versions.env`, so upgrades are script PRs, not workflow pushes. These are CI-only test tools and never shipped.
- **Where TS gives byte-identical output, WASM must too.** The upstream docx README says a no-op save returns the original bytes.
- **Validity:**
  - The saved file's Open XML SDK validator error set must be a subset of the input's (no new errors).
  - LibreOffice headless reopens and converts it with exit 0 and the same sheet, page or slide count as the input.
  - Real Office "opens without repair" is a manual spot check per E-stage, on 10 fixtures per format.
- **Upstream's own README says saving is not lossless,** so E0 records TS's round-trip losses as an allow-list with reasons. It ratchets down, and WASM may never be worse than TS.

**(b) Edit round trip.**
- The same scripted op files (`edits/*.json`) run on the TS editor and on WASM.
- The saved-model diffs against the input must be identical, normalised as in (a).
- Reopening the saved file passes the viewer gates.

**(c) Undo/redo parity.**
- The model snapshot after every do, undo and redo step equals the TS editor's snapshot.
- Undoing N steps back to the start and then saving passes (a) against the original.

**(d) Edit-path perf.**
- **Metrics:** keystroke-to-paint (input event to a double rAF after commit), IME commit-to-paint, command latency, undo latency, save time (command to Blob), and wasm memory growth over a 1,000-op session.
- **Budget:** on every tier, keystroke-to-paint, undo and save beat the TS editor under the same CI-ratio rule, applied once the format's edit path exists. No absolute numbers until the E0 baseline exists.

**(e) One engine, and crash recovery.**
- There is only WASM, so the old "open on TS" routing is gone.
- The loader keeps the original bytes and an op journal.
- If WASM crashes during editing, the user gets a recoverable error. The journal replays into a fresh worker, and the last good in-memory save is offered if replay fails.
- If WASM crashes while viewing, the document reloads once in a fresh worker; if that fails, the user sees an error.
- **Every crash is a P0 bug.** Crash-recovery tests run in CI.

**Testing policy (Jared, 2026-10-11).** Few tests, each checking something non-obvious. No coverage targets. The differential suites (same fixtures through Rust and the TS oracle: parity, pixels, lossless round trip, edit and undo parity) are the main check. A new bug becomes a new fixture first, and a unit test only when no fixture can isolate it. A unit test exists only for a non-obvious invariant the suites could miss (e.g. Excel's fictitious 1900-02-29, shared-string index edges, odd zip layouts, IME composition order), and its first line says which bug it catches. Tests that restate the code are rejected in review. Upstream unit tests are not ported wholesale.

## 6. Definition of done
- **Stage 0.**
  - Builds natively and to wasm32; CI green; corpus verified.
  - LICENSE is MIT, with Apache-2.0 text, NOTICE and PROVENANCE kept for ported files.
  - **Done:** workflow skeletons and the AGENTS.md no-touch rule.
- **Stage 1.**
  - Harness, history, gate and dashboard are in place, and the TS BASELINE.md is frozen.
  - `ci.yml`, `perf.yml` and `pages.yml` are on main, from the first commit or pushed by Chief of Staff or Jared.
  - The skeleton jobs `roundtrip`, `edit-parity`, `undo-parity`, `a11y`, `ime` and `scoreboard` are reserved now.
- **Stage 2.** Part-graph parity, zip-bomb and DTD tests, and the transport decision.
- **R0.**
  - **R0 (decided):** mirror upstream's rendering approach in Rust. The DOM-vs-canvas comparison (1M-cell xlsx, 300-page docx; first paint, 60 fps scroll/zoom, keystroke-to-paint, memory, payload; must pass pixel, accessibility, selection/copy, CJK IME and find gates) is deferred to Jared's post-conversion renderer redesign and is kept here as the method for that work.
  - **Port rule:** what stays identical is the drawing approach, the structure and the output (pixel and parity gates against TS). The one allowed change: Rust batches each frame's draw and DOM-building calls into a single handoff across the WASM boundary (e.g. one command buffer per frame applied by the loader). No per-element WASM-to-JS calls in the render path.
  - **Painter inventory (R0 deliverable):** for each upstream painter, record format, canvas vs DOM output, what it draws, and its Rust module, so batching priorities are explicit.
  - **Redesign milestone:** DOM-vs-canvas, in-wasm shaping and open metric-compatible fonts sit in a separate post-port milestone owned by Jared; not claimable until he opens it.
  - **Jared's decisions (2026-10-11):**
    - **P24 slide show:** not needed now. It stays an optional, deferred row for later; the PowerPoint focus is the editor (#122).
    - **Font source (#121):** configurable by the host, because most organisations run internal font mirrors. The default matches upstream (remote webfont fetch), with a host option to point at an internal mirror URL or turn remote fetches off entirely. With fetches off, rendering uses locally available fonts and records the substitution in diagnostics.
  - **Fonts and text measurement follow upstream for now** (same fonts and measurement path the TS painters use), so line breaks and page counts match the TS reference. Moving shaping into wasm and choosing metric-compatible open fonts (Carlito, Caladea; Office fonts cannot be redistributed) is part of Jared's later renderer redesign.
  - Done when: the painter inventory (#34) covers every in-scope file including stylesheets; the Rust module mapping (#35), the worker/main-thread split (#36, including a batched per-frame DOM read-back channel for measurement) and the loader API plus JS size budget (#37) are merged; each inventory row is marked M1 or deferred; JS library dependencies of upstream painters are listed with their Rust replacement; Head of Product signs #33. Renderer issues then become claimable.
- **Stages 3, 4 and 5.** All viewer gates are green for the format, including accessibility and bidi. The unsupported-content rate is 0. The hard perf gate is armed and green, and the scoreboard races are published.
- **Stage 6.** Shell interactions pass the accessibility, selection and keyboard scripts on all target browsers.
- **Stage 7.** Fuzzing clean (1 CPU-hour per target), crash recovery tested, memory ceiling enforced, M1 report published.
- **E0.** Harnesses run TS-against-TS, validators run in CI, the TS baselines and losses are recorded, and the chrome framework passes accessibility.
- **E-text.** The IME, bidi, selection and caret, clipboard and accessibility gates pass on desktop and in the iOS/Android keyboard smoke tests.
- **E1 to E4.** The format passes gates (a)–(e) and the reopen passes the viewer gates.
- **E5.** Save-path fuzzing clean, the 1,000-op memory gate passes, and the final report is published.

## 7. Questions for Jared (reply "defaults" to accept all)
1. **Renderer technology: decided.** Port upstream's approach as-is; Jared iterates on renderer design after the conversion.
2. **Threads.** Default: no SharedArrayBuffer and no COOP/COEP requirement; single-threaded wasm plus a worker. A threaded build only on your request.
3. **Budget, said plainly.** The scope is now well beyond the plan's "roughly double". The plan sized only a 25–45k-line read-path port and kept the roughly 217k production lines of `src/ui` in TS; now the renderers, the editor UI and a text-editing surface are also built in Rust. No credible line or credit estimate exists until R0. Default: M1 first, and M2 issues are labelled `blocked-m2` until the M1 gate passes and you confirm the budget.
4. **iOS Safari.** Default: a full target (functional, accessibility and IME smoke tests on WebKit and real-device spot checks); perf is gated in Chromium.
5. **CI cadence.** Default:
   - **PRs:** S/M tiers at 1x, parity, round trip on the parity corpus, accessibility and IME.
   - **Main and nightly:** the full corpus at 1x and 4x, validators and scoreboard races.

## Appendix: the plan's open questions (changes since v2)
- Q3 "Keep the TS version alongside?" → TS stays, **for tests only**, permanently. It is never shipped.
- Q4 "Viewer edits?" → The viewer is now built in Rust (Stages R0 and 6).
- Q8 "Legacy and encrypted files" → OUT (Jared): only .docx, .xlsx and .pptx; others show an unsupported-format message.
- The others are unchanged from v2: MIT plus Apache-2.0 for ported files; new repo; full editor suite; browsers in §3; Excel → Word → PowerPoint; CI cadence in Q5.

## Risks and gaps
1. **The biggest risk is building a text-editing surface from scratch:** IME, bidi, spellcheck integration (no native spellcheck without a contenteditable or hidden input), accessibility, mobile virtual keyboards and selection handles. Browsers give these to editable DOM almost for free; owning them in Rust is a multi-stage effort.
2. **Renderer fidelity.** The TS painters were tuned against Office, and a fresh Rust renderer has to re-earn the pixel thresholds format by format, pptx effects especially.
3. **Fonts and shaping inside the size budget.** In-wasm shaping (e.g. rustybuzz/swash) plus fallback fonts adds weight to the wasm. The plan itself deferred it for that reason. Browser-side shaping keeps it out of the wasm but needs a measurement boundary. Decide at R0.
4. **No fallback means a wasm bug is an outage for that user.** Fuzzing, crash recovery, the journal replay and staged rollout (pinned versions for embedders) are critical.
5. **The plan needs a rewrite for the lock.** #18, #19, #23, #27 and #29 assume TS painters. #14 assumes browser measurement. The R0, E-text and E0–E5 issues do not exist yet. The `nitro-xml-*` rename and the MIT licence wording are still not applied to PLAN.md, issues.json, gen_issues.py and #1.
6. **Still open from v2:**
   - round-trip fidelity of unknown markup (the TS oracle is itself lossy)
   - real Office checks are manual only
   - iOS memory is unmeasured
   - small-file startup is now a hard gate
7. **Scoreboard risks.** A competitor's "fastest config" can be contested, and some entrants do much less work. The equivalence rule handles the second; publishing the harness and accepting corrections handles the first.
