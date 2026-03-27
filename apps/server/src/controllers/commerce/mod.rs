pub mod admin;
pub mod products;
pub mod store;

use loco_rs::controller::Routes;

pub fn routes() -> Routes {
    rustok_commerce::controllers::routes()
}
