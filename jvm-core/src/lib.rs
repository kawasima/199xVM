//! 199xVM — a minimal JVM interpreter targeting WebAssembly.
//!
//! The public API exposed via `wasm-bindgen` lets the browser pass in
//! pre-compiled `.class` files (as `Uint8Array`) and invoke a static method,
//! receiving the string representation of the return value.

mod class_file;
mod heap;
mod interpreter;

use wasm_bindgen::prelude::*;

use class_file::{parse, parse_class_name};
use heap::JValue;
use interpreter::Vm;

const RESOURCE_MAGIC: &[u8; 4] = b"RSRC";

/// Load a bundle image and invoke a static method.
///
/// # Arguments
///
/// * `class_data` — concatenated bundle entries (each entry preceded by a
///   4-byte big-endian payload length).
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
    let out = match vm.invoke_static_threaded(main_class, method_name, descriptor, vec![]) {
        Ok(result) => {
            // If the result is a non-String object, call toString() on it.
            if let JValue::Ref(Some(ref r)) = result {
                let is_java_string = matches!(r.borrow().native, heap::NativePayload::JavaString(_));
                if !is_java_string {
                    let class_name = r.borrow().class_name.clone();
                    if let Ok(s) = vm.invoke_virtual(r.clone(), &class_name, "toString", "()Ljava/lang/String;", vec![]) {
                        jvalue_to_string(&s)
                    } else {
                        jvalue_to_string(&result)
                    }
                } else {
                    jvalue_to_string(&result)
                }
            } else {
                jvalue_to_string(&result)
            }
        }
        Err(e) => format!("ERROR: {e}"),
    };
    vm.flush_printstreams();
    out
}

/// Load classes and resources from bundle bytes into a VM using lazy parsing.
///
/// Class entries are raw `.class` bytes registered under their internal class
/// name. Resource entries use a small `RSRC` envelope carrying path, payload,
/// and last-modified metadata. Classes are not parsed until first access,
/// matching standard ClassLoader lazy-loading semantics, and `*.class`
/// resources are synthesized from the loaded class table.
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
        if let Some((name, last_modified, bytes)) = parse_resource_entry(class_bytes) {
            vm.load_resource(name, bytes, last_modified);
            continue;
        }
        match parse_class_name(class_bytes) {
            Some(name) => {
                vm.load_lazy(name, class_bytes.to_vec());
            }
            None => eprintln!("Warning: skipping bundle entry with unreadable name"),
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

fn parse_resource_entry(payload: &[u8]) -> Option<(String, i64, Vec<u8>)> {
    if payload.len() < 20 || &payload[..4] != RESOURCE_MAGIC {
        return None;
    }
    let path_len = u32::from_be_bytes(payload[4..8].try_into().ok()?) as usize;
    let last_modified = i64::from_be_bytes(payload[8..16].try_into().ok()?);
    let data_len = u32::from_be_bytes(payload[16..20].try_into().ok()?) as usize;
    let expected_len = 20usize.checked_add(path_len)?.checked_add(data_len)?;
    if payload.len() != expected_len {
        return None;
    }
    let path_bytes = &payload[20..20 + path_len];
    let data_bytes = payload[20 + path_len..].to_vec();
    let path = std::str::from_utf8(path_bytes).ok()?.to_owned();
    Some((path, last_modified, data_bytes))
}
