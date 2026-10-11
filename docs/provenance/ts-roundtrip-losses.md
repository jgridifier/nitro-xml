# TS round-trip losses (E0 gate a)

Machine-readable inventory: [`parity/roundtrip/ts-losses.json`](../../parity/roundtrip/ts-losses.json).

## Purpose

Upstream’s own README says saving is not lossless. Before the Rust writer
([#65](https://github.com/jgridifier/nitro-xml/issues/65)) lands, E0 records
what the pinned TS engine (`ChristopherVR/ooxml@e66aaec`) drops or rewrites on
a no-op open→save across every redistributable package in
`fixtures/manifest.json` (xlsx / xlsm / xltx, docx / docm / dotx, pptx / pptm /
potx).

That list is the bar: WASM must meet or beat TS. The allow-list **ratchets
down only**.

## How it is produced

```bash
bash scripts/ci/roundtrip.sh
# or: cd parity/roundtrip && bun install && bun ./run.mjs --ci
```

Comparer (PRODUCT_SPEC §5 gate a):

- same part set and content types
- relationships equal on `(Id, Type, Target, TargetMode)`
- binary parts byte-equal (`vbaProject.bin` must survive)
- XML parts equal after C14N (prefixes → namespaces, attribute order ignored,
  insignificant whitespace ignored except `xml:space="preserve"`)
- unknown parts, `extLst`, and `mc:AlternateContent` flagged when they change

Open XML SDK validator subset checks and LibreOffice headless reopen are
**wired** but soft-skip until [#48](https://github.com/jgridifier/nitro-xml/issues/48)
installs the tools via `scripts/ci/setup-validators.sh`.

## Reading `ts-losses.json`

| Field | Meaning |
|---|---|
| `counts.byFormat` | Per-family lossless / lossy fixture counts |
| `counts.vba` | Macro fixtures where `vbaProject.bin` was preserved |
| `topLossCategories` | Aggregated loss reasons (xml-rewrite, rels, …) |
| `hardRequirements` | Fixture ids where TS is already lossless — WASM must match |
| `allowList` | Lossy fixtures with per-part reasons |

Regenerate and commit `ts-losses.json` whenever the upstream pin or the corpus
changes, and only when the allow-list shrinks or stays equal.

## Snapshot (corpus @ pin e66aaec)

| Family | Lossless | Lossy | Notes |
|---|---:|---:|---|
| xlsx (+ xlsm / xltx) | 0 | 46 | Writer regenerates workbook/sheets/styles/strings |
| docx (+ docm / dotx) | 79 | 1 | Unchanged model returns original bytes; one non-zip row errors |
| pptx (+ pptm / potx) | 9 | 108 | Slide re-serialization + docProps churn |
| **vbaProject.bin** | 10/10 preserved | | Macro packages carry the binary unchanged |

Top loss categories (part-level hits): `docProps`, `xml-rewrite`, `rels`,
`extLst`, `content-types`, `sharedStrings`, `styles`, `calcChain`.
