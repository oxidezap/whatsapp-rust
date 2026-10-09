#[path = "../build/pin_clone.rs"]
mod pin_clone;

#[test]
fn clone_covers_all_fields_and_preserves_other_derives() {
    let output = pin_clone::generate(
        "#[derive(Clone, PartialEq, Default)] pub struct Message { pub first: String, pub future: Option<u32> } #[derive(Clone)] pub struct Other;",
    ).unwrap();
    assert!(output.contains("#[derive(PartialEq, Default)]"));
    assert!(output.contains("first: ::core::clone::Clone::clone(&self.first)"));
    assert!(output.contains("future: ::core::clone::Clone::clone(&self.future)"));
    assert!(output.contains("#[inline(never)]"));
    assert!(output.contains("#[derive(Clone)]\npub struct Other;"));
    assert!(
        pin_clone::generate(&output).is_err(),
        "must not silently double-apply"
    );
}

#[test]
fn generator_drift_fails_closed() {
    for source in [
        "pub struct Other;",
        "pub struct Message { pub value: u32 }",
        "#[derive(Clone)] pub struct Message<T> { pub value: T }",
        "#[derive(Clone)] pub struct Message(u32);",
        "#[derive(Clone)] pub struct Message {} #[derive(Clone)] pub struct Message {}",
    ] {
        assert!(pin_clone::generate(source).is_err(), "{source}");
    }
}
