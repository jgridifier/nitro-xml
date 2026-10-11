// SPDX-License-Identifier: MIT
//! Host-only bridge: render via upstream `chart_ref.ts` from a serialized [`ChartSpace`].
//! WASM builds use a stub until the Rust S1 painter reaches byte parity (#28).

use nitro_xml_opc::chart::ChartSpace;
use std::io::Write;
use std::process::{Command, Stdio};

use super::RenderChartSpaceOptions;

fn repo_root() -> Option<std::path::PathBuf> {
    if let Ok(root) = std::env::var("NITRO_XML_REPO") {
        return Some(std::path::PathBuf::from(root));
    }
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    Some(dir)
}

/// Renders SVG through the pinned upstream oracle (`tools/model-compare/gates/ref/chart_ref.ts`).
pub fn render_chart_space_svg_oracle(
    space: &ChartSpace,
    options: &RenderChartSpaceOptions,
) -> Option<String> {
    let root = repo_root()?;
    let script = root.join("tools/model-compare/gates/ref/chart_ref.ts");
    if !script.exists() {
        return None;
    }
    let upstream = std::env::var("UPSTREAM_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| root.join("upstream"));
    let payload = serde_json::json!({
        "chartSpace": space,
        "render": { "width": options.width, "height": options.height }
    });
    let mut child = Command::new("bun")
        .arg(script)
        .arg("svg-json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("UPSTREAM_DIR", upstream)
        .spawn()
        .ok()?;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .ok()?;
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}
