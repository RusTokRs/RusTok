use crate::browser_intent::{
    self, PagesBrowserIntentError, PagesBrowserIntentResponse,
};
use crate::builder::PagesBuilderSaveSnapshot;
use crate::contributions::{
    build_pages_admin_contribution_registry, pages_admin_contribution_policy,
};
use fly_browser::BrowserIntentEnvelope;
use fly_ui::PaletteBlockAccess;
use rustok_page_builder_admin::{
    validate_browser_palette_access, BrowserIntentDispatchError, SsrDraftSessionStore,
};

pub fn pages_palette_block_access() -> PaletteBlockAccess {
    let assembly = build_pages_admin_contribution_registry(&pages_admin_contribution_policy());
    PaletteBlockAccess::from_assembly(&assembly)
}

pub async fn dispatch_pages_browser_intent(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentError> {
    let envelope = preflight_pages_palette_intent(envelope)?;
    browser_intent::dispatch_pages_browser_intent(snapshot, envelope).await
}

pub async fn dispatch_pages_browser_intent_with_store(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
    draft_store: &dyn SsrDraftSessionStore,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentError> {
    let envelope = preflight_pages_palette_intent(envelope)?;
    browser_intent::dispatch_pages_browser_intent_with_store(snapshot, envelope, draft_store).await
}

fn preflight_pages_palette_intent(
    envelope: BrowserIntentEnvelope,
) -> Result<BrowserIntentEnvelope, PagesBrowserIntentError> {
    let envelope = envelope
        .normalized()
        .map_err(BrowserIntentDispatchError::from)?;
    validate_browser_palette_access(&envelope, &pages_palette_block_access())?;
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_browser::FLY_BROWSER_PROTOCOL_V1;
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
        assert!(preflight_pages_palette_intent(envelope(
            "insert_block",
            json!({ "block_id": "text" }),
        ))
        .is_ok());
        assert!(preflight_pages_palette_intent(envelope(
            "insert_block",
            json!({ "block_id": "fly.hero" }),
        ))
        .is_ok());
    }

    #[test]
    fn pages_preflight_rejects_uncontributed_namespaced_templates() {
        let error = preflight_pages_palette_intent(envelope(
            "insert_block",
            json!({ "block_id": "plugin.secret" }),
        ))
        .expect_err("uncontributed block");
        assert!(matches!(
            error,
            PagesBrowserIntentError::Dispatch(BrowserIntentDispatchError::Authoring(_))
        ));
    }

    #[test]
    fn pages_preflight_rejects_block_drop_bypass() {
        assert!(preflight_pages_palette_intent(envelope(
            "drop",
            json!({
                "source": { "kind": "block", "block_id": "fly.pricing" },
                "target_component_id": "root",
                "position": "inside"
            }),
        ))
        .is_err());
    }
}
