pub mod inventory;
pub mod products;
pub mod variants;

use axum::routing::{get, post, put};
use loco_rs::controller::Routes;

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/commerce")
        .add(
            "/products",
            get(products::list_products).post(products::create_product),
        )
        .add(
            "/products/{id}",
            get(products::show_product)
                .put(products::update_product)
                .delete(products::delete_product),
        )
        .add("/products/{id}/publish", post(products::publish_product))
        .add(
            "/products/{id}/unpublish",
            post(products::unpublish_product),
        )
        .add(
            "/products/{product_id}/variants",
            get(variants::list_variants).post(variants::create_variant),
        )
        .add(
            "/variants/{id}",
            get(variants::show_variant)
                .put(variants::update_variant)
                .delete(variants::delete_variant),
        )
        .add("/variants/{id}/prices", put(variants::update_prices))
        .add("/variants/{id}/inventory", get(inventory::get_inventory))
        .add(
            "/variants/{id}/inventory/adjust",
            post(inventory::adjust_inventory),
        )
        .add(
            "/variants/{id}/inventory/set",
            post(inventory::set_inventory),
        )
        .add("/inventory/check", post(inventory::check_availability))
}
