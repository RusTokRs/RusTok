const SOURCE: &str = include_str!("../src/services/category.rs");

fn function_source(name: &str) -> &str {
    let marker = format!("pub async fn {name}(");
    let start = SOURCE
        .find(marker.as_str())
        .unwrap_or_else(|| panic!("missing category service function {name}"));
    let after_start = &SOURCE[start + marker.len()..];
    let end = after_start
        .find("\n    pub async fn ")
        .unwrap_or(after_start.len());
    &SOURCE[start..start + marker.len() + end]
}

#[test]
fn category_pagination_filters_archived_rows_in_sql_without_preloading_ids() {
    let list_source = function_source("list_paginated_with_locale_fallback");

    assert!(
        list_source.contains("not_in_subquery(archived_category_ids_subquery(tenant_id))"),
        "category pagination must exclude archived rows in the database query"
    );
    assert!(
        !list_source.contains("forum_category_lifecycle::Entity::find()"),
        "category pagination must not load all lifecycle rows into memory"
    );
    assert!(
        !list_source.contains(".all(&self.db)"),
        "category pagination must remain bounded before hydration"
    );

    let helper_start = SOURCE
        .find("fn archived_category_ids_subquery")
        .expect("missing lifecycle subquery helper");
    let helper = &SOURCE[helper_start
        ..SOURCE[helper_start..]
            .find("\nasync fn lock_category_tree_in_tx")
            .map(|offset| helper_start + offset)
            .expect("missing helper boundary")];
    assert!(helper.contains("forum_category_lifecycle::Column::CategoryId"));
    assert!(helper.contains("forum_category_lifecycle::Column::TenantId"));
    assert!(helper.contains(".eq(tenant_id)"));
}
