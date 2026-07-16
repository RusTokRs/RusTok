use crate::builder::{self, PagesBuilderFacade, PagesBuilderSaveSnapshot};
use crate::core;
use crate::transport;
use fly_browser::{BrowserIntentEnvelope, FLY_BROWSER_PROTOCOL_V1};
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderCapabilityResponse, PublishPageBuilderResult,
};
use rustok_page_builder_admin::{
    dispatch_browser_intent, AdminCanvasController, BrowserIntentDispatchError,
    BrowserIntentDispatchResult, BrowserIntentEffect, InMemorySsrDraftSessionStore,
    PageBuilderAdminFacade, PageBuilderAdminFacadeError, SsrDraftSessionError,
    SsrDraftSessionStore,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PagesBrowserIntentResponse {
    pub result: BrowserIntentDispatchResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence: Option<PageBuilderCapabilityResponse>,
    pub reload: bool,
    pub draft_token: String,
    pub draft_generation: u64,
}

pub fn pages_browser_draft_store() -> &'static InMemorySsrDraftSessionStore {
    static STORE: OnceLock<InMemorySsrDraftSessionStore> = OnceLock::new();
    STORE.get_or_init(InMemorySsrDraftSessionStore::editor_default)
}

pub async fn dispatch_pages_browser_intent(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentError> {
    dispatch_pages_browser_intent_with_store(snapshot, envelope, pages_browser_draft_store()).await
}

pub async fn dispatch_pages_browser_intent_with_store(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
    draft_store: &dyn SsrDraftSessionStore,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentError> {
    let envelope = envelope
        .normalized()
        .map_err(BrowserIntentDispatchError::from)?;
    if envelope
        .page_id
        .as_deref()
        .is_some_and(|page_id| page_id != snapshot.page_id)
    {
        return Err(PagesBrowserIntentError::PageMismatch {
            expected: snapshot.page_id,
            actual: envelope.page_id.unwrap_or_default(),
        });
    }

    let page = transport::fetch_page(
        snapshot.token.clone(),
        snapshot.tenant_slug.clone(),
        snapshot.page_id.clone(),
    )
    .await?
    .ok_or(PagesBrowserIntentError::PageNotFound)?;
    let persisted_revision = builder::page_revision(&page);

    let loaded_session = match envelope.draft_token.as_deref() {
        Some(token) => draft_store.load(token, &snapshot.page_id)?,
        None => None,
    };
    if let (Some(expected), Some(session)) = (envelope.draft_generation, loaded_session.as_ref()) {
        if expected != session.generation {
            return Err(PagesBrowserIntentError::Draft(
                SsrDraftSessionError::GenerationConflict {
                    expected: session.generation,
                    actual: expected,
                },
            ));
        }
    }

    let (mut controller, session_token, session_generation) = match loaded_session {
        Some(session) if session.controller.revision_id() == persisted_revision => (
            session.controller,
            Some(session.token),
            Some(session.generation),
        ),
        Some(session) => {
            draft_store.remove(&session.token)?;
            (
                controller_from_page(&page, &persisted_revision, &snapshot.default_locale)?,
                None,
                None,
            )
        }
        None => (
            controller_from_page(&page, &persisted_revision, &snapshot.default_locale)?,
            None,
            None,
        ),
    };

    let mut result = dispatch_browser_intent(&mut controller, envelope.clone())?;
    let mut requests = request_effects(&result);

    if envelope.is_mutating() && result.dirty && requests.is_empty() {
        let save = BrowserIntentEnvelope {
            protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
            instance_id: envelope.instance_id.clone(),
            intent: "save".to_string(),
            payload: json!({}),
            sequence: envelope.sequence.map(|sequence| sequence.saturating_add(1)),
            page_id: Some(snapshot.page_id.clone()),
            revision: Some(controller.revision_id().to_string()),
            project_hash: Some(controller.editor().revision().project_hash.hex()),
            draft_token: session_token.clone(),
            draft_generation: session_generation,
        };
        let save_result = dispatch_browser_intent(&mut controller, save)?;
        requests = request_effects(&save_result);
        result.effects.extend(save_result.effects);
    }

    let mut persistence = None;
    if let Some(request) = requests.into_iter().next() {
        let facade_snapshot = snapshot.clone();
        let facade = PagesBuilderFacade::new(move || facade_snapshot.clone(), |_page, _project| {});
        let response = facade.execute(request).await?;
        if let PageBuilderCapabilityResponse::Publish(PublishPageBuilderResult {
            revision_id,
            ..
        }) = &response
        {
            controller.acknowledge_save(revision_id.clone())?;
            result.revision_id = revision_id.clone();
            result.dirty = false;
            result.project_hash = controller.editor().revision().project_hash.hex();
            result.command_sequence = controller.editor().revision().command_sequence;
            result.project_data =
                fly::GrapesJsV1Codec::encode_value(controller.editor().document())?;
        }
        persistence = Some(response);
    }

    let committed = draft_store.commit(
        session_token.as_deref(),
        session_generation,
        controller,
    )?;

    Ok(PagesBrowserIntentResponse {
        reload: envelope.is_mutating(),
        result,
        persistence,
        draft_token: committed.token,
        draft_generation: committed.generation,
    })
}

fn controller_from_page(
    page: &crate::model::PageDetail,
    revision_id: &str,
    default_locale: &str,
) -> Result<AdminCanvasController, PagesBrowserIntentError> {
    let seed = core::edit_form_seed_from_page(page, default_locale);
    builder::controller_from_project(&page.id, revision_id, &seed.project_data_text)
        .map_err(PagesBrowserIntentError::from)
}

fn request_effects(result: &BrowserIntentDispatchResult) -> Vec<PageBuilderCapabilityRequest> {
    result
        .effects
        .iter()
        .filter_map(|effect| match effect {
            BrowserIntentEffect::Request { request, .. } => Some(request.clone()),
            BrowserIntentEffect::Announce { .. } => None,
        })
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum PagesBrowserIntentError {
    #[error("Pages document was not found")]
    PageNotFound,
    #[error("Page Builder browser request targets `{actual}`, but endpoint owns `{expected}`")]
    PageMismatch { expected: String, actual: String },
    #[error(transparent)]
    Dispatch(#[from] BrowserIntentDispatchError),
    #[error(transparent)]
    Draft(#[from] SsrDraftSessionError),
    #[error(transparent)]
    Facade(#[from] PageBuilderAdminFacadeError),
    #[error(transparent)]
    Transport(#[from] transport::TransportError),
    #[error(transparent)]
    Canvas(#[from] rustok_page_builder_admin::AdminCanvasError),
    #[error(transparent)]
    Fly(#[from] fly::FlyError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_effects_extracts_only_consumer_requests() {
        let request = PageBuilderCapabilityRequest::Publish(
            rustok_page_builder::dto::PublishPageBuilderInput {
                page_id: "home".to_string(),
                revision_id: "rev-1".to_string(),
                schema_version: "grapesjs_v1".to_string(),
                project_data: json!({ "pages": [] }),
            },
        );
        let result = BrowserIntentDispatchResult {
            page_id: "home".to_string(),
            revision_id: "rev-1".to_string(),
            project_hash: "hash".to_string(),
            command_sequence: 1,
            dirty: true,
            selected_component_id: None,
            project_data: json!({}),
            effects: vec![
                BrowserIntentEffect::Announce {
                    message: "changed".to_string(),
                },
                BrowserIntentEffect::Request {
                    request: request.clone(),
                    expected_hash: None,
                    command_sequence: 1,
                },
            ],
        };
        assert_eq!(request_effects(&result), vec![request]);
    }
}
