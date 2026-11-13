// SPDX-License-Identifier: MIT
//! Measures the large-paragraph regression fixture in a fresh process. Pass a DOCX path.
//! Reports live/requested allocation peaks separately from Linux process peak RSS (KiB).
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

struct Meter;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);
fn add(size: usize) {
    let live = LIVE.fetch_add(size, Relaxed) + size;
    PEAK.fetch_max(live, Relaxed);
    CALLS.fetch_add(1, Relaxed);
}
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            add(layout.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) };
        LIVE.fetch_sub(layout.size(), Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(p, layout, size) };
        if !p.is_null() {
            LIVE.fetch_sub(layout.size(), Relaxed);
            add(size);
        }
        p
    }
}
#[global_allocator]
static ALLOCATOR: Meter = Meter;
fn main() {
    let path = std::env::args().nth(1).expect("DOCX path");
    let bytes = std::fs::read(&path).expect("read fixture");
    if std::env::args().any(|a| a == "--xml") {
        let mut pkg = nitro_xml_opc::opc::Package::open(&bytes).unwrap();
        let xml = pkg.take_text("word/document.xml").unwrap().unwrap();
        let doc = nitro_xml_opc::xml::parse_xml_str(&xml).unwrap();
        let mut stack = vec![&doc.root];
        let (mut elements, mut slots, mut used) = (0, 0, 0);
        while let Some(el) = stack.pop() {
            elements += 1;
            slots += el.children.capacity();
            used += el.children.len();
            for n in &el.children {
                if let nitro_xml_opc::xml::Node::Element(e) = n {
                    stack.push(e);
                }
            }
        }
        println!("xml_elements={elements} child_slots={slots} used_slots={used} node_size={} live_bytes={} peak_bytes={} allocations={}", std::mem::size_of::<nitro_xml_opc::xml::Node>(), LIVE.load(Relaxed), PEAK.load(Relaxed), CALLS.load(Relaxed));
        return;
    }
    let start = Instant::now();
    let loaded = nitro_xml_docx::read_package(&bytes).expect("load fixture");
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    let live = LIVE.load(Relaxed);
    let peak = PEAK.load(Relaxed);
    let allocations = CALLS.load(Relaxed);
    let rss = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .map(str::to_owned)
        });
    println!("path={path} blocks={} load_ms={ms:.3} live_bytes={live} peak_bytes={peak} allocations={allocations} {}", loaded.model.blocks.len(), rss.unwrap_or_default());
}
