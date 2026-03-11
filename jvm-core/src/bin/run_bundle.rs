use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        eprintln!("usage: run_bundle <bundle.bin> <main_class> <method> <descriptor>");
        std::process::exit(2);
    }
    let bundle = fs::read(&args[1]).unwrap_or_else(|e| {
        eprintln!("failed to read bundle {}: {}", args[1], e);
        std::process::exit(2);
    });
    let out = jvm_core::run_static_native(&bundle, &args[2], &args[3], &args[4]);
    println!("{out}");
}
