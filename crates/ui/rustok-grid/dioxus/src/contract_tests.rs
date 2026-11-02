use super::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "rustok-grid-dioxus");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_filter_commit_mode_contract() {
    assert_eq!(FilterCommitMode::default(), FilterCommitMode::OnCommit);
    assert!(!FilterCommitMode::OnCommit.is_live());
    assert!(FilterCommitMode::Live.is_live());
}

#[test]
fn test_grid_cell_construction_contract() {
    let cell = GridCell {
        item: "item-123",
        column_id: "name".to_string(),
        row_id: "row-1".to_string(),
    };

    assert_eq!(cell.item, "item-123");
    assert_eq!(cell.column_id, "name");
    assert_eq!(cell.row_id, "row-1");
}

#[test]
fn test_reexported_types_contract() {
    let col = GridColumnDef::new("title", "Title");
    assert_eq!(col.id, "title");
    assert_eq!(col.title, "Title");

    let width = ColumnWidth::fixed(120);
    assert_eq!(width.current, 120);
    assert_eq!(width.min, 120);
    assert_eq!(width.max, 120);

    let state = GridState::new();
    assert_eq!(state.pagination.page, 1);
}

