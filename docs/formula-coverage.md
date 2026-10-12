# Formula coverage

Upstream `FUNCTION_CATALOG` (`ChristopherVR/ooxml` @ `e66aaec`) has **481** functions. Rust `get_function` / `formula::fns::registered_functions` implements **469**. The gap is **12**, and it is only the Database family.

`scripts/ci/editor_recalc.sh` rechecks this inventory. A name that appears in the catalog and is not in the table below fails the gate.

## Ported families

| Family | Catalog | Rust | Issue |
|---|---:|---:|---|
| Math & Trig | 79 | 79 | #73 |
| Statistical | 145 | 145 | #73 |
| Logical | 19 | 19 | #74 |
| Text | 44 | 44 | #74 |
| Information | 19 | 19 | #74 |
| Date & Time | 25 | 25 | #74 |
| Lookup & Reference | 38 | 38 | #75 |
| Financial | 50 | 50 | #75 |
| Engineering | 50 | 50 | #75 |
| Database | 12 | 0 | approved gap (next follow-up) |
| **Total** | **481** | **469** | |

Lookup & Reference includes the dynamic-array functions (`FILTER`, `SORT`, `UNIQUE`, `SEQUENCE`, and the rest of that family).

## Approved gap

The only unported catalog names are the 12 Database functions. They are approved for this PR; the next follow-up registers them. Nothing else is missing.

- `DAVERAGE`
- `DCOUNT`
- `DCOUNTA`
- `DGET`
- `DMAX`
- `DMIN`
- `DPRODUCT`
- `DSTDEV`
- `DSTDEVP`
- `DSUM`
- `DVAR`
- `DVARP`

## Editor recalculation

`rs/crates/nitro-xml-xlsx/src/editor/` ports `session-calc.ts`. After each xlsx edit step (apply, undo, redo) the session calculator runs:

- cell edits: `recalculate_from` on the cells the step touched
- structural edits (row/column insert and delete, sheet add/rename/delete, batch): full reload of the host grid, `invalidate`, then `recalculate_all`

Rust `CalcEngine` has no `shift_cells` or `rename_sheet`, so a shift or rename hint cannot move the graph in place. Reloading the cells the host already shifted and recalculating everything is the follow path for those hints.

Formula and spill values are copied back onto `XlsxHost` sheets. `model_snapshot` and undo snapshots therefore show the calculated result instead of the null left when a formula is typed. Manual calculation mode (`calcMode: manual`) keeps that null until Calculate Now.

Workbook recalculation evaluates registered calls (`FormulaAst::Call` and `Invoke`) through `formula::fns`. A name that is not in the catalog, or a formula that did not parse, keeps a stored value when one exists and is `#NAME?` otherwise. The 12 Database functions are catalog names with no implementation, so they are not keep-cached and evaluate to `#NAME?`. `NOW` and `RAND` use the pinned harness clock (2026-10-03 12:00) and the seeded random sequence.
