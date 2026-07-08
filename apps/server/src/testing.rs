//! Server-owned test fixtures used while runtime tests move off Loco helpers.

pub async fn get_server_app_context() -> loco_rs::app::AppContext {
    loco_rs::tests_cfg::app::get_app_context().await
}
