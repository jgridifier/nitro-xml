<!-- SPDX-License-Identifier: MIT -->
# Releasing Nitro XML (npm · crates.io · PyPI)

Single version source: repo-root [`VERSION`](../VERSION). Git tags must be
`v$(cat VERSION)` (for example `v0.1.0`).

## Dry-run (default)

```bash
DRY_RUN=1 bash scripts/ci/release_publish.sh
```

Or push a `v*` tag / run the **release** workflow with dry-run enabled. No
packages are uploaded while `DRY_RUN=1`.

## One-time setup (Jared)

See also [PUBLISHING_NAMES.md](PUBLISHING_NAMES.md) for name registration.

### npm
1. Create user + org/scope **`nitro-xml`**.
2. Create granular access token (automation) with publish on `nitro-xml` and
   `@nitro-xml/*`.
3. Repo secret **`NPM_TOKEN`** (used as `NODE_AUTH_TOKEN`).
4. Provenance: workflow already has `id-token: write` and `npm publish --provenance`.

### crates.io
1. Log in with GitHub at https://crates.io/
2. Create API token → repo secret **`CARGO_REGISTRY_TOKEN`**.
3. First publish order: `nitro-xml-opc` → `nitro-xml-render` → format crates →
   `nitro-xml` → `nitro-xml-wasm` (see `scripts/ci/crates_package_dry_run.sh`).

### PyPI (trusted publishing / OIDC)
1. Create account at https://pypi.org/
2. Project **`nitro-xml`**: Publishing → Trusted publishers → add GitHub
   - Owner: `jgridifier`
   - Repository: `nitro-xml`
   - Workflow: `release.yml`
   - Environment: `release`
3. No long-lived PyPI password required when OIDC is configured.
4. Create GitHub Environment **`release`** (optional reviewers).

### Flip dry-run off
Repository variable **`RELEASE_DRY_RUN=false`** (Actions → Variables), or
`workflow_dispatch` with dry-run unchecked. Keep `true` until the first
intentional publish.

## Do not
- Commit tokens
- Raise loader / perf budgets in a release PR
- Edit branch protection from agents
