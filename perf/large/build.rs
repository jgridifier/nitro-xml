// SPDX-License-Identifier: MIT
fn main() {
    let main_bench = std::env::var("NITRO_PPTX_MAIN_BENCH").is_ok();
    println!("cargo::rustc-check-cfg=cfg(pptx_main_bench)");
    if main_bench {
        println!("cargo::rustc-cfg=pptx_main_bench");
    }
}
