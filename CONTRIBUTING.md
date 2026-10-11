# Contributing to Nitro XML

> **Testing rule:** no coverage targets; the differential suites against upstream are the main check; a new bug becomes a corpus file first and a unit test only when no file can isolate it; each unit test's first line names the bug it catches. See [AGENTS.md §0](AGENTS.md#0-testing-rule-top-level-from-jared-see-the-testing-policy-in-docsproduct_specmd).

Thanks for helping. Most work here is done by coding agents in parallel, so the rules are strict and short.

1. **Read [AGENTS.md](AGENTS.md).** It is the coordination contract: how to pick a ready issue, claim it (`claiming` comment plus assignee or `agent:<name>` label), branch (`issue-<N>-<slug>`), open a small PR with `Closes #N`, run the gates locally and post a handoff comment.
2. **Gates** are defined in [docs/PRODUCT_SPEC.md](docs/PRODUCT_SPEC.md) §5 and run by `scripts/ci/*.sh`. Required checks are added per gate as each one is implemented.
3. **Never edit `.github/workflows/`.** Ask in your PR if YAML must change.
4. **Licence.** Contributions are accepted under the MIT licence. Files ported from upstream ooxml keep `SPDX-License-Identifier: Apache-2.0`, a provenance line and a PROVENANCE.md row. Dependencies must be MIT, Apache-2.0, BSD, ISC, Zlib or Unicode licensed. `bash scripts/ci/licenses.sh` (job `licenses`, every PR and main) enforces it: cargo-deny on the dependency graph (`deny.toml`), crate `license` fields (`MIT`, or `MIT AND Apache-2.0` for a crate holding ported files), an SPDX header on every source file, a PROVENANCE.md row for every Apache-2.0 file, and the same allow-list for npm production dependencies of `packages/*`. Exceptions go in `deny.toml` with a reason and the approving PR.
5. **Fixtures.** Only files whose licence permits redistribution, recorded in `fixtures/manifest.json`.
6. **Humans** are welcome too: open an issue first for anything not already on the [board](https://github.com/users/jgridifier/projects/2).
