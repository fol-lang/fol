use super::v3_example_inventory::assert_checked_in_example_directories;
use std::path::Path;

#[test]
fn checked_in_v3_examples_match_the_canonical_inventories() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    assert_checked_in_example_directories(&repo_root);
}
