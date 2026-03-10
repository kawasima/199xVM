//! 199xVM — a minimal JVM interpreter targeting WebAssembly.
//!
//! The public API exposed via `wasm-bindgen` lets the browser pass in
//! pre-compiled `.class` files (as `Uint8Array`) and invoke a static method,
//! receiving the string representation of the return value.

mod class_file;
mod heap;
mod interpreter;

use wasm_bindgen::prelude::*;

use class_file::parse;
use heap::JValue;
use interpreter::Vm;

/// Load one or more `.class` files and invoke a static method.
///
/// # Arguments
///
/// * `class_data` — concatenated raw `.class` bytes (each class preceded by a
///   4-byte big-endian length).
/// * `main_class` — internal class name of the entry point (e.g.
///   `"net/unit8/raoh/Decoders"`).
/// * `method_name` — static method to invoke (e.g. `"run"`).
/// * `descriptor` — JVM method descriptor (e.g. `"()Ljava/lang/String;"`).
///
/// Returns the `toString()` of the result, or an error message prefixed with
/// `"ERROR: "`.
#[wasm_bindgen]
pub fn run_static(
    class_bundle: &[u8],
    main_class: &str,
    method_name: &str,
    descriptor: &str,
) -> String {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    run_static_native(class_bundle, main_class, method_name, descriptor)
}

/// Parse a single `.class` file and return the class name if successful.
/// Useful for validating that a class loaded correctly.
#[wasm_bindgen]
pub fn parse_class(class_bytes: &[u8]) -> String {
    match parse(class_bytes) {
        Ok(cf) => {
            let name = cf.constant_pool.class_name(cf.this_class).to_owned();
            format!("OK: {name} (v{}.{})", cf.major_version, cf.minor_version)
        }
        Err(e) => format!("ERROR: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Public (non-wasm) API for testing
// ---------------------------------------------------------------------------

/// Load classes from a bundle and invoke a static method.
/// Same logic as `run_static` but without `wasm_bindgen`.
pub fn run_static_native(
    class_bundle: &[u8],
    main_class: &str,
    method_name: &str,
    descriptor: &str,
) -> String {
    let mut vm = Vm::new();
    load_bundle(&mut vm, class_bundle);
    match vm.invoke_static(main_class, method_name, descriptor, vec![]) {
        Ok(result) => {
            // If the result is a non-String object, call toString() on it.
            if let JValue::Ref(Some(ref r)) = result {
                let is_java_string = matches!(r.borrow().native, heap::NativePayload::JavaString(_));
                if !is_java_string {
                    let class_name = r.borrow().class_name.clone();
                    match vm.invoke_virtual(r.clone(), &class_name, "toString", "()Ljava/lang/String;", vec![]) {
                        Ok(s) => return jvalue_to_string(&s),
                        Err(_) => {}
                    }
                }
            }
            jvalue_to_string(&result)
        }
        Err(e) => format!("ERROR: {e}"),
    }
}

/// Load classes from bundle bytes into a VM.
pub fn load_bundle(vm: &mut Vm, class_bundle: &[u8]) {
    let mut pos = 0usize;
    while pos + 4 <= class_bundle.len() {
        let len = u32::from_be_bytes([
            class_bundle[pos],
            class_bundle[pos + 1],
            class_bundle[pos + 2],
            class_bundle[pos + 3],
        ]) as usize;
        pos += 4;
        if pos + len > class_bundle.len() { break; }
        let class_bytes = &class_bundle[pos..pos + len];
        pos += len;
        match parse(class_bytes) {
            Ok(cf) => vm.load_class(cf),
            Err(e) => eprintln!("Warning: skipping unparseable class: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn jvalue_to_string(v: &JValue) -> String {
    match v {
        JValue::Void => "void".to_owned(),
        JValue::Int(i) => i.to_string(),
        JValue::Long(l) => l.to_string(),
        JValue::Float(f) => f.to_string(),
        JValue::Double(d) => d.to_string(),
        JValue::Ref(None) => "null".to_owned(),
        JValue::Ref(Some(r)) => {
            let obj = r.borrow();
            match &obj.native {
                heap::NativePayload::JavaString(s) => s.clone(),
                heap::NativePayload::Array(v) => format!("[{}]", v.iter().map(jvalue_to_string).collect::<Vec<_>>().join(", ")),
                _ => format!("{}@obj", obj.class_name),
            }
        }
        JValue::ReturnAddress(a) => format!("ret:{a}"),
    }
}
