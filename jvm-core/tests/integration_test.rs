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
// try/catch: NullPointerException caught in handler
// ---------------------------------------------------------------------------

#[test]
fn try_catch_npe() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "TryCatchTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "CAUGHT");
}

// ---------------------------------------------------------------------------
// Factorial with long arithmetic
// ---------------------------------------------------------------------------

fn factorial_bundle() -> &'static [u8] {
    include_bytes!("../../test-classes/factorial-bundle.bin")
}

#[test]
fn factorial_long() {
    let bundle = combined_bundle(shim_bundle(), factorial_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "Factorial",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "10!=3628800 15!=1307674368000");
}
