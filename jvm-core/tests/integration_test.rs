//! Integration tests for the 199xVM.
//!
//! These tests load pre-compiled `.class` bundles and verify bytecode execution
//! without requiring a browser or WASM runtime.
//!
//! Prerequisites:
//!   ./build-shim.sh          (builds jdk-shim/bundle.bin)
//!   ./build-test-bundle.sh   (builds test-classes/bundle.bin)

/// Combine shim + app bundles.
fn combined_bundle(shim: &[u8], app: &[u8]) -> Vec<u8> {
    let mut combined = Vec::with_capacity(shim.len() + app.len());
    combined.extend_from_slice(shim);
    combined.extend_from_slice(app);
    combined
}

fn shim_bundle() -> &'static [u8] {
    include_bytes!("../../jdk-shim/bundle.bin")
}

fn test_bundle() -> &'static [u8] {
    include_bytes!("../../test-classes/bundle.bin")
}

fn raoh_bundle() -> &'static [u8] {
    include_bytes!("../../raoh-classes/bundle.bin")
}

fn hello_bundle() -> Vec<u8> {
    let hello = include_bytes!("../../hello-classes/Hello.class");
    let mut bundle: Vec<u8> = Vec::new();
    bundle.extend_from_slice(&(hello.len() as u32).to_be_bytes());
    bundle.extend_from_slice(hello);
    bundle
}

// ---------------------------------------------------------------------------
// Hello World
// ---------------------------------------------------------------------------

#[test]
fn hello_world() {
    let bundle = combined_bundle(shim_bundle(), &hello_bundle());
    let result = jvm_core::run_static_native(&bundle, "Hello", "greet", "()Ljava/lang/String;");
    assert_eq!(result, "Hello, World!");
}

// ---------------------------------------------------------------------------
// Integer.toString via shim bytecode
// ---------------------------------------------------------------------------

#[test]
fn integer_tostring() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "IntToStringTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "42");
}

// ---------------------------------------------------------------------------
// String concatenation: "OK: " + Integer.valueOf(42)
// ---------------------------------------------------------------------------

#[test]
fn string_concat() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "StringConcatTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "OK: 42");
}

// ---------------------------------------------------------------------------
// ArrayList basic operations
// ---------------------------------------------------------------------------

#[test]
fn arraylist_basics() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "ListTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "2: hello world");
}

// ---------------------------------------------------------------------------
// Raoh Runner.run() – validation pipeline
// ---------------------------------------------------------------------------

#[test]
fn raoh_runner() {
    let bundle = combined_bundle(shim_bundle(), raoh_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "net/unit8/raoh/playground/Runner",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "OK: 42");
}

// ---------------------------------------------------------------------------
// Wildcard-import static call: ObjectDecoders.string().decode("abc")
// The compiler emits invokestatic with return type Ljava/lang/Object; but the
// real method returns Lnet/unit8/raoh/StringDecoder; — the VM must resolve it.
// ---------------------------------------------------------------------------

#[test]
fn wildcard_import_static_call() {
    // Build a minimal class bundle that calls ObjectDecoders.string().decode("abc").
    // We compile this inline using the javac.ts output via a pre-built fixture.
    // The fixture class is checked in at test-classes/WildcardTest.class.
    let wildcard_test = include_bytes!("../../test-classes/WildcardTest.class");
    let mut user_bundle: Vec<u8> = Vec::new();
    user_bundle.extend_from_slice(&(wildcard_test.len() as u32).to_be_bytes());
    user_bundle.extend_from_slice(wildcard_test);

    let mut combined = shim_bundle().to_vec();
    combined.extend_from_slice(raoh_bundle());
    combined.extend_from_slice(&user_bundle);

    let result = jvm_core::run_static_native(
        &combined,
        "WildcardTest",
        "run",
        "()Ljava/lang/String;",
    );
    // string().decode("abc") — "abc" is a valid non-blank string, so Ok("abc")
    assert_eq!(result, "OK: abc");
}

// ---------------------------------------------------------------------------
// Err.toString() — validation failure returns formatted error string
// ---------------------------------------------------------------------------

#[test]
fn err_tostring() {
    let err_test = include_bytes!("../../test-classes/ErrToStringTest.class");
    let mut user_bundle: Vec<u8> = Vec::new();
    user_bundle.extend_from_slice(&(err_test.len() as u32).to_be_bytes());
    user_bundle.extend_from_slice(err_test);

    let mut combined = shim_bundle().to_vec();
    combined.extend_from_slice(raoh_bundle());
    combined.extend_from_slice(&user_bundle);

    let result = jvm_core::run_static_native(
        &combined,
        "ErrToStringTest",
        "run",
        "()Ljava/lang/String;",
    );
    // string().maxLength(1).decode("abc") fails — Err.toString() returns "Err[/: <message>]"
    assert_eq!(result, "Err[/: must be at most 1 characters]");
}

// ---------------------------------------------------------------------------
// decode(value) — single-arg default method on Decoder interface
// Compiled by javac.ts: invokevirtual java/lang/Object.decode(String)Object
// The VM must resolve to Decoder.decode(I) default method via interface walk.
// ---------------------------------------------------------------------------

#[test]
fn decode_single_arg() {
    // DecodeSingleArgTest.class generated by javac.ts from:
    //   string().maxLength(2).decode("123")  → Err result, toString() called by lib.rs
    let class_bytes = include_bytes!("../../test-classes/DecodeSingleArgTest.class");
    let mut user_bundle: Vec<u8> = Vec::new();
    user_bundle.extend_from_slice(&(class_bytes.len() as u32).to_be_bytes());
    user_bundle.extend_from_slice(class_bytes);

    let mut combined = shim_bundle().to_vec();
    combined.extend_from_slice(raoh_bundle());
    combined.extend_from_slice(&user_bundle);

    let result = jvm_core::run_static_native(
        &combined,
        "Hello",
        "run",
        "()Ljava/lang/String;",
    );
    eprintln!("decode_single_arg result: {result}");
    // "123" has length 3 which exceeds maxLength(2), so should be Err[...]
    assert!(result.starts_with("Err["), "unexpected result: {result}");
}
