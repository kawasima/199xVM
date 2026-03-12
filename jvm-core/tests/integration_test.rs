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

// ---------------------------------------------------------------------------
// Arrays.copyOf(int[], int)
// ---------------------------------------------------------------------------

#[test]
fn arrays_copy_of_int() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "ArraysCopyOfTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "5:1,3,0");
}

// ---------------------------------------------------------------------------
// Stream.reduce(BinaryOperator) returning Optional
// ---------------------------------------------------------------------------

#[test]
fn stream_reduce_optional() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "StreamReduceTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "abc:false");
}

// ---------------------------------------------------------------------------
// Lambda SAM dispatch respects argument arity (overloaded interface methods)
// ---------------------------------------------------------------------------

#[test]
fn lambda_overload_arity() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "LambdaOverloadTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "HELLO!|HI?");
}

// ---------------------------------------------------------------------------
// ClassLoader API: getSystemClassLoader, Class.forName, loadClass
// ---------------------------------------------------------------------------

#[test]
fn classloader_api() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "ClassLoaderTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "cl:ok|forName:ok|loadClass:ok");
}

#[test]
fn classloader_missing_class_throws_cnfe() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "ClassLoaderNegativeTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ClassNotFoundException:com.example.NonExistentClass");
}

// ---------------------------------------------------------------------------
// JVMS §5.5: ExceptionInInitializerError when <clinit> throws
// ---------------------------------------------------------------------------

#[test]
fn clinit_exception_wrapped_in_eiie() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "ClinitExceptionTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ExceptionInInitializerError");
}

// ---------------------------------------------------------------------------
// JVMS §5.4.3.3: AbstractMethodError does not fire for concrete implementations
// ---------------------------------------------------------------------------

#[test]
fn concrete_interface_method_no_abstract_method_error() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "AbstractMethodTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "hello");
}

// ---------------------------------------------------------------------------
// JVMS §5.5: erroneous-state — second access throws NoClassDefFoundError
// ---------------------------------------------------------------------------

#[test]
fn clinit_erroneous_state_throws_ncdfe_on_second_access() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "ClinitErroneousStateTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "EIIE,NCDFE");
}
