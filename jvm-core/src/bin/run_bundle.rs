use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 5 {
        eprintln!("usage: run_bundle <bundle.bin>... <main_class> <method> <descriptor>");
        std::process::exit(2);
    }
    // Last 3 args are class, method, descriptor; everything before is bundle files
    let bundle_files = &args[1..args.len() - 3];
    let main_class = &args[args.len() - 3];
    let method = &args[args.len() - 2];
    let descriptor = &args[args.len() - 1];

    let mut combined = Vec::new();
    for path in bundle_files {
        let data = fs::read(path).unwrap_or_else(|e| {
            eprintln!("failed to read bundle {}: {}", path, e);
            std::process::exit(2);
        });
        combined.extend_from_slice(&data);
    }
    let out = jvm_core::run_static_native(&combined, main_class, method, descriptor);
    println!("{out}");
}
