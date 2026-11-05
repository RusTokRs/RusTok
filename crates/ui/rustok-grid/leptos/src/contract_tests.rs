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

#[test]
fn client_grid_renders_second_page_once_and_localizes_its_chrome() {
    use leptos::prelude::*;
    let owner = Owner::new();
    owner.with(|| {
        let rows = RwSignal::new((1..=25).collect::<Vec<u64>>());
        let pagination = RwSignal::new(GridPagination::new(2, 10, 25));
        let render = || view! {
            <DataGrid columns=vec![GridColumnDef::new("id", "Identifiant")]
                data=Signal::derive(move || rows.get()) key_fn=|id: &u64| *id
                cell_renderer=Callback::new(|(id, _): (u64, String)| view! { <span>{format!("row-{id}")}</span> }.into_any())
                pagination=pagination messages=rustok_grid::GridMessages::for_locale(Some("fr")) />
        }.to_html();
        let second = render();
        assert!(second.contains("row-11</span>"));
        assert!(second.contains("row-20</span>"));
        assert!(!second.contains("row-1</span>"));
        assert!(!second.contains("row-21</span>"));
        assert!(second.contains("Suivant"));
        assert!(!second.contains("Rows per page"));
        pagination.update(|page| page.set_page(3));
        let third = render();
        assert!(third.contains("row-21</span>"));
        assert!(!third.contains("row-11</span>"));
    });
}
