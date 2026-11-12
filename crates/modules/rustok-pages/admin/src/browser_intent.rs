use crate::browser_problem::{BROWSER_REVISION_CONFLICT_CODE, PagesBrowserIntentProblem};
use crate::builder::{self, PagesBuilderFacade, PagesBuilderSaveSnapshot};
use crate::core;
use crate::transport;
use fly::{RUNTIME_FALLBACK_LOCALES_FIELD, RUNTIME_LOCALE_FIELD, normalize_locale_tag};
use fly_browser::{BrowserIntentEnvelope, FLY_BROWSER_PROTOCOL};
use fly_ui::{CapabilityState, UiIntent};
use rustok_page_builder::RuntimeContextExamplePolicy;
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderCapabilityResponse, PublishPageBuilderResult,
};
use rustok_page_builder::runtime_context::{
    PageBuilderRuntimeExampleRequest, generate_page_builder_runtime_example,
};
use rustok_page_builder_admin::{
    AdminCanvasController, BrowserIntentDispatchError, BrowserIntentDispatchResult,
    BrowserIntentEffect, DurableSsrDraftSessionStore, PageBuilderAdminFacade,
    PageBuilderAdminFacadeError, SsrDraftSessionError, SsrDraftSessionStore,
    dispatch_browser_intent, validate_browser_capability_access,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const MAX_RUNTIME_CONTEXT_BYTES: usize = 256 * 1024;
const MAX_RUNTIME_LOCALE_BYTES: usize = 64;
const MAX_RUNTIME_FALLBACK_LOCALES: usize = 16;

/// Lifetime of a cached persisted-page revision (audit §5.1). Bounded so a
/// revision saved by another editor is picked up (and the draft session
/// re-seeded) within this window even if this process never re-fetches.
const PERSISTED_REVISION_TTL: Duration = Duration::from_secs(30);
/// Upper bound on cached (token, tenant, page) revision entries.
const PERSISTED_REVISION_CACHE_MAX_ENTRIES: usize = 512;

static CUSTOM_STORE: OnceLock<Arc<dyn SsrDraftSessionStore>> = OnceLock::new();
static DEFAULT_STORE: OnceLock<DurableSsrDraftSessionStore> = OnceLock::new();
static PERSISTED_REVISION_CACHE: OnceLock<PersistedRevisionCache> = OnceLock::new();

/// In-process cache mapping an auth context (token, tenant, page) to the
/// persisted revision observed for it, letting interactive intents skip the
/// `fetch_page` round-trip while their draft session is already in sync
/// (audit §5.1). Correctness does not depend on freshness: the save path
/// re-fetches the page and fails with `REVISION_CONFLICT` before writing, so
/// a stale entry can only delay detection of a concurrent edit within the
/// TTL — it can never overwrite one.
#[derive(Default)]
struct PersistedRevisionCache {
    entries: Mutex<HashMap<u64, (String, Instant)>>,
}

impl PersistedRevisionCache {
    fn get(&self, key: u64, now: Instant) -> Option<String> {
        let mut entries = self
            .entries
            .lock()
            .expect("persisted revision cache lock poisoned");
        match entries.get(&key) {
            Some((revision, expires_at)) if now < *expires_at => Some(revision.clone()),
            Some(_) => {
                entries.remove(&key);
                None
            }
            None => None,
        }
    }

    fn put(&self, key: u64, revision: String, now: Instant) {
        let mut entries = self
            .entries
            .lock()
            .expect("persisted revision cache lock poisoned");
        entries.retain(|_, (_, expires_at)| now < *expires_at);
        if entries.len() >= PERSISTED_REVISION_CACHE_MAX_ENTRIES {
            // Bounded memory over correctness-critical state: dropping the
            // whole map only costs extra fetches, never wrong answers.
            entries.clear();
        }
        entries.insert(key, (revision, now + PERSISTED_REVISION_TTL));
    }

    /// Forgets the revision cached for `key`, so the next intent re-fetches
    /// the page instead of trusting a revision a failed save just disproved.
    fn invalidate(&self, key: u64) {
        self.entries
            .lock()
            .expect("persisted revision cache lock poisoned")
            .remove(&key);
    }
}

fn persisted_revision_cache() -> &'static PersistedRevisionCache {
    PERSISTED_REVISION_CACHE.get_or_init(PersistedRevisionCache::default)
}

/// Cache key derived from the auth context and page. The token is hashed so
/// credential material is not duplicated into long-lived cache entries, and
/// distinct tokens never share a view of a page's persisted revision.
fn persisted_revision_key(snapshot: &PagesBuilderSaveSnapshot) -> u64 {
    let mut hasher = DefaultHasher::new();
    snapshot.token.hash(&mut hasher);
    snapshot.tenant_slug.hash(&mut hasher);
    snapshot.page_id.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PagesBrowserIntentResponse {
    pub result: BrowserIntentDispatchResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence: Option<PageBuilderCapabilityResponse>,
    /// Set when the intent was applied to the draft session but persisting
    /// it failed (audit §6.4). The response is still a success: `result`
    /// carries the draft state (dirty, base revision unchanged) and `reload`
    /// is `true`, so the client re-renders from the server instead of
    /// keeping an optimistic canvas the server never accepted. The problem
    /// uses the same shape as the HTTP error body of this endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save_failure: Option<PagesBrowserIntentProblem>,
    /// `true` when the server render the browser holds is out of date: the document, the
    /// preview context or the page the editor is on changed, or a save was answered.
    pub reload: bool,
    pub draft_token: String,
    pub draft_generation: u64,
    pub runtime_context: Value,
}

/// Registers a custom or distributed SSR draft session store (e.g. backed by Redis or database KV).
/// Must be called during host boot before the first intent is processed.
pub fn set_pages_browser_draft_store(
    store: Arc<dyn SsrDraftSessionStore>,
) -> Result<(), &'static str> {
    CUSTOM_STORE
        .set(store)
        .map_err(|_| "Pages browser draft store has already been initialized")
}

pub fn pages_browser_draft_store() -> &'static dyn SsrDraftSessionStore {
    if let Some(custom) = CUSTOM_STORE.get() {
        return custom.as_ref();
    }
    DEFAULT_STORE.get_or_init(DurableSsrDraftSessionStore::editor_default)
}

pub fn pages_browser_default_durable_store() -> &'static DurableSsrDraftSessionStore {
    DEFAULT_STORE.get_or_init(DurableSsrDraftSessionStore::editor_default)
}

pub async fn dispatch_pages_browser_intent(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
    capabilities: CapabilityState,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentError> {
    dispatch_pages_browser_intent_with_store(
        snapshot,
        envelope,
        pages_browser_draft_store(),
        capabilities,
    )
    .await
}

pub async fn dispatch_pages_browser_intent_with_store(
    snapshot: PagesBuilderSaveSnapshot,
    envelope: BrowserIntentEnvelope,
    draft_store: &dyn SsrDraftSessionStore,
    capabilities: CapabilityState,
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

    // Load the draft session before any page fetch: a session already in
    // sync with the cached persisted revision lets the dispatch skip the
    // fetch entirely (audit §5.1). The page document is only needed to seed
    // or re-seed a controller — never for an in-sync session.
    let loaded_session = match envelope.draft_token.as_deref() {
        Some(token) => draft_store.load(token, &snapshot.page_id, snapshot.owner.as_ref())?,
        None => None,
    };
    if let (Some(expected), Some(session)) = (envelope.draft_generation, loaded_session.as_ref())
        && expected != session.generation
    {
        return Err(PagesBrowserIntentError::Draft(
            SsrDraftSessionError::GenerationConflict {
                expected: session.generation,
                actual: expected,
            },
        ));
    }

    let revision_key = persisted_revision_key(&snapshot);
    let now = Instant::now();
    let session_in_sync = loaded_session.as_ref().is_some_and(|session| {
        persisted_revision_cache()
            .get(revision_key, now)
            .is_some_and(|cached| cached == session.controller.revision_id())
    });

    let (mut controller, mut runtime_context, session_token, session_generation) =
        match loaded_session {
            Some(session) if session_in_sync => (
                session.controller,
                session.runtime_context,
                Some(session.token),
                Some(session.generation),
            ),
            loaded => {
                let page = transport::fetch_page(
                    snapshot.token.clone(),
                    snapshot.tenant_slug.clone(),
                    snapshot.page_id.clone(),
                )
                .await?
                .ok_or(PagesBrowserIntentError::PageNotFound)?;
                // Never seed a controller with the starter document for a page whose
                // document is withheld from this principal.
                if !page.document_readable {
                    return Err(PagesBrowserIntentError::DocumentUnavailable);
                }
                let persisted_revision = builder::page_revision(&page);
                persisted_revision_cache().put(revision_key, persisted_revision.clone(), now);
                let generated_context = generated_runtime_context(&page, &snapshot.default_locale);
                match loaded {
                    Some(session) if session.controller.revision_id() == persisted_revision => (
                        session.controller,
                        session.runtime_context,
                        Some(session.token),
                        Some(session.generation),
                    ),
                    Some(session) => {
                        draft_store.remove(&session.token, snapshot.owner.as_ref())?;
                        (
                            controller_from_page(
                                &page,
                                &persisted_revision,
                                &snapshot.default_locale,
                            )?,
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
                }
            }
        };

    // Narrow the server-side controller to the caller's editor
    // capabilities. The controller is created with the full presentation,
    // so without this every intent routed through its state machine —
    // including the autosave envelope synthesized below — would run
    // against full capabilities instead of the caller's.
    controller
        .dispatch(UiIntent::SetEditableCapabilities(capabilities))
        .map_err(PagesBrowserIntentError::from)?;

    let context_update = matches!(
        envelope.intent.as_str(),
        "set_runtime_context" | "set_runtime_locale"
    );
    let failed_save_before = failed_save_hash(&controller);
    let open_page_before = open_page(&controller);
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
    let mut save_failure = None;

    // A dirty working copy is autosaved after every intent — `select` and
    // `hover` included. Once a save has failed, an intent that leaves the
    // document untouched has nothing new to save: repeating the save would
    // fail the same way and answer every click and hover with a
    // `save_failure` and a reload. The save is retried by the next intent
    // that changes the document, or by an explicit `save`.
    let unchanged_since_failed_save =
        failed_save_before.is_some() && failed_save_before == failed_save_hash(&controller);

    if !context_update && result.dirty && requests.is_empty() && !unchanged_since_failed_save {
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
        // The synthesized autosave is a draft save and must clear the same
        // capability gate as an explicit save. Callers without the save
        // capability keep their working copy dirty — the client shows the
        // unsaved indicator — instead of receiving a late Forbidden from
        // the facade after the intent already mutated the session.
        if validate_browser_capability_access(&save, capabilities).is_ok() {
            match dispatch_browser_intent(&mut controller, save) {
                Ok(save_result) => {
                    requests = request_effects(&save_result);
                    result.effects.extend(save_result.effects);
                }
                // The autosave could not even be requested — typically the
                // edit introduced a blocking validation diagnostic. That is
                // a failed save of an intent that did apply, not a failed
                // intent: handled like a save the consumer rejected below.
                Err(error) => {
                    controller.mark_save_failed()?;
                    save_failure = Some(PagesBrowserIntentProblem::from_blocked_save(&error));
                }
            }
        }
    }

    let mut persistence = None;
    if let Some(request) = single_request(requests)? {
        let is_save = matches!(request, PageBuilderCapabilityRequest::Publish(_));
        let facade_snapshot = snapshot.clone();
        let facade = PagesBuilderFacade::new(move || facade_snapshot.clone(), |_page, _project| {});
        match facade.execute(request).await {
            Ok(response) => {
                if let PageBuilderCapabilityResponse::Publish(PublishPageBuilderResult {
                    revision_id,
                    ..
                }) = &response
                {
                    controller.acknowledge_save(revision_id.clone())?;
                    // The publish advanced the persisted revision; record it so the
                    // following intents keep hitting the fetch-free fast path
                    // instead of re-fetching the page they just wrote.
                    persisted_revision_cache().put(
                        revision_key,
                        revision_id.clone(),
                        Instant::now(),
                    );
                    result = controller_snapshot(&controller)?;
                }
                persistence = Some(response);
            }
            // A rejected save is not a failed dispatch (audit §6.4): the
            // intent already changed the working copy, and the browser has
            // applied it optimistically. Returning an HTTP error here would
            // drop the change from the draft session while the canvas keeps
            // showing it. Instead the change stays in the draft, marked as
            // unsaved, and the client is told to re-render from the server.
            Err(error) if is_save => {
                record_save_failure(&mut controller, &error)?;
                // Whatever revision was cached is no longer trustworthy —
                // for a conflict it is known to be stale. The next intent
                // re-fetches the page and re-seeds the session if needed.
                persisted_revision_cache().invalidate(revision_key);
                result = controller_snapshot(&controller)?;
                save_failure = Some(PagesBrowserIntentProblem::from_save_failure(&error));
            }
            Err(error) => return Err(error.into()),
        }
    }

    // The server renders the page of the document the editor is on, and the draft session
    // committed below carries that page to the next request. An intent that only moves the
    // editor to another page (`activate_page`) changes the render and leaves the document as
    // it is, so no other term of `reload` reports it. A caller without a session keeps
    // nothing and is not sent into a reload that would show the page it came from.
    let page_switched = capabilities.edit && open_page_before != open_page(&controller);

    let (draft_token, draft_generation) = if capabilities.edit {
        let committed = draft_store.commit_with_context(
            session_token.as_deref(),
            session_generation,
            controller,
            runtime_context.clone(),
            snapshot.owner.as_ref(),
        )?;
        (committed.token, committed.generation)
    } else {
        // Read-only callers never touch the draft store (audit §2.5): a
        // customer's `select` must not spawn or bump a session, so the
        // response echoes the loaded token (if any) instead of committing.
        (
            session_token.unwrap_or_default(),
            session_generation.unwrap_or(0),
        )
    };

    Ok(PagesBrowserIntentResponse {
        reload: context_update
            || page_switched
            || envelope.is_mutating()
            || persistence.is_some()
            || save_failure.is_some(),
        result,
        persistence,
        save_failure,
        draft_token,
        draft_generation,
        runtime_context,
    })
}

/// Index and id of the page of the document the editor is on, which is the page the server
/// renders into the canvas.
fn open_page(controller: &AdminCanvasController) -> (usize, Option<String>) {
    (
        controller.active_page_index(),
        controller.active_page_summary().and_then(|page| page.id),
    )
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
        project_data: Some(fly::GrapesJsCodec::encode_value(
            controller.editor().document(),
        )?),
        effects: Vec::new(),
    })
}

/// Hash of the working copy when its last save failed and nothing has been
/// saved since; `None` while the working copy is clean or not yet rejected.
fn failed_save_hash(controller: &AdminCanvasController) -> Option<fly::ProjectHash> {
    let revision = controller.editor().revision();
    revision.save_failed.then_some(revision.project_hash)
}

/// The one consumer request a dispatch may produce (audit §6.3).
///
/// Every intent yields at most one request today, and the response has room
/// for one `persistence` outcome. A second request is a contract change that
/// must be handled deliberately; failing the dispatch — before anything is
/// executed — beats silently dropping every request after the first.
fn single_request(
    mut requests: Vec<PageBuilderCapabilityRequest>,
) -> Result<Option<PageBuilderCapabilityRequest>, PagesBrowserIntentError> {
    if requests.len() > 1 {
        return Err(PagesBrowserIntentError::UnsupportedRequestBatch {
            count: requests.len(),
        });
    }
    Ok(requests.pop())
}

/// Marks the working copy as "changes not on the server" after a rejected
/// save: a lost update is recorded as a conflict (retrying against the same
/// revision would conflict again), anything else as a failed save.
fn record_save_failure(
    controller: &mut AdminCanvasController,
    error: &PageBuilderAdminFacadeError,
) -> Result<(), PagesBrowserIntentError> {
    if error.stable_code.as_deref() == Some(BROWSER_REVISION_CONFLICT_CODE) {
        let expected = controller.revision_id().to_string();
        controller.mark_save_conflict(expected, "a newer revision".to_string())?;
    } else {
        controller.mark_save_failed()?;
    }
    Ok(())
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
    #[error("{}", crate::core::DOCUMENT_UNAVAILABLE_MESSAGE)]
    DocumentUnavailable,
    #[error("Page Builder browser request targets `{actual}`, but endpoint owns `{expected}`")]
    PageMismatch { expected: String, actual: String },
    #[error("invalid runtime preview context: {0}")]
    RuntimeContext(String),
    #[error(
        "Page Builder browser intent produced {count} consumer requests, but one dispatch executes at most one"
    )]
    UnsupportedRequestBatch { count: usize },
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
            project_data: Some(json!({})),
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

    fn save_request(revision_id: &str) -> PageBuilderCapabilityRequest {
        PageBuilderCapabilityRequest::Publish(rustok_page_builder::dto::PublishPageBuilderInput {
            page_id: "home".to_string(),
            revision_id: revision_id.to_string(),
            project_data: json!({ "pages": [] }),
        })
    }

    #[test]
    fn single_request_passes_zero_or_one_and_rejects_a_batch() {
        assert_eq!(single_request(Vec::new()).expect("no request"), None);
        assert_eq!(
            single_request(vec![save_request("rev-1")]).expect("one request"),
            Some(save_request("rev-1"))
        );
        // A second request must never be dropped silently (audit §6.3).
        assert!(matches!(
            single_request(vec![save_request("rev-1"), save_request("rev-2")]),
            Err(PagesBrowserIntentError::UnsupportedRequestBatch { count: 2 })
        ));
    }

    fn dirty_controller() -> AdminCanvasController {
        let mut controller = AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{ "id": "hero", "type": "section" }]
                    }
                }]
            }),
        )
        .expect("controller");
        controller
            .dispatch(UiIntent::execute(fly::EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: fly::ComponentPatch::default().merge_style(json!({ "margin": "8px" })),
            }))
            .expect("edit");
        controller
    }

    fn two_page_controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "home-root",
                        "type": "wrapper",
                        "components": [{ "id": "hero", "type": "section" }]
                    }
                }, {
                    "id": "about",
                    "component": { "id": "about-root", "type": "wrapper" }
                }]
            }),
        )
        .expect("controller")
    }

    fn form_envelope(intent: &str, payload: Value) -> BrowserIntentEnvelope {
        BrowserIntentEnvelope {
            protocol: FLY_BROWSER_PROTOCOL.to_string(),
            instance_id: "pages-endpoint".to_string(),
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

    // `reload` is decided from `open_page` before and after the dispatch. The endpoint itself
    // fetches the page over the transport and has no test; these pin the two halves it reads.
    #[test]
    fn open_page_follows_the_page_a_form_opens_or_creates() {
        let mut controller = two_page_controller();
        assert_eq!(open_page(&controller), (0, Some("home".to_string())));

        let opened = form_envelope("activate_page", json!({ "page_id": "about" }));
        assert!(
            !opened.is_mutating(),
            "opening a page changes no document, so `is_mutating` cannot ask for the reload"
        );
        dispatch_browser_intent(&mut controller, opened).expect("open page form");
        assert_eq!(open_page(&controller), (1, Some("about".to_string())));

        let created = form_envelope("create_page", json!({ "page_id": "contact", "name": "" }));
        dispatch_browser_intent(&mut controller, created).expect("create page form");
        assert_eq!(open_page(&controller), (2, Some("contact".to_string())));
    }

    #[test]
    fn open_page_is_unchanged_by_an_intent_that_stays_on_the_page() {
        let mut controller = two_page_controller();
        let before = open_page(&controller);

        let select = form_envelope("select", json!({ "component_id": "hero" }));
        dispatch_browser_intent(&mut controller, select).expect("select");
        assert_eq!(open_page(&controller), before);

        // Opening the page the editor is already on is not a switch either.
        let same = form_envelope("activate_page", json!({ "page_id": "home" }));
        dispatch_browser_intent(&mut controller, same).expect("open the current page");
        assert_eq!(open_page(&controller), before);
    }

    #[test]
    fn failed_save_keeps_the_edit_and_marks_the_draft_unsaved() {
        let mut controller = dirty_controller();
        let edited_hash = controller.editor().revision().project_hash;
        record_save_failure(
            &mut controller,
            &PageBuilderAdminFacadeError::new("Pages rejected the document"),
        )
        .expect("record failure");

        // The edit is still in the working copy, on the same base revision,
        // and both the editor and the UI state say it is not saved.
        let revision = controller.editor().revision();
        assert_eq!(revision.project_hash, edited_hash);
        assert!(revision.dirty);
        assert!(revision.save_failed);
        assert!(!revision.save_conflict);
        assert!(!revision.save_in_progress);
        assert!(controller.ui().state.dirty.save_failed);
        assert_eq!(controller.revision_id(), "rev-1");

        let snapshot = controller_snapshot(&controller).expect("snapshot");
        assert!(snapshot.dirty);
        assert_eq!(snapshot.revision_id, "rev-1");
    }

    #[test]
    fn failed_save_hash_tracks_the_document_the_save_failed_for() {
        let mut controller = dirty_controller();
        // Dirty but never rejected: the autosave has not been tried yet.
        assert_eq!(failed_save_hash(&controller), None);

        record_save_failure(
            &mut controller,
            &PageBuilderAdminFacadeError::new("Pages rejected the document"),
        )
        .expect("record failure");
        let failed = failed_save_hash(&controller);
        assert!(failed.is_some());

        // An intent that does not touch the document leaves nothing new to save.
        controller
            .dispatch(UiIntent::Select(Some("hero".to_string())))
            .expect("select");
        assert_eq!(failed_save_hash(&controller), failed);

        // A further edit is new work: the hash moves, so the autosave runs again.
        controller
            .dispatch(UiIntent::execute(fly::EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: fly::ComponentPatch::default().merge_style(json!({ "margin": "16px" })),
            }))
            .expect("second edit");
        let after_edit = failed_save_hash(&controller);
        assert!(after_edit.is_some());
        assert_ne!(after_edit, failed);
    }

    #[test]
    fn revision_conflict_on_save_is_recorded_as_a_lost_update() {
        let mut controller = dirty_controller();
        record_save_failure(
            &mut controller,
            &PageBuilderAdminFacadeError::with_stable_code(
                "REVISION_CONFLICT: expected `rev-1`, current `rev-2`",
                BROWSER_REVISION_CONFLICT_CODE,
            ),
        )
        .expect("record conflict");
        let revision = controller.editor().revision();
        assert!(revision.dirty);
        assert!(revision.save_failed);
        assert!(revision.save_conflict);
        assert!(controller.ui().state.dirty.save_conflict);
    }

    #[test]
    fn save_failure_is_serialized_only_when_present() {
        let response = |save_failure: Option<PagesBrowserIntentProblem>| {
            let controller = dirty_controller();
            serde_json::to_value(PagesBrowserIntentResponse {
                result: controller_snapshot(&controller).expect("snapshot"),
                persistence: None,
                reload: save_failure.is_some(),
                save_failure,
                draft_token: "token".to_string(),
                draft_generation: 2,
                runtime_context: json!({}),
            })
            .expect("serialize")
        };
        assert!(response(None).get("save_failure").is_none());

        let failed = response(Some(PagesBrowserIntentProblem::from_save_failure(
            &PageBuilderAdminFacadeError::new("Pages rejected the document"),
        )));
        assert_eq!(failed["reload"], true);
        assert_eq!(failed["result"]["dirty"], true);
        assert_eq!(failed["result"]["revision_id"], "rev-1");
        assert_eq!(
            failed["save_failure"],
            json!({
                "status": 422,
                "error": "Pages rejected the document",
                "code": "SAVE_FAILED",
                "intent": "save"
            })
        );
    }

    #[test]
    fn persisted_revision_cache_invalidates_one_key() {
        let cache = PersistedRevisionCache::default();
        let now = Instant::now();
        cache.put(1, "rev-1".to_string(), now);
        cache.put(2, "rev-2".to_string(), now);
        cache.invalidate(1);
        assert_eq!(cache.get(1, now), None);
        assert_eq!(cache.get(2, now).as_deref(), Some("rev-2"));
        // Invalidating an absent key is a no-op.
        cache.invalidate(1);
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
        assert_eq!(context["$locale"], "ru-RU");
        assert_eq!(context["$fallback_locales"], json!(["en", "de-DE"]));
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

    #[test]
    fn pages_browser_draft_store_resolves_and_stores_session() {
        let default_durable = pages_browser_default_durable_store();
        assert_eq!(default_durable.memory().len().unwrap(), 0);
        let store = pages_browser_draft_store();
        assert!(
            store
                .load("non-existent-token", "home", None)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn persisted_revision_cache_roundtrips_per_key() {
        let cache = PersistedRevisionCache::default();
        let now = Instant::now();
        assert_eq!(cache.get(1, now), None);
        cache.put(1, "rev-1".to_string(), now);
        cache.put(2, "rev-2".to_string(), now);
        assert_eq!(cache.get(1, now).as_deref(), Some("rev-1"));
        assert_eq!(cache.get(2, now).as_deref(), Some("rev-2"));
        // A later write for the same key wins.
        cache.put(1, "rev-9".to_string(), now);
        assert_eq!(cache.get(1, now).as_deref(), Some("rev-9"));
    }

    #[test]
    fn persisted_revision_cache_expires_after_ttl() {
        let cache = PersistedRevisionCache::default();
        let now = Instant::now();
        cache.put(1, "rev-1".to_string(), now);
        let before_ttl = now + PERSISTED_REVISION_TTL - Duration::from_nanos(1);
        assert_eq!(cache.get(1, before_ttl).as_deref(), Some("rev-1"));
        assert_eq!(cache.get(1, now + PERSISTED_REVISION_TTL), None);
        // Expired entries are dropped, not resurrected by older timestamps.
        assert_eq!(cache.get(1, now), None);
    }

    #[test]
    fn persisted_revision_cache_is_bounded() {
        let cache = PersistedRevisionCache::default();
        let now = Instant::now();
        for key in 0..PERSISTED_REVISION_CACHE_MAX_ENTRIES as u64 {
            cache.put(key, format!("rev-{key}"), now);
        }
        assert_eq!(
            cache.entries.lock().unwrap().len(),
            PERSISTED_REVISION_CACHE_MAX_ENTRIES
        );
        // One entry over capacity drops the whole map: correctness never
        // depends on the cache, only latency does.
        cache.put(
            PERSISTED_REVISION_CACHE_MAX_ENTRIES as u64 + 1,
            "rev-overflow".to_string(),
            now,
        );
        let entries = cache.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries
                .values()
                .next()
                .map(|(revision, _)| revision.as_str()),
            Some("rev-overflow")
        );
    }

    #[test]
    fn persisted_revision_key_separates_auth_contexts() {
        let snapshot =
            |token: Option<&str>, tenant: Option<&str>, page: &str| PagesBuilderSaveSnapshot {
                token: token.map(ToString::to_string),
                tenant_slug: tenant.map(ToString::to_string),
                page_id: page.to_string(),
                default_locale: "en".to_string(),
                owner: None,
            };
        let base = snapshot(Some("token-a"), Some("tenant-1"), "home");
        assert_eq!(
            persisted_revision_key(&base),
            persisted_revision_key(&snapshot(Some("token-a"), Some("tenant-1"), "home"))
        );
        assert_ne!(
            persisted_revision_key(&base),
            persisted_revision_key(&snapshot(Some("token-b"), Some("tenant-1"), "home"))
        );
        assert_ne!(
            persisted_revision_key(&base),
            persisted_revision_key(&snapshot(Some("token-a"), Some("tenant-2"), "home"))
        );
        assert_ne!(
            persisted_revision_key(&base),
            persisted_revision_key(&snapshot(Some("token-a"), Some("tenant-1"), "about"))
        );
    }
}
