<!-- SPDX-License-Identifier: MIT -->
# Publishing names (availability check — do not register yet)

Read-only check of npm, crates.io and PyPI for the names Nitro XML intends to
publish under. **Nothing was registered.** Re-run with:

```bash
bash scripts/ci/check_publish_names.sh
```

Checked on **2026-10-22** (UTC) from this repo's CI-shaped agent environment,
with a descriptive `User-Agent` (crates.io returns 403 without one).

## Recommendation (final names)

| Registry | Recommended publish name | Import / use name | Notes |
|----------|--------------------------|-------------------|--------|
| **npm** | `nitro-xml` | `import … from "nitro-xml"` / `"nitro-xml/loader"` | Matches the existing package directory and PRODUCT_SPEC. |
| **npm** | `@nitro-xml/react` | `import { NitroViewer } from "@nitro-xml/react"` | Scoped React binding. Requires creating the `nitro-xml` npm org once (see checklist below). |
| **crates.io** | `nitro-xml` (umbrella) + `nitro-xml-{opc,render,xlsx,docx,pptx,wasm}` | `nitro_xml`, `nitro_xml_xlsx`, … | Hyphenated crate names; Rust converts `-` → `_` in paths. Publish internals first (or fold under one crate — follow-up packaging PR). |
| **PyPI** | `nitro-xml` | `import nitro_xml` | PyPI normalizes hyphens; module uses underscore. |

Alternates checked and also free (keep as fallback only): `nitroxml`, `nitro_xml`,
`nitro-ooxml`, `ooxml-nitro`, `nitro-office`, `jgridifier-nitro-xml`. Prefer the
primary set above so npm / crates / PyPI share one product name.

## Availability matrix

| Name | npm | crates.io | PyPI |
|------|:---:|:---------:|:----:|
| `nitro-xml` | **free** | **free** | **free** |
| `nitro_xml` | **free** | **free** | **free** |
| `nitroxml` | **free** | **free** | **free** |
| `@nitro-xml/react` | **free** | n/a | n/a |
| `@nitro-xml/core` | **free** | n/a | n/a |
| `@nitro-xml/loader` | **free** | n/a | n/a |
| `@nitro-xml/wasm` | **free** | n/a | n/a |
| `nitro-xml-opc` | n/a | **free** | **free** |
| `nitro-xml-render` | n/a | **free** | **free** |
| `nitro-xml-xlsx` | n/a | **free** | **free** |
| `nitro-xml-docx` | n/a | **free** | **free** |
| `nitro-xml-pptx` | n/a | **free** | **free** |
| `nitro-xml-wasm` | n/a | **free** | **free** |
| npm org `nitro-xml` | **missing** (scope not found) | n/a | n/a |

`free` means HTTP 404 from the registry API (name not claimed). This is not a
reservation; anyone can take a free name until Jared registers it.

## One-time steps for Jared (names only — no publish in this PR)

1. **npm**
   - Create an npm user (or use existing) at https://www.npmjs.com/signup
   - Create the org / scope **`nitro-xml`**: https://www.npmjs.com/org/create
   - Do **not** `npm publish` yet; packaging PRs land first. When ready, enable
     **provenance** on the GitHub Actions OIDC publish (release workflow PR).
2. **crates.io**
   - Log in via GitHub at https://crates.io/
   - Create an API token with publish scope; store as repo secret
     `CARGO_REGISTRY_TOKEN` (release workflow PR). Do not publish crates yet.
3. **PyPI**
   - Create account at https://pypi.org/account/register/
   - Prefer **Trusted Publishing** (OIDC) for the GitHub repo
     `jgridifier/nitro-xml` once the release workflow exists — no long-lived
     password. Project name to claim on first successful publish: `nitro-xml`.
4. **Re-check before first publish:**
   `bash scripts/ci/check_publish_names.sh` — abort if any recommended name
   flipped to taken.

## Out of scope here

- Registering or publishing any name
- npm / crates.io / PyPI package contents (follow-up PRs)
- `.github/workflows/release.yml` (separate PR; agents normally do not edit
  workflows — that PR will ask Jared to merge the YAML)
