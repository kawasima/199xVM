//! 199xVM — a minimal JVM interpreter targeting WebAssembly.
//!
//! The public API exposed via `wasm-bindgen` lets the browser pass in
//! pre-compiled `.class` files (as `Uint8Array`) and invoke a static method,
//! receiving the string representation of the return value.

mod class_file;
pub mod heap;
pub mod interpreter;

#[cfg(target_arch = "wasm32")]
use js_sys::Array;
use wasm_bindgen::prelude::*;

use class_file::{parse, parse_class_name};
use heap::JValue;
pub use interpreter::launcher::{JvmProcess, ProcessExit, ProcessState};
pub use interpreter::StdioMode;
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

/// Load a shim bundle + application JARs and invoke a static method.
///
/// * `shim_bundle` — JDK shim classes in flat bundle format.
/// * `jar_data` — concatenated JAR files, each preceded by a 4-byte big-endian
///   length (`[u32 len][jar bytes] × N`).  Pass an empty slice if no JARs.
#[wasm_bindgen]
pub fn run_with_jars(
    shim_bundle: &[u8],
    jar_data: &[u8],
    main_class: &str,
    method_name: &str,
    descriptor: &str,
) -> String {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    run_with_jars_native(shim_bundle, jar_data, main_class, method_name, descriptor)
}

/// Convert a single JAR to the flat bundle format (for use with `run_static`).
///
/// Returns the bundle bytes, or an empty array if the JAR is invalid.
#[wasm_bindgen]
pub fn jar_to_bundle(jar_bytes: &[u8]) -> Vec<u8> {
    jar_to_bundle_native(jar_bytes)
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
    invoke_and_collect(&mut vm, main_class, method_name, descriptor)
}

/// Load a shim bundle + application JARs and invoke a static method.
pub fn run_with_jars_native(
    shim_bundle: &[u8],
    jar_data: &[u8],
    main_class: &str,
    method_name: &str,
    descriptor: &str,
) -> String {
    let mut vm = Vm::new();
    load_bundle(&mut vm, shim_bundle);
    if let Err(e) = load_jars(&mut vm, jar_data) {
        return format!("ERROR: {e}");
    }
    invoke_and_collect(&mut vm, main_class, method_name, descriptor)
}

pub fn launch_classpath_main_native(
    shim_bundle: &[u8],
    jar_data: &[u8],
    main_class: &str,
    args: &[String],
    stdin: StdioMode,
    stdout: StdioMode,
    stderr: StdioMode,
) -> Result<JvmProcess, String> {
    interpreter::launcher::JvmProcess::launch_classpath_main(
        shim_bundle,
        jar_data,
        &normalize_main_class(main_class),
        args,
        stdin,
        stdout,
        stderr,
    )
}

/// Convert a single JAR to flat bundle format.
pub fn jar_to_bundle_native(jar_bytes: &[u8]) -> Vec<u8> {
    use std::io::Cursor;
    let reader = Cursor::new(jar_bytes);
    let mut archive = match zip::ZipArchive::new(reader) {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(f) => f,
            Err(_) => continue,
        };
        if !file.name().ends_with(".class") {
            continue;
        }
        let mut buf = Vec::with_capacity(file.size() as usize);
        if std::io::Read::read_to_end(&mut file, &mut buf).is_err() {
            continue;
        }
        let len = buf.len() as u32;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&buf);
    }
    out
}

/// Load JARs from a framed byte array into a VM.
///
/// Each JAR is preceded by a 4-byte big-endian length: `[u32 len][jar bytes] × N`.
pub fn load_jars(vm: &mut Vm, jar_data: &[u8]) -> Result<(), String> {
    let mut pos = 0usize;
    let mut jar_index = 0usize;
    while pos < jar_data.len() {
        if pos + 4 > jar_data.len() {
            return Err(format!("Truncated JAR frame header at byte offset {pos}"));
        }
        let len = u32::from_be_bytes([
            jar_data[pos],
            jar_data[pos + 1],
            jar_data[pos + 2],
            jar_data[pos + 3],
        ]) as usize;
        pos += 4;
        if pos + len > jar_data.len() {
            return Err(format!(
                "Truncated JAR frame payload for entry #{jar_index} at byte offset {}",
                pos - 4
            ));
        }
        let jar_bytes = &jar_data[pos..pos + len];
        pos += len;
        vm.load_jar(jar_bytes)
            .map_err(|e| format!("Failed to load classpath JAR #{jar_index}: {e}"))?;
        jar_index += 1;
    }
    Ok(())
}

fn normalize_main_class(main_class: &str) -> String {
    if main_class.contains('/') {
        main_class.to_owned()
    } else {
        main_class.replace('.', "/")
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct JvmProcessHandle {
    inner: JvmProcess,
}

#[cfg(target_arch = "wasm32")]
fn parse_low_level_stdio(spec: &str, name: &str) -> Result<StdioMode, JsValue> {
    match spec {
        "pipe" => Ok(StdioMode::Pipe),
        "ignore" => Ok(StdioMode::Ignore),
        "inherit" => Err(JsValue::from_str(&format!(
            "launchClasspathMainLowLevel does not accept stdio.{name}=\"inherit\"; normalize it in the caller"
        ))),
        _ => Err(JsValue::from_str(&format!(
            "launchClasspathMainLowLevel stdio.{name} must be \"pipe\" or \"ignore\""
        ))),
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = launchClasspathMainLowLevel)]
pub fn launch_classpath_main_low_level(
    shim_bundle: &[u8],
    jar_data: &[u8],
    main_class: &str,
    args: Array,
    stdin: &str,
    stdout: &str,
    stderr: &str,
) -> Result<JvmProcessHandle, JsValue> {
    // This low-level export operates on concrete VM stdio modes only.
    // Higher-level launcher policy such as public `"inherit"` handling lives in web/launcher.js.
    let args = args
        .iter()
        .map(|value| {
            value.as_string().ok_or_else(|| {
                JsValue::from_str("launchClasspathMainLowLevel args must be strings")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let stdin = parse_low_level_stdio(stdin, "stdin")?;
    let stdout = parse_low_level_stdio(stdout, "stdout")?;
    let stderr = parse_low_level_stdio(stderr, "stderr")?;
    let inner = launch_classpath_main_native(
        shim_bundle,
        jar_data,
        main_class,
        &args,
        stdin,
        stdout,
        stderr,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    Ok(JvmProcessHandle { inner })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl JvmProcessHandle {
    pub fn pump(&mut self, max_rounds: usize) -> String {
        match self.inner.pump(max_rounds) {
            ProcessState::Running => "running".to_owned(),
            ProcessState::WaitingForInput => "waiting".to_owned(),
            ProcessState::Exited => "exited".to_owned(),
        }
    }

    pub fn write_stdin(&mut self, bytes: &[u8]) {
        self.inner.write_stdin(bytes);
    }

    pub fn close_stdin(&mut self) {
        self.inner.close_stdin();
    }

    pub fn take_stdout(&mut self) -> Vec<u8> {
        self.inner.take_stdout()
    }

    pub fn take_stderr(&mut self) -> Vec<u8> {
        self.inner.take_stderr()
    }

    pub fn is_exited(&self) -> bool {
        self.inner.is_exited()
    }

    pub fn exit_code(&self) -> i32 {
        self.inner.exit().map(|exit| exit.exit_code).unwrap_or(-1)
    }

    pub fn uncaught_exception(&self) -> Option<String> {
        self.inner
            .exit()
            .and_then(|exit| exit.uncaught_exception.clone())
    }

    pub fn kill(&mut self) {
        self.inner.kill();
    }
}

fn invoke_and_collect(
    vm: &mut Vm,
    main_class: &str,
    method_name: &str,
    descriptor: &str,
) -> String {
    let out = match vm.invoke_static_threaded(main_class, method_name, descriptor, vec![]) {
        Ok(result) => {
            // If the result is a non-String object, call toString() on it.
            if let JValue::Ref(Some(ref r)) = result {
                let is_java_string =
                    matches!(r.borrow().native, heap::NativePayload::JavaString(_));
                if !is_java_string {
                    let class_name = r.borrow().class_name.clone();
                    if let Ok(s) = vm.invoke_virtual(
                        r.clone(),
                        &class_name,
                        "toString",
                        "()Ljava/lang/String;",
                        vec![],
                    ) {
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

/// Load classes from bundle bytes into a VM using lazy parsing.
///
/// Each class's raw bytes are registered under its internal class name.
/// The class is not parsed until it is first accessed during execution,
/// matching standard ClassLoader lazy-loading semantics.
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
        if pos + len > class_bundle.len() {
            break;
        }
        let class_bytes = &class_bundle[pos..pos + len];
        pos += len;
        match parse_class_name(class_bytes) {
            Some(name) => vm.load_lazy(name, class_bytes.to_vec()),
            None => match parse(class_bytes) {
                Ok(class_file) => vm.load_class(class_file),
                Err(err) => eprintln!("Warning: skipping class with unreadable name: {err}"),
            },
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
                heap::NativePayload::Array(v) => format!(
                    "[{}]",
                    v.iter()
                        .map(jvalue_to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                _ => format!("{}@obj", obj.class_name),
            }
        }
        JValue::ReturnAddress(a) => format!("ret:{a}"),
    }
}
