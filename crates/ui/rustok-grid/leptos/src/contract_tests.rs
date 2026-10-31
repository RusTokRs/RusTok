use super::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "rustok-grid-leptos");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
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

    assert_eq!(CHECKBOX_COLUMN_ID, "__checkbox");
}

