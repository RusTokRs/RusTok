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
    let mut pag = GridPagination::paged(95, 10);
    assert_eq!(pag.total_pages(), 10);
    assert_eq!(pag.current_page(), 1);
    assert_eq!(pag.effective_offset(), 0);
    assert_eq!(pag.effective_limit(), 10);

    // Navigating past total clamps to last page
    pag.goto_page(100);
    assert_eq!(pag.current_page(), 10);
    assert_eq!(pag.effective_offset(), 90);
    assert_eq!(pag.effective_limit(), 10);

    // Navigating below 1 clamps to 1
    pag.goto_page(0);
    assert_eq!(pag.current_page(), 1);

    // Empty datasets still maintain at least 1 page
    let empty_pag = GridPagination::paged(0, 10);
    assert_eq!(empty_pag.total_pages(), 1);
    assert_eq!(empty_pag.current_page(), 1);
}

#[test]
fn test_column_width_bounds_clamping_contract() {
    let width = ColumnWidth::new(80, 400).with_current(500);
    assert_eq!(width.current, 400);

    let stepped_down = step_resized_width(width, -350);
    assert_eq!(stepped_down.current, 80);

    let stepped_up = step_resized_width(width, 100);
    assert_eq!(stepped_up.current, 400);
}

#[test]
fn test_cell_value_total_ordering_contract() {
    let empty = CellValue::Empty;
    let b_false = CellValue::Boolean(false);
    let b_true = CellValue::Boolean(true);
    let num_1 = CellValue::Number(1.0);
    let num_10 = CellValue::Number(10.0);
    let txt_a = CellValue::Text("apple".to_string());
    let txt_b = CellValue::Text("banana".to_string());

    assert!(empty < b_false);
    assert!(b_false < b_true);
    assert!(b_true < num_1);
    assert!(num_1 < num_10);
    assert!(num_10 < txt_a);
    assert!(txt_a < txt_b);
}

#[test]
fn test_grid_state_apply_rows_pipeline_contract() {
    let cols = vec![
        GridColumnDef::new("id", "ID"),
        GridColumnDef::new("name", "Name"),
        GridColumnDef::new("score", "Score"),
    ];

    let mut state = GridState::new(cols);
    state.set_page_size(2);

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

    // Sort by score descending
    state.set_sort("score", SortDirection::Desc);
    let (rendered, total) = state.apply_rows(&dataset, |row, col| {
        row.get(col).cloned().unwrap_or(CellValue::Empty)
    });

    assert_eq!(total, 3);
    assert_eq!(rendered.len(), 2);
    // Highest score first (Bob: 95)
    assert_eq!(
        rendered[0].get("name"),
        Some(&CellValue::Text("Bob".to_string()))
    );
    // Second highest (Alice: 85)
    assert_eq!(
        rendered[1].get("name"),
        Some(&CellValue::Text("Alice".to_string()))
    );
}
