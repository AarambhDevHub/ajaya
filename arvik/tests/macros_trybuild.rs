#![cfg(feature = "macros")]

#[test]
fn macros_compile_ui() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/macros/pass/*.rs");
    tests.compile_fail("tests/ui/macros/fail/*.rs");
}
