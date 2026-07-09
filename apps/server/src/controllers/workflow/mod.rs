use loco_rs::controller::Routes;

pub fn routes() -> Routes {
    rustok_workflow::controllers::routes()
}

pub fn webhook_routes() -> Routes {
    rustok_workflow::controllers::webhook_routes()
}
