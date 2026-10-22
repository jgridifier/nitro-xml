# nitro-xml (Python / PyPI)

Python bindings for [Nitro XML](https://github.com/jgridifier/nitro-xml): open `.xlsx`, `.docx`, and `.pptx`, read cell or plain text, and save with lossless round-trip. The extension module is built with [maturin](https://www.maturin.rs/) from the `nitro-xml-py` Rust crate (`rs/crates/nitro-xml-py`).

**This tree is not published to PyPI from this repository yet.** Use local builds for development and CI dry-runs.

## Requirements

- Python 3.9+
- Rust toolchain (see `rust-toolchain.toml` at the repo root)
- `maturin` and `pytest` (install via pip)

## Local development (Linux)

From this directory:

```bash
pip install maturin pytest
maturin develop --release
pytest
```

`maturin develop` compiles the PyO3 extension in place so `import nitro_xml` works without installing a wheel.

## Building wheels (dry run)

```bash
maturin build --release
```

Wheels land in `python/dist/`. Run the repo gate:

```bash
bash scripts/ci/pypi_build_dry_run.sh
```

## API sketch

```python
import nitro_xml

doc = nitro_xml.open_path("workbook.xlsx")  # or open_bytes(data, "workbook.xlsx")
assert doc.format == "xlsx"
print(doc.cell_text(0, "A1"))

doc = nitro_xml.open_path("report.docx")
print(doc.text())

out = doc.save()  # bytes
```

## Cross-platform wheels (release workflow, not in this PR)

For a future PyPI release, build per-OS wheels with one of:

| Platform | Typical approach |
|----------|------------------|
| Linux x86_64 / aarch64 | `cibuildwheel` on GitHub Actions (manylinux) or `maturin build` in a manylinux container |
| macOS x86_64 / arm64 | `cibuildwheel` with macOS runners, or local `maturin build` on each arch |
| Windows x86_64 | `cibuildwheel` on `windows-latest`, or `maturin build` with MSVC + Rust |

Upload is `twine upload dist/*` after signing/tagging; **do not publish until the project explicitly enables it.**

## Licence

The Python packaging metadata is MIT. The native extension links Rust crates that include Apache-2.0 ported sources; see the root `PROVENANCE.md` and crate `license` fields (`MIT AND Apache-2.0` where applicable).
