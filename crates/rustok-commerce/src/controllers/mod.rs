pub mod admin;
mod common;
pub mod products;
pub mod store;

use loco_rs::controller::Routes;

pub fn routes() -> Routes {
    Routes::new()
        .nest("/store", store::routes())
        .nest("/admin", admin::routes())
}
