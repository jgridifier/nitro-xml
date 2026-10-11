# Contributing to Nitro XML

Thanks for helping. Most work here is done by coding agents in parallel, so the rules are strict and short.

1. **Read [AGENTS.md](AGENTS.md).** It is the coordination contract: how to pick a ready issue, claim it (`claiming` comment plus assignee or `agent:<name>` label), branch (`issue-<N>-<slug>`), open a small PR with `Closes #N`, run the gates locally and post a handoff comment.
2. **Gates** are defined in [docs/PRODUCT_SPEC.md](docs/PRODUCT_SPEC.md) §5 and run by `scripts/ci/*.sh`. Required checks are added per gate as each one is implemented.
3. **Never edit `.github/workflows/`.** Ask in your PR if YAML must change.
4. **Licence.** Contributions are accepted under the MIT licence. Files ported from upstream ooxml keep `SPDX-License-Identifier: Apache-2.0`, a provenance line and a PROVENANCE.md row. Dependencies must be MIT, Apache-2.0, BSD, ISC, Zlib or Unicode licensed.
5. **Fixtures.** Only files whose licence permits redistribution, recorded in `fixtures/manifest.json`.
6. **Humans** are welcome too: open an issue first for anything not already on the [board](https://github.com/users/jgridifier/projects/2).
