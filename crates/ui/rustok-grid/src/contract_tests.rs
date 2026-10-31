use std::cmp::Ordering;
use std::collections::BTreeMap;

use super::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "rustok-grid");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_pagination_invariants_and_bounds_contract() {
    let mut pag = GridPagination::new(1, 10, 95);
    assert_eq!(pag.total_pages(), 10);
    assert_eq!(pag.page, 1);
    assert_eq!(pag.from_index(), 1);
    assert_eq!(pag.to_index(), 10);

    // Navigating past total clamps to last page
    let effective = pag.go_to_page(100);
    assert_eq!(effective, 10);
    assert_eq!(pag.page, 10);
    assert_eq!(pag.from_index(), 91);
    assert_eq!(pag.to_index(), 95);

    // Navigating below 1 clamps to 1
    let back_first = pag.go_to_page(0);
    assert_eq!(back_first, 1);
    assert_eq!(pag.page, 1);

    // Empty datasets still maintain at least 1 page
    let empty_pag = GridPagination::new(1, 10, 0);
    assert_eq!(empty_pag.total_pages(), 1);
    assert_eq!(empty_pag.page, 1);
}

#[test]
fn test_column_width_bounds_clamping_contract() {
    let width = ColumnWidth::resizable(150, 80, 400);
    assert_eq!(width.current, 150);
    assert_eq!(width.min, 80);
    assert_eq!(width.max, 400);

    let stepped_down = step_resized_width(width.current, -100, width.min, width.max);
    assert_eq!(stepped_down, 80);

    let stepped_up = step_resized_width(width.current, 300, width.min, width.max);
    assert_eq!(stepped_up, 400);
}

#[test]
fn test_cell_value_total_ordering_contract() {
    let empty = CellValue::Empty;
    let b_false = CellValue::Bool(false);
    let b_true = CellValue::Bool(true);
    let num_1 = CellValue::Number(1.0);
    let num_10 = CellValue::Number(10.0);
    let txt_a = CellValue::Text("apple".to_string());
    let txt_b = CellValue::Text("banana".to_string());

    assert_eq!(empty.compare(&b_false), Ordering::Less);
    assert_eq!(b_false.compare(&b_true), Ordering::Less);
    assert_eq!(b_true.compare(&num_1), Ordering::Less);
    assert_eq!(num_1.compare(&num_10), Ordering::Less);
    assert_eq!(num_10.compare(&txt_a), Ordering::Less);
    assert_eq!(txt_a.compare(&txt_b), Ordering::Less);
    assert_eq!(txt_b.compare(&txt_b), Ordering::Equal);
}

#[test]
fn test_grid_state_apply_rows_pipeline_contract() {
    let mut state = GridState::new();
    state.pagination.set_page_size(2);
    state.sort = SortState::new("score", SortDirection::Desc);

    let row1 = BTreeMap::from([
        ("id".to_string(), CellValue::Number(1.0)),
        ("name".to_string(), CellValue::Text("Alice".to_string())),
        ("score".to_string(), CellValue::Number(85.0)),
    ]);
    let row2 = BTreeMap::from([
        ("id".to_string(), CellValue::Number(2.0)),
        ("name".to_string(), CellValue::Text("Bob".to_string())),
        ("score".to_string(), CellValue::Number(95.0)),
    ]);
    let row3 = BTreeMap::from([
        ("id".to_string(), CellValue::Number(3.0)),
        ("name".to_string(), CellValue::Text("Charlie".to_string())),
        ("score".to_string(), CellValue::Number(70.0)),
    ]);

    let dataset = vec![row1, row2, row3];

    let rendered = state.apply_rows(&dataset, |row, col| row.get(col).cloned().unwrap_or(CellValue::Empty));

    assert_eq!(rendered.len(), 2);
    // Highest score first (Bob: 95)
    assert_eq!(rendered[0].get("name"), Some(&CellValue::Text("Bob".to_string())));
    // Second highest (Alice: 85)
    assert_eq!(rendered[1].get("name"), Some(&CellValue::Text("Alice".to_string())));
}
