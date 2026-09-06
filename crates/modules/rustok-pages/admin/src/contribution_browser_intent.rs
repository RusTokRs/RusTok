use crate::browser_intent::{self, PagesBrowserIntentError, PagesBrowserIntentResponse};
use crate::builder::PagesBuilderSaveSnapshot;
use crate::contributions::{
    build_pages_admin_contribution_registry, pages_admin_contribution_policy,
};
use fly_browser::BrowserIntentEnvelope;
use fly_ui::{CapabilityState, PaletteBlockAccess};
use rustok_page_builder_admin::{
    BrowserCapabilityAccessError, BrowserCapabilityDenial, BrowserIntentDispatchError,
    SsrDraftSessionStore, browser_capability_denial, validate_browser_capability_access,
    validate_browser_palette_access,
};

#[derive(Debug, thiserror::Error)]
pub enum PagesBrowserIntentAccessError {
    #[error(transparent)]
    Capability(#[from] BrowserCapabilityAccessError),
    #[error(transparent)]
    Pages(#[from] PagesBrowserIntentError),
}

impl PagesBrowserIntentAccessError {
    pub fn capability_denial(&self) -> Option<&BrowserCapabilityDenial> {
        match self {
            Self::Capability(error) => browser_capability_denial(error),
            Self::Pages(_) => None,
        }
    }
}

pub fn pages_palette_block_access() -> PaletteBlockAccess {
    let assembly = build_pages_admin_contribution_registry(&pages_admin_contribution_policy());
    PaletteBlockAccess::from_assembly(&assembly)
}

pub async fn dispatch_pages_browser_intent(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentAccessError> {
    dispatch_pages_browser_intent_with_capabilities(snapshot, envelope, CapabilityState::full())
        .await
}

pub async fn dispatch_pages_browser_intent_with_capabilities(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
    capabilities: CapabilityState,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentAccessError> {
    let envelope = preflight_pages_intent(envelope, capabilities)?;
    Ok(browser_intent::dispatch_pages_browser_intent(snapshot, envelope).await?)
}

pub async fn dispatch_pages_browser_intent_with_store(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
    draft_store: &dyn SsrDraftSessionStore,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentAccessError> {
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
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentAccessError> {
    let envelope = preflight_pages_intent(envelope, capabilities)?;
    Ok(
        browser_intent::dispatch_pages_browser_intent_with_store(snapshot, envelope, draft_store)
            .await?,
    )
}

fn preflight_pages_intent(
    envelope: BrowserIntentEnvelope,
    capabilities: CapabilityState,
) -> Result<BrowserIntentEnvelope, PagesBrowserIntentAccessError> {
    let envelope = envelope
        .normalized()
        .map_err(BrowserIntentDispatchError::from)
        .map_err(PagesBrowserIntentError::from)?;
    validate_browser_palette_access(&envelope, &pages_palette_block_access())
        .map_err(PagesBrowserIntentError::from)?;
    validate_browser_capability_access(&envelope, capabilities)?;
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_browser::FLY_BROWSER_PROTOCOL;
    use fly_ui::EditorCapability;
    use serde_json::{Value, json};

    fn envelope(intent: &str, payload: Value) -> BrowserIntentEnvelope {
        BrowserIntentEnvelope {
            protocol: FLY_BROWSER_PROTOCOL.to_string(),
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
        assert!(
            preflight_pages_intent(
                envelope("insert_block", json!({ "block_id": "text" })),
                CapabilityState::full(),
            )
            .is_ok()
        );
        assert!(
            preflight_pages_intent(
                envelope("insert_block", json!({ "block_id": "fly.hero" })),
                CapabilityState::full(),
            )
            .is_ok()
        );
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
            PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Dispatch(
                BrowserIntentDispatchError::Authoring(_)
            ))
        ));
    }

    #[test]
    fn pages_preflight_rejects_block_drop_bypass() {
        assert!(
            preflight_pages_intent(
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
            .is_err()
        );
    }

    #[test]
    fn pages_preflight_preserves_typed_capability_denial() {
        let capabilities = CapabilityState {
            publish: false,
            ..CapabilityState::full()
        };
        let error = preflight_pages_intent(envelope("save", json!({})), capabilities)
            .expect_err("publish capability denial");
        assert_eq!(
            error.capability_denial().map(|denial| denial.capability),
            Some(EditorCapability::Publish)
        );
        assert!(matches!(
            error,
            PagesBrowserIntentAccessError::Capability(BrowserCapabilityAccessError::Denied(_))
        ));
        assert!(
            preflight_pages_intent(
                envelope("select", json!({ "component_id": "hero" })),
                capabilities,
            )
            .is_ok()
        );
    }

    #[test]
    fn pages_preflight_rejects_capability_bypass() {
        let capabilities = CapabilityState {
            properties: false,
            ..CapabilityState::full()
        };
        let error = preflight_pages_intent(
            envelope(
                "rename_page",
                json!({ "page_id": "home", "new_page_id": "landing" }),
            ),
            capabilities,
        )
        .expect_err("properties capability denial");
        assert_eq!(
            error.capability_denial().map(|denial| denial.capability),
            Some(EditorCapability::Properties)
        );
    }
}
