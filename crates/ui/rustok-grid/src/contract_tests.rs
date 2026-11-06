use std::{cmp::Ordering, collections::BTreeMap};

use super::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "rustok-grid");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_pagination_invariants_and_bounds_contract() {
    let mut pagination = GridPagination::new(1, 10, 95);
    assert_eq!(pagination.total_pages(), 10);
    assert_eq!(pagination.page, 1);
    assert_eq!(pagination.offset(), 0);
    assert_eq!(pagination.page_size, 10);

    // Navigation reports the effective page; the partial final display window
    // does not change the page size an owner uses for its next fetch.
    assert_eq!(pagination.go_to_page(100), 10);
    assert_eq!(pagination.offset(), 90);
    assert_eq!(pagination.page_size, 10);
    assert_eq!(pagination.visible_len(), 5);
    assert_eq!((pagination.from_index(), pagination.to_index()), (91, 95));
    assert!(!pagination.has_next);
    assert!(pagination.has_previous());

    assert_eq!(pagination.go_to_page(0), 1);
    assert_eq!(pagination.offset(), 0);
    assert_eq!(pagination.go_to_page(3), 3);
    pagination.set_mode(PaginationMode::Infinite);
    assert_eq!(pagination.offset(), 20, "fetches still request page 3");
    assert_eq!(
        pagination.visible_len(),
        30,
        "infinite rendering retains the prefix"
    );

    let empty = GridPagination::new(0, 10, 0);
    assert_eq!((empty.total_pages(), empty.page), (1, 1));
    assert_eq!(
        (empty.from_index(), empty.to_index(), empty.visible_len()),
        (0, 0, 0)
    );
    assert!(!empty.has_next);
    let zero_size = GridPagination::new(0, 0, 5);
    assert_eq!((zero_size.page, zero_size.page_size), (1, 1));
}

#[test]
fn test_column_width_bounds_clamping_contract() {
    let width = ColumnWidth::resizable(500, 80, 400);
    assert_eq!(width.current, 400);
    assert_eq!(
        step_resized_width(width.current, -350, width.min, width.max),
        80,
    );
    assert_eq!(
        step_resized_width(width.current, 100, width.min, width.max),
        400,
    );
    assert_eq!(
        step_resized_width(width.current, i32::MIN, width.min, width.max),
        80,
    );
    assert_eq!(
        step_resized_width(width.current, i32::MAX, width.min, width.max),
        400,
    );
    assert_eq!(ColumnWidth::resizable(500, 400, 80), width);
}

#[test]
fn test_cell_value_total_ordering_contract() {
    let ordered = [
        CellValue::Empty,
        CellValue::boolean(false),
        CellValue::boolean(true),
        CellValue::number(1.0),
        CellValue::number(10.0),
        CellValue::text("apple"),
        CellValue::text("banana"),
    ];
    for pair in ordered.windows(2) {
        assert_eq!(pair[0].compare(&pair[1]), Ordering::Less);
        assert_eq!(pair[1].compare(&pair[0]), Ordering::Greater);
    }
    for value in &ordered {
        assert_eq!(value.compare(value), Ordering::Equal);
    }
    // The public constructor rejects non-finite numbers. A malformed raw
    // variant still sorts with missing values instead of poisoning the order.
    assert_eq!(CellValue::number(f64::INFINITY), CellValue::Empty);
    assert_eq!(
        CellValue::Number(f64::NAN).compare(&CellValue::Empty),
        Ordering::Equal
    );
}

#[test]
fn test_grid_state_apply_rows_pipeline_contract() {
    fn row(id: f64, name: &str, score: f64, visible: bool) -> BTreeMap<String, CellValue> {
        BTreeMap::from([
            ("id".into(), CellValue::number(id)),
            ("name".into(), CellValue::text(name)),
            ("score".into(), CellValue::number(score)),
            ("visible".into(), CellValue::boolean(visible)),
        ])
    }
    let dataset = vec![
        row(1.0, "Alice", 85.0, true),
        row(2.0, "Bob", 95.0, true),
        row(3.0, "Charlie", 70.0, false),
        row(4.0, "Dana", 95.0, true),
        row(5.0, "Eve", 100.0, false),
    ];
    let cell_of = |row: &BTreeMap<String, CellValue>, column: &str| {
        row.get(column).cloned().unwrap_or(CellValue::Empty)
    };
    let names = |rows: &[BTreeMap<String, CellValue>]| {
        rows.iter()
            .map(|row| row["name"].display_text())
            .collect::<Vec<_>>()
    };
    let mut state = GridState::new().with_pagination(1, 2, dataset.len() as u64);
    state.sort.set("score", SortDirection::Desc);
    assert_eq!(names(&state.apply_rows(&dataset, cell_of)), ["Eve", "Bob"]);

    state.pagination.set_page(2);
    assert!(state.set_filter("visible", FilterValue::Boolean(true)));
    assert_eq!(
        state.pagination.page, 1,
        "changed filters invalidate page state"
    );
    // apply_rows returns rendered rows, not a second total source. The owner
    // obtains its filtered count through the same canonical filter vocabulary.
    let filtered = state.filters.retain_matching(&dataset, cell_of);
    assert_eq!(filtered.len(), 3);
    state.pagination.set_total(filtered.len() as u64);
    assert_eq!(names(&state.apply_rows(&dataset, cell_of)), ["Bob", "Dana"]);

    state.pagination.set_page(2);
    assert_eq!(names(&state.apply_rows(&dataset, cell_of)), ["Alice"]);
    assert_eq!(state.pagination.total, 3);
    assert_eq!(state.toggle_sort("score"), None);
    assert_eq!(state.pagination.page, 1);
    assert_eq!(
        names(&state.apply_rows(&dataset, cell_of)),
        ["Alice", "Bob"]
    );
}
