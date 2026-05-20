use axum::routing::get;
use loco_rs::controller::Routes;

pub mod comments;
pub mod posts;

pub fn routes() -> Routes {
    rustok_blog::controllers::routes().add("/health", get(super::health::health))
}
