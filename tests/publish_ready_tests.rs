#[test]
fn cargo_toml_contains_recommended_crates_io_metadata() {
    let manifest = include_str!("../Cargo.toml");

    for field in [
        "description = ",
        "readme = ",
        "license = ",
        "documentation = ",
        "keywords = ",
        "categories = ",
    ] {
        assert!(
            manifest.contains(field),
            "missing recommended package metadata field: {field}"
        );
    }
}

#[test]
fn readme_mentions_library_and_cli_usage() {
    let readme = include_str!("../README.md");

    assert!(readme.contains("## CLI usage"));
    assert!(readme.contains("## Library usage"));
}
