use crate::{AdminCanvasController, BrowserIntentDispatchError, dispatch_browser_intent};
use fly::AssetCatalog;
use fly_browser::{BrowserIntentEnvelope, BrowserIntentKind, FLY_BROWSER_PROTOCOL};
use serde_json::{Value, json};

fn controller() -> AdminCanvasController {
    AdminCanvasController::new(
        "home",
        "rev-1",
        json!({
            "assets": [{
                "id": "hero",
                "src": "/old.webp",
                "name": "Old hero",
                "providerFuture": { "preserve": true }
            }],
            "pages": [{
                "id": "home",
                "flyPageMeta": { "slug": "home" },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "image", "type": "image" }]
                }
            }]
        }),
    )
    .expect("controller")
}

fn intent(
    controller: &AdminCanvasController,
    kind: BrowserIntentKind,
    payload: Value,
) -> BrowserIntentEnvelope {
    BrowserIntentEnvelope {
        protocol: FLY_BROWSER_PROTOCOL.to_string(),
        instance_id: "ssr-assets".to_string(),
        intent: kind.as_str().to_string(),
        payload,
        sequence: Some(1),
        page_id: Some(controller.page_id().to_string()),
        revision: Some(controller.revision_id().to_string()),
        project_hash: Some(controller.editor().revision().project_hash.hex()),
        draft_token: None,
        draft_generation: None,
    }
}

#[test]
fn browser_dispatches_asset_upsert_apply_and_safe_remove_contracts() {
    let mut controller = controller();

    let request = intent(
        &controller,
        BrowserIntentKind::UpsertAsset,
        json!({
            "asset_id": "hero",
            "source": "/new.webp",
            "name": "Hero"
        }),
    );
    let result = dispatch_browser_intent(&mut controller, request).expect("upsert asset");
    assert!(result.dirty);
    let asset = AssetCatalog::from_document(controller.editor().document())
        .get("hero")
        .cloned()
        .expect("hero asset");
    assert_eq!(asset.source, "/new.webp");
    assert_eq!(asset.name.as_deref(), Some("Hero"));
    assert_eq!(asset.raw["providerFuture"]["preserve"], true);

    let request = intent(
        &controller,
        BrowserIntentKind::SelectAsset,
        json!({
            "component_id": "image",
            "asset_id": "hero",
            "source_attribute": "src"
        }),
    );
    dispatch_browser_intent(&mut controller, request).expect("apply asset");
    let component = controller.editor().document().component("image").unwrap();
    assert_eq!(component.attributes["src"], "/new.webp");
    assert_eq!(component.attributes["data-fly-asset-id"], "hero");

    let request = intent(
        &controller,
        BrowserIntentKind::RemoveAsset,
        json!({ "asset_id": "hero" }),
    );
    let error = dispatch_browser_intent(&mut controller, request)
        .expect_err("referenced asset removal must fail");
    assert!(matches!(error, BrowserIntentDispatchError::Authoring(_)));
    assert!(
        error
            .to_string()
            .contains("still referenced by component(s): image")
    );

    let request = intent(&controller, BrowserIntentKind::Undo, json!({}));
    dispatch_browser_intent(&mut controller, request).expect("undo asset assignment");
    assert!(
        controller
            .editor()
            .document()
            .component("image")
            .unwrap()
            .attributes
            .get("data-fly-asset-id")
            .is_none()
    );

    let request = intent(
        &controller,
        BrowserIntentKind::RemoveAsset,
        json!({ "asset_id": "hero" }),
    );
    dispatch_browser_intent(&mut controller, request).expect("remove unreferenced asset");
    assert!(
        AssetCatalog::from_document(controller.editor().document())
            .get("hero")
            .is_none()
    );
}

#[test]
fn unsafe_asset_source_is_rejected_by_browser_dispatch() {
    let mut controller = controller();
    let request = intent(
        &controller,
        BrowserIntentKind::UpsertAsset,
        json!({
            "asset_id": "unsafe",
            "source": "javascript:alert(1)"
        }),
    );
    let error = dispatch_browser_intent(&mut controller, request).expect_err("unsafe source");
    assert!(matches!(error, BrowserIntentDispatchError::Authoring(_)));
    assert!(
        error
            .to_string()
            .contains("rejected by the default asset policy")
    );
}

#[test]
fn stale_asset_mutation_is_rejected_before_dispatch() {
    let mut controller = controller();
    let mut request = intent(
        &controller,
        BrowserIntentKind::RemoveAsset,
        json!({ "asset_id": "hero" }),
    );
    request.project_hash = Some("stale".to_string());
    let error = dispatch_browser_intent(&mut controller, request).expect_err("stale hash");
    assert!(matches!(
        error,
        BrowserIntentDispatchError::ProjectHashConflict { .. }
    ));
}
