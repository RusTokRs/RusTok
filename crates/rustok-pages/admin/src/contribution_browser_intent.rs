use crate::browser_intent::{self, PagesBrowserIntentError, PagesBrowserIntentResponse};
use crate::builder::PagesBuilderSaveSnapshot;
use crate::contributions::{
    build_pages_admin_contribution_registry, pages_admin_contribution_policy,
};
use fly_browser::BrowserIntentEnvelope;
use fly_ui::{CapabilityState, PaletteBlockAccess};
use rustok_page_builder_admin::{
    validate_browser_capability_access, validate_browser_palette_access,
    BrowserIntentDispatchError, SsrDraftSessionStore,
};

pub fn pages_palette_block_access() -> PaletteBlockAccess {
    let assembly = build_pages_admin_contribution_registry(&pages_admin_contribution_policy());
    PaletteBlockAccess::from_assembly(&assembly)
}

pub async fn dispatch_pages_browser_intent(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentError> {
    dispatch_pages_browser_intent_with_capabilities(
        snapshot,
        envelope,
        CapabilityState::full(),
    )
    .await
}

pub async fn dispatch_pages_browser_intent_with_capabilities(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
    capabilities: CapabilityState,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentError> {
    let envelope = preflight_pages_intent(envelope, capabilities)?;
    browser_intent::dispatch_pages_browser_intent(snapshot, envelope).await
}

pub async fn dispatch_pages_browser_intent_with_store(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
    draft_store: &dyn SsrDraftSessionStore,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentError> {
    dispatch_pages_browser_intent_with_store_and_capabilities(
        snapshot,
        envelope,
        draft_store,
        CapabilityState::full(),
    )
    .await
}

pub async fn dispatch_pages_browser_intent_with_store_and_capabilities(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
    draft_store: &dyn SsrDraftSessionStore,
    capabilities: CapabilityState,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentError> {
    let envelope = preflight_pages_intent(envelope, capabilities)?;
    browser_intent::dispatch_pages_browser_intent_with_store(snapshot, envelope, draft_store).await
}

fn preflight_pages_intent(
    envelope: BrowserIntentEnvelope,
    capabilities: CapabilityState,
) -> Result<BrowserIntentEnvelope, PagesBrowserIntentError> {
    let envelope = envelope
        .normalized()
        .map_err(BrowserIntentDispatchError::from)?;
    validate_browser_palette_access(&envelope, &pages_palette_block_access())?;
    validate_browser_capability_access(&envelope, capabilities)?;
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_browser::FLY_BROWSER_PROTOCOL_V1;
    use fly_ui::EditorCapability;
    use rustok_page_builder_admin::browser_capability_denial;
    use serde_json::{json, Value};

    fn envelope(intent: &str, payload: Value) -> BrowserIntentEnvelope {
        BrowserIntentEnvelope {
            protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
            instance_id: "pages-test".to_string(),
            intent: intent.to_string(),
            payload,
            sequence: Some(1),
            page_id: Some("home".to_string()),
            revision: Some("rev-1".to_string()),
            project_hash: None,
            draft_token: None,
            draft_generation: None,
        }
    }

    #[test]
    fn pages_preflight_allows_primitives_and_declared_templates() {
        assert!(preflight_pages_intent(
            envelope("insert_block", json!({ "block_id": "text" })),
            CapabilityState::full(),
        )
        .is_ok());
        assert!(preflight_pages_intent(
            envelope("insert_block", json!({ "block_id": "fly.hero" })),
            CapabilityState::full(),
        )
        .is_ok());
    }

    #[test]
    fn pages_preflight_rejects_uncontributed_namespaced_templates() {
        let error = preflight_pages_intent(
            envelope("insert_block", json!({ "block_id": "plugin.secret" })),
            CapabilityState::full(),
        )
        .expect_err("uncontributed block");
        assert!(matches!(
            error,
            PagesBrowserIntentError::Dispatch(BrowserIntentDispatchError::Authoring(_))
        ));
    }

    #[test]
    fn pages_preflight_rejects_block_drop_bypass() {
        assert!(preflight_pages_intent(
            envelope(
                "drop",
                json!({
                    "source": { "kind": "block", "block_id": "fly.pricing" },
                    "target_component_id": "root",
                    "position": "inside"
                }),
            ),
            CapabilityState::full(),
        )
        .is_err());
    }

    #[test]
    fn pages_preflight_rejects_capability_bypass() {
        let capabilities = CapabilityState {
            publish: false,
            ..CapabilityState::full()
        };
        let error = preflight_pages_intent(envelope("save", json!({})), capabilities)
            .expect_err("publish capability denial");
        let PagesBrowserIntentError::Dispatch(error) = error else {
            panic!("expected browser dispatch capability denial");
        };
        assert_eq!(
            browser_capability_denial(&error).map(|denial| denial.capability),
            Some(EditorCapability::Publish)
        );
        assert!(preflight_pages_intent(
            envelope("select", json!({ "component_id": "hero" })),
            capabilities,
        )
        .is_ok());
    }
}
