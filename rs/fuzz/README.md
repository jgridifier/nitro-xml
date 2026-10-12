# SPDX-License-Identifier: MIT

# nitro-xml cargo-fuzz targets (Part of #30)

Parser-hardening fuzz harnesses for the shared foundation and each format entry
point. The main `rs/` workspace stays on the pinned stable toolchain;
`rust-toolchain.toml` here selects nightly for libFuzzer.

## Targets

| Target | Entry point |
|--------|-------------|
| `xml_parse` | `nitro_xml_opc::xml::parse_xml_bytes` |
| `zip_open` | `ZipPackage::open` + read every part |
| `opc_open` | `Package::open` |
| `xlsx_load` | `nitro_xml_xlsx::load_workbook` |
| `docx_load` | `nitro_xml_docx::read_package` |
| `pptx_load` | `Presentation::open` |
| `safe_href` | `is_safe_hyperlink_href` / `hyperlink_policy` |

## Seeds

Curated seeds live in `seed/<target>/` (from `fixtures/`). Limit / crash fixtures
and legitimate high-compression cases live in `fixtures/fuzz/`.
`scripts/ci/fuzz-smoke.sh` copies both into `corpus/<target>/` and replays with
`-runs=0`.

## Default security limits (`ZipLimits` / `ParseOptions`)

| Limit | Default | Notes |
|-------|---------|-------|
| `max_input_bytes` | 50 MiB | upstream parity |
| `max_parts` | 10 000 | upstream parity |
| `max_total_bytes` | 300 MiB | upstream parity |
| `max_part_bytes` | 200 MiB | upstream parity |
| `max_compression_ratio` | **1000** | safer than upstream; `0` disables |
| `min_ratio_check_bytes` | **10 MiB** | ratio ignored below this size |
| `max_depth` (XML) | **4096** | safer than upstream (unbounded); above parity `deep.xml` (2000) |

Use `ZipLimits::upstream_parity()` / `PackageOpenOptions::upstream_parity()` for
absolute caps only. Override any field via `ZipPackage::open_with_limits` or
`Package::open_with_options`.

## Local campaign

```bash
cd rs/fuzz
cargo fuzz run xml_parse -- -max_total_time=3600
```

Crashes land in `artifacts/<target>/`. Minimise and copy into `fixtures/fuzz/`
(not a unit test) before fixing.
