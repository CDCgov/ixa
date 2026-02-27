use std::collections::BTreeSet;
use std::path::PathBuf;

fn get_example_dirs() -> BTreeSet<String> {
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    std::fs::read_dir(&examples_dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", examples_dir.display()))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                Some(entry.file_name().to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect()
}

fn get_documented_examples() -> BTreeSet<String> {
    let doc_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/book/src/examples.md");
    let content = std::fs::read_to_string(&doc_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", doc_path.display()));

    // Parse ## `example-name` headers
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("### `")?;
            let name = rest.strip_suffix('`')?;
            Some(name.to_string())
        })
        .collect()
}

#[test]
fn examples_doc_lists_all_examples() {
    let on_disk = get_example_dirs();
    let in_doc = get_documented_examples();

    let missing_from_doc: Vec<_> = on_disk.difference(&in_doc).collect();
    let missing_from_disk: Vec<_> = in_doc.difference(&on_disk).collect();

    assert!(
        missing_from_doc.is_empty() && missing_from_disk.is_empty(),
        "Examples doc is out of sync.\n  \
         In examples/ but not in docs: {missing_from_doc:?}\n  \
         In docs but not in examples/: {missing_from_disk:?}"
    );
}
