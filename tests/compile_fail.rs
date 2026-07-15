#[test]
fn invalid_static_semantics_are_rejected() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
}
