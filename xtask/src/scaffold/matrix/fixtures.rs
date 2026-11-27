pub(super) const CLI_ADAPTER: &str = include_str!("discovery_cli.rs.in");
pub(super) const CLI_CONTRACT: &str = include_str!("discovery_cli_contract.rs.in");

pub(super) const DISTRIBUTION_CONTRACT: &str = concat!(
    r#"#[test]
fn registers_each_selected_scaffold_once() {
    let registry = rustok_distribution::build_registry();
    for slug in [
        "scaffold_matrix_headless",
        "scaffold_matrix_admin",
        "scaffold_matrix_storefront",
        "scaffold_matrix_dual",
        "scaffold_matrix_localized",
    ] {
        assert_eq!(registry.modules().filter(|module| module.slug() == slug).count(), 1);
    }
}
"#,
    include_str!("index_fixture.rs.in"),
    include_str!("index_query_fixture.rs.in"),
    include_str!("index_graphql_fixture.rs.in")
);
