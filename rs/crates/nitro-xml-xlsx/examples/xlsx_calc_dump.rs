// SPDX-License-Identifier: MIT
//! Dump dependency-graph recalculation for one or more xlsx files (#72).

use std::process;

use nitro_xml_xlsx::formula::load_calc_engine;

fn main() {
    let args: Vec<String> = std::env::args()
        .skip(1)
        .map(|arg| arg.to_string())
        .collect();
    if args.is_empty() {
        eprintln!("usage: xlsx_calc_dump <file.xlsx>...");
        process::exit(2);
    }
    print!("{{\"results\":[");
    for (index, path) in args.iter().enumerate() {
        if index > 0 {
            print!(",");
        }
        let file = serde_json::to_string(path).unwrap_or_else(|_| "\"\"".to_owned());
        match std::fs::read(path) {
            Err(err) => {
                let message =
                    serde_json::to_string(&err.to_string()).unwrap_or_else(|_| "\"\"".to_owned());
                print!("{{\"file\":{file},\"error\":{message}}}");
            }
            Ok(bytes) => match load_calc_engine(&bytes) {
                Err(err) => {
                    let message = serde_json::to_string(&err).unwrap_or_else(|_| "\"\"".to_owned());
                    print!("{{\"file\":{file},\"error\":{message}}}");
                }
                Ok(mut engine) => {
                    engine.prepare(None);
                    engine.recalculate_all();
                    print!("{{\"file\":{file},\"cells\":{}}}", engine.cells_json());
                }
            },
        }
    }
    println!("]}}");
}
