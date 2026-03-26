pub mod admin;
pub mod inventory;
pub mod products;
pub mod store;
pub mod variants;

use loco_rs::controller::Routes;

pub fn routes() -> Routes {
    rustok_commerce::controllers::routes()
}
