use crate::builder::{self, PagesBuilderFacade, PagesBuilderSaveSnapshot};
use crate::core;
use crate::transport;
use fly_browser::{BrowserIntentEnvelope, FLY_BROWSER_PROTOCOL_V1};
use fly_ui::UiIntent;
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderCapabilityResponse, PublishPageBuilderResult,
};
use rustok_page_builder_admin::{
    dispatch_browser_intent, AdminCanvasController, BrowserIntentDispatchError,
    BrowserIntentDispatchResult, BrowserIntentEffect, PageBuilderAdminFacade,
    PageBuilderAdminFacadeError,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PagesBrowserIntentResponse {
    pub result: BrowserIntentDispatchResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence: Option<PageBuilderCapabilityResponse>,
    pub reload: bool,
}

pub async fn dispatch_pages_browser_intent(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentError> {
    let envelope = envelope
        .normalized()
        .map_err(BrowserIntentDispatchError::from)?;
    if envelope.page_id.as_deref().is_some_and(|page_id| page_id != snapshot.page_id) {
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
    let revision_id = builder::page_revision(&page);
    let seed = core::edit_form_seed_from_page(&page, &snapshot.default_locale);
    let mut controller = builder::controller_from_project(
        &page.id,
        &revision_id,
        &seed.project_data_text,
    )?;

    apply_selection_hint(&mut controller, &envelope.payload)?;
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
            revision: Some(revision_id.clone()),
            project_hash: Some(controller.editor().revision().project_hash.hex()),
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
            result.project_data = fly::GrapesJsV1Codec::encode_value(controller.editor().document())?;
        }
        persistence = Some(response);
    }

    Ok(PagesBrowserIntentResponse {
        reload: envelope.is_mutating(),
        result,
        persistence,
    })
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

fn apply_selection_hint(
    controller: &mut AdminCanvasController,
    payload: &Value,
) -> Result<(), PagesBrowserIntentError> {
    let Some(component_id) = payload
        .get("selected_component_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|component_id| !component_id.is_empty())
    else {
        return Ok(());
    };
    controller.dispatch(UiIntent::Select(Some(component_id.to_string())))?;
    Ok(())
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
