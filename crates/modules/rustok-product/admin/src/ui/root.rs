use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use super::categories::CategoriesPage;
use super::product_editor::ProductEditorPage;
use super::product_grid::ProductGridPage;

fn resolve_product_edit_id(subpath: Option<&str>) -> Option<String> {
    let sub = subpath?.trim_matches('/');
    if sub.starts_with("edit/") {
        let id = sub.trim_start_matches("edit/").trim_matches('/');
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    // If subpath is a direct UUID
    if uuid::Uuid::parse_str(sub).is_ok() {
        return Some(sub.to_string());
    }
    None
}

#[component]
pub fn ProductAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let subpath = route_context.subpath();
    let query_id = route_context
        .query_param("id")
        .or_else(|| route_context.query_param("product_id"))
        .map(ToString::to_string);

    let edit_id = resolve_product_edit_id(subpath).or(query_id);

    let view = if route_context.subpath_matches("categories") {
        view! { <CategoriesPage /> }.into_any()
    } else if route_context.subpath_matches("new") {
        view! { <ProductEditorPage is_new=true /> }.into_any()
    } else if let Some(pid) = edit_id {
        view! { <ProductEditorPage is_new=false product_id=Some(pid) /> }.into_any()
    } else {
        view! { <ProductGridPage /> }.into_any()
    };

    view! {
        <div class="product-admin-container w-full min-h-[500px]">
            {view}
        </div>
    }
}
