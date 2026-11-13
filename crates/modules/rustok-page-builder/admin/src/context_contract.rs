use crate::{AdminCanvasController, PageBuilderAdminFacade, PageBuilderAdminHostContext};
use fly_ui::{ContributionAssemblyResult, EditorCapabilityEvaluation};
use serde_json::json;
use std::sync::Arc;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn host_context_and_shared_facade_are_send_sync() {
    assert_send_sync::<PageBuilderAdminHostContext>();
    assert_send_sync::<Arc<dyn PageBuilderAdminFacade>>();
    assert_send_sync::<Arc<ContributionAssemblyResult>>();
    assert_send_sync::<Arc<EditorCapabilityEvaluation>>();
}

#[test]
fn preview_endpoint_accepts_owned_and_borrowed_values_and_clears_blank_values() {
    let controller = AdminCanvasController::new(
        "home",
        "rev-1",
        json!({ "pages": [{ "id": "home", "component": { "id": "root", "type": "wrapper" } }] }),
    )
    .expect("controller");
    let context = PageBuilderAdminHostContext::new(controller);
    assert_eq!(context.preview_endpoint, None);
    let borrowed = context.clone().with_preview_endpoint("/preview");
    assert_eq!(borrowed.preview_endpoint.as_deref(), Some("/preview"));
    let owned = String::from("/preview?mode=editor");
    let configured = context.with_preview_endpoint(owned.clone());
    assert_eq!(configured.preview_endpoint.as_deref(), Some(owned.as_str()));
    for blank in ["", " \t\n"] {
        assert_eq!(
            configured
                .clone()
                .with_preview_endpoint(blank)
                .preview_endpoint,
            None
        );
    }
}
