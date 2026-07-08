/// Loco route isolation layer.
///
/// Server controllers should import this alias instead of importing
/// `loco_rs::controller::*` directly. The final Axum router cutover should
/// replace this module's public contract together with `App::routes`.
pub use loco_rs::controller::{AppRoutes, Routes};

pub fn default_app_routes() -> AppRoutes {
    AppRoutes::with_default_routes()
}

pub fn mount_route(routes: AppRoutes, route: Routes) -> AppRoutes {
    routes.add_route(route)
}
