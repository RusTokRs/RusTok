use ::leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core::route::ProductAdminRoute;

mod attribute_lifecycle;
pub mod attributes;
pub mod catalog_admin;
pub mod catalog_facets;
pub mod categories;
mod combinations;
mod initial_variants;
pub mod leptos;
pub mod product_editor;
pub mod product_grid;
mod variant_axes;
mod variant_create;
mod variant_mutation;

pub use attributes::AttributesPage;
pub use catalog_facets::AdminCatalogFacetPanel;
pub use categories::CategoriesPage;
pub use product_editor::ProductEditorPage;
pub use product_grid::ProductGridPage;

/// Mounted Product administration surface.
#[component]
pub fn ProductAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let view = match ProductAdminRoute::resolve(&route_context) {
        ProductAdminRoute::Catalog => view! { <ProductGridPage /> }.into_any(),
        ProductAdminRoute::Categories => view! { <CategoriesPage /> }.into_any(),
        ProductAdminRoute::Attributes => view! { <AttributesPage /> }.into_any(),
        ProductAdminRoute::NewProduct => view! { <ProductEditorPage is_new=true /> }.into_any(),
        ProductAdminRoute::EditProduct(id) => {
            view! { <ProductEditorPage is_new=false product_id=id /> }.into_any()
        }
    };

    view! {
        <div class="product-admin-container w-full min-h-[500px]">
            {view}
        </div>
    }
}
