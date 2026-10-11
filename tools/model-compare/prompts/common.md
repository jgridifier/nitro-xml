You are working on jgridifier/nitro-xml, a Rust/WASM port of ChristopherVR/ooxml, in this git worktree. Read AGENTS.md and docs/PRODUCT_SPEC.md first; they are authoritative (scope rules, licence headers, the testing rule in AGENTS.md §0). Read the issue with `gh issue view <N> --repo jgridifier/nitro-xml` (read-only: do not comment, label or push).

The upstream TypeScript source is in the `upstream/` submodule at the pinned commit (run `git submodule update --init` if it is empty). It is the reference for behaviour; it never ships.

When you are done, the acceptance gate `bash tools/model-compare/gates/issue-<N>.sh` must pass; you may run it yourself. Commit your work on the current branch as jgridifier <jgridifier@users.noreply.github.com> with a clear message. Do not push. Do not edit anything under tools/model-compare/ or .github/. Finish with a short summary of what you did and the gate result.
