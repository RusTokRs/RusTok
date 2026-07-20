use crate::builder::{self, PagesBuilderFacade, PagesBuilderSaveSnapshot};
use crate::core;
use crate::transport;
use fly::{RUNTIME_FALLBACK_LOCALES_FIELD, RUNTIME_LOCALE_FIELD, normalize_locale_tag};
use fly_browser::{BrowserIntentEnvelope, FLY_BROWSER_PROTOCOL};
use rustok_page_builder::RuntimeContextExamplePolicy;
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderCapabilityResponse, PublishPageBuilderResult,
};
use rustok_page_builder::runtime_context::{
    PageBuilderRuntimeExampleRequest, generate_page_builder_runtime_example,
};
use rustok_page_builder_admin::{
    AdminCanvasController, BrowserIntentDispatchError, BrowserIntentDispatchResult,
    BrowserIntentEffect, InMemorySsrDraftSessionStore, PageBuilderAdminFacade,
    PageBuilderAdminFacadeError, SsrDraftSessionError, SsrDraftSessionStore,
    dispatch_browser_intent,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::sync::OnceLock;

const MAX_RUNTIME_CONTEXT_BYTES: usize = 256 * 1024;
const MAX_RUNTIME_LOCALE_BYTES: usize = 64;
const MAX_RUNTIME_FALLBACK_LOCALES: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PagesBrowserIntentResponse {
    pub result: BrowserIntentDispatchResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence: Option<PageBuilderCapabilityResponse>,
    pub reload: bool,
    pub draft_token: String,
    pub draft_generation: u64,
    pub runtime_context: Value,
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
    let generated_context = generated_runtime_context(&page, &snapshot.default_locale);

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

    let (mut controller, mut runtime_context, session_token, session_generation) =
        match loaded_session {
            Some(session) if session.controller.revision_id() == persisted_revision => (
                session.controller,
                session.runtime_context,
                Some(session.token),
                Some(session.generation),
            ),
            Some(session) => {
                draft_store.remove(&session.token)?;
                (
                    controller_from_page(&page, &persisted_revision, &snapshot.default_locale)?,
                    generated_context,
                    None,
                    None,
                )
            }
            None => (
                controller_from_page(&page, &persisted_revision, &snapshot.default_locale)?,
                generated_context,
                None,
                None,
            ),
        };

    let context_update = matches!(
        envelope.intent.as_str(),
        "set_runtime_context" | "set_runtime_locale"
    );
    let mut result = match envelope.intent.as_str() {
        "set_runtime_context" => {
            runtime_context = runtime_context_from_payload(&envelope.payload)?;
            controller_snapshot(&controller)?
        }
        "set_runtime_locale" => {
            runtime_context = runtime_locale_from_payload(&runtime_context, &envelope.payload)?;
            controller_snapshot(&controller)?
        }
        _ => dispatch_browser_intent(&mut controller, envelope.clone())?,
    };
    let mut requests = request_effects(&result);

    if !context_update && result.dirty && requests.is_empty() {
        let save = BrowserIntentEnvelope {
            protocol: FLY_BROWSER_PROTOCOL.to_string(),
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
            result = controller_snapshot(&controller)?;
        }
        persistence = Some(response);
    }

    let committed = draft_store.commit_with_context(
        session_token.as_deref(),
        session_generation,
        controller,
        runtime_context.clone(),
    )?;

    Ok(PagesBrowserIntentResponse {
        reload: context_update || envelope.is_mutating() || persistence.is_some(),
        result,
        persistence,
        draft_token: committed.token,
        draft_generation: committed.generation,
        runtime_context,
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

fn generated_runtime_context(page: &crate::model::PageDetail, default_locale: &str) -> Value {
    let seed = core::edit_form_seed_from_page(page, default_locale);
    let project_data = serde_json::from_str::<Value>(&seed.project_data_text)
        .unwrap_or_else(|_| Value::Object(Map::new()));
    let mut context = generate_page_builder_runtime_example(PageBuilderRuntimeExampleRequest {
        project_data,
        policy: RuntimeContextExamplePolicy::default(),
    })
    .ok()
    .map(|response| response.example.input_context)
    .unwrap_or_else(|| Value::Object(Map::new()));
    if let (Some(locale), Some(context)) = (
        normalize_locale_tag(default_locale),
        context.as_object_mut(),
    ) {
        context
            .entry(RUNTIME_LOCALE_FIELD.to_string())
            .or_insert(Value::String(locale));
    }
    context
}

fn runtime_context_from_payload(payload: &Value) -> Result<Value, PagesBrowserIntentError> {
    let context = if let Some(source) = payload.get("context_json").and_then(Value::as_str) {
        if source.len() > MAX_RUNTIME_CONTEXT_BYTES {
            return Err(PagesBrowserIntentError::RuntimeContext(format!(
                "runtime context exceeds {MAX_RUNTIME_CONTEXT_BYTES} bytes"
            )));
        }
        serde_json::from_str::<Value>(source).map_err(|error| {
            PagesBrowserIntentError::RuntimeContext(format!(
                "runtime context JSON is invalid: {error}"
            ))
        })?
    } else if let Some(context) = payload.get("context") {
        context.clone()
    } else {
        payload.clone()
    };
    if !context.is_object() {
        return Err(PagesBrowserIntentError::RuntimeContext(
            "runtime context must be a JSON object".to_string(),
        ));
    }
    Ok(context)
}

fn runtime_locale_from_payload(
    runtime_context: &Value,
    payload: &Value,
) -> Result<Value, PagesBrowserIntentError> {
    let mut context = runtime_context.as_object().cloned().unwrap_or_default();
    let locale_source = payload
        .get("locale")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if locale_source.is_empty() {
        context.remove(RUNTIME_LOCALE_FIELD);
    } else {
        if locale_source.len() > MAX_RUNTIME_LOCALE_BYTES {
            return Err(PagesBrowserIntentError::RuntimeContext(format!(
                "runtime locale exceeds {MAX_RUNTIME_LOCALE_BYTES} bytes"
            )));
        }
        let locale = normalize_locale_tag(locale_source).ok_or_else(|| {
            PagesBrowserIntentError::RuntimeContext(format!(
                "runtime locale `{locale_source}` is invalid"
            ))
        })?;
        context.insert(RUNTIME_LOCALE_FIELD.to_string(), Value::String(locale));
    }

    let fallback_sources = payload
        .get("fallback_locales")
        .map(locale_sources)
        .unwrap_or_default();
    if fallback_sources.len() > MAX_RUNTIME_FALLBACK_LOCALES {
        return Err(PagesBrowserIntentError::RuntimeContext(format!(
            "runtime locale fallback chain exceeds {MAX_RUNTIME_FALLBACK_LOCALES} entries"
        )));
    }
    let mut fallback_locales = Vec::new();
    for source in fallback_sources {
        if source.len() > MAX_RUNTIME_LOCALE_BYTES {
            return Err(PagesBrowserIntentError::RuntimeContext(format!(
                "runtime fallback locale exceeds {MAX_RUNTIME_LOCALE_BYTES} bytes"
            )));
        }
        let locale = normalize_locale_tag(&source).ok_or_else(|| {
            PagesBrowserIntentError::RuntimeContext(format!(
                "runtime fallback locale `{source}` is invalid"
            ))
        })?;
        if !fallback_locales.contains(&locale) {
            fallback_locales.push(locale);
        }
    }
    if fallback_locales.is_empty() {
        context.remove(RUNTIME_FALLBACK_LOCALES_FIELD);
    } else {
        context.insert(
            RUNTIME_FALLBACK_LOCALES_FIELD.to_string(),
            Value::Array(fallback_locales.into_iter().map(Value::String).collect()),
        );
    }
    Ok(Value::Object(context))
}

fn locale_sources(value: &Value) -> Vec<String> {
    match value {
        Value::String(value) => value
            .split([',', ';', '\n'])
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect(),
        Value::Array(values) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn controller_snapshot(
    controller: &AdminCanvasController,
) -> Result<BrowserIntentDispatchResult, PagesBrowserIntentError> {
    let revision = controller.editor().revision();
    Ok(BrowserIntentDispatchResult {
        page_id: controller.page_id().to_string(),
        revision_id: controller.revision_id().to_string(),
        project_hash: revision.project_hash.hex(),
        command_sequence: revision.command_sequence,
        dirty: revision.dirty,
        selected_component_id: controller.ui().state.selection.component_id.clone(),
        project_data: fly::GrapesJsCodec::encode_value(controller.editor().document())?,
        effects: Vec::new(),
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

#[derive(Debug, thiserror::Error)]
pub enum PagesBrowserIntentError {
    #[error("Pages document was not found")]
    PageNotFound,
    #[error("Page Builder browser request targets `{actual}`, but endpoint owns `{expected}`")]
    PageMismatch { expected: String, actual: String },
    #[error("invalid runtime preview context: {0}")]
    RuntimeContext(String),
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

    #[test]
    fn runtime_context_form_requires_a_json_object() {
        assert_eq!(
            runtime_context_from_payload(&json!({
                "context_json": "{\"customer\":{\"name\":\"Ada\"}}"
            }))
            .unwrap()["customer"]["name"],
            "Ada"
        );
        assert!(runtime_context_from_payload(&json!({ "context_json": "[]" })).is_err());
        assert!(runtime_context_from_payload(&json!({ "context_json": "{" })).is_err());
    }

    #[test]
    fn runtime_locale_form_preserves_business_context_and_normalizes_tags() {
        let context = runtime_locale_from_payload(
            &json!({
                "$locale": "en",
                "customer": { "name": "Ada" }
            }),
            &json!({
                "locale": " RU_ru ",
                "fallback_locales": "en, DE-de; en"
            }),
        )
        .expect("locale update");
        assert_eq!(context["$locale"], "ru-ru");
        assert_eq!(context["$fallback_locales"], json!(["en", "de-de"]));
        assert_eq!(context["customer"]["name"], "Ada");
    }

    #[test]
    fn runtime_locale_form_rejects_invalid_tags() {
        assert!(
            runtime_locale_from_payload(&json!({}), &json!({ "locale": "invalid locale" }),)
                .is_err()
        );
        assert!(
            runtime_locale_from_payload(
                &json!({}),
                &json!({ "locale": "ru", "fallback_locales": "en, bad locale" }),
            )
            .is_err()
        );
    }
}
