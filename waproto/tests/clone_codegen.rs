#[path = "../build/pin_clone.rs"]
mod pin_clone;

fn generate(source: &str, targets: &[&str]) -> Result<String, String> {
    let mut file = syn::parse_file(source).map_err(|error| error.to_string())?;
    pin_clone::apply(&mut file, targets)?;
    Ok(prettyplease::unparse(&file))
}

#[test]
fn clone_covers_all_fields_and_preserves_other_derives() {
    let output = generate(
        "#[derive(Clone, PartialEq, Default)] pub struct Message { pub first: String, pub future: Option<u32> } #[derive(Clone)] pub struct Other;",
        &["Message"],
    ).unwrap();
    assert!(output.contains("#[derive(PartialEq, Default)]"));
    assert!(output.contains("first: ::core::clone::Clone::clone(&self.first)"));
    assert!(output.contains("future: ::core::clone::Clone::clone(&self.future)"));
    assert!(output.contains("#[inline(never)]"));
    assert!(output.contains("#[derive(Clone)]\npub struct Other;"));
    assert!(
        generate(&output, &["Message"]).is_err(),
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
        assert!(generate(source, &["Message"]).is_err(), "{source}");
    }
}

#[test]
fn selected_structs_have_separate_pinned_clone_impls() {
    let source = "#[derive(Clone)] struct Message { field: u8 } #[derive(Clone)] struct ContextInfo { other: u16 }";
    let output = generate(source, &["Message", "ContextInfo"]).unwrap();
    assert!(output.contains("impl ::core::clone::Clone for Message"));
    assert!(output.contains("impl ::core::clone::Clone for ContextInfo"));
    assert!(output.contains("other: ::core::clone::Clone::clone(&self.other)"));
    assert_eq!(output.matches("#[inline(never)]").count(), 2);
    assert!(generate(source, &["Message", "BotMetadata"]).is_err());
}
