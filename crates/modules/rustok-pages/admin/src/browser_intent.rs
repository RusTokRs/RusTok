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
    BrowserIntentEffect, DraftSessionOwner, DurableSsrDraftSessionStore, PageBuilderAdminFacade,
    PageBuilderAdminFacadeError, SaveAttempt, SsrDraftOperationCommit, SsrDraftSessionError,
    SsrDraftSessionSnapshot, SsrDraftSessionStore, dispatch_browser_intent,
    validate_browser_capability_access,
};
use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const MAX_RUNTIME_CONTEXT_BYTES: usize = 256 * 1024;
const MAX_RUNTIME_LOCALE_BYTES: usize = 64;
const MAX_RUNTIME_FALLBACK_LOCALES: usize = 16;

/// Lifetime of a cached persisted-page revision (audit §5.1). After expiry,
/// the next intent re-fetches and can detect another editor's saved revision.
/// A stale draft must never be discarded.
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
            .unwrap_or_else(|poisoned| poisoned.into_inner());
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
            .unwrap_or_else(|poisoned| poisoned.into_inner());
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
            .unwrap_or_else(|poisoned| poisoned.into_inner())
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
    /// preview context or the page the editor is on changed, or a write was answered.
    ///
    /// A preview answer does not reload: it is the document the caller asked for, and a reload
    /// would replace it with a render of the server state, which does not contain it.
    pub reload: bool,
    pub draft_token: String,
    pub draft_generation: u64,
    pub runtime_context: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_status: Option<String>,
    #[serde(default)]
    pub operation_replayed: bool,
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

    // Receipt resolution is read-only. It never resubmits the original command:
    // a missing receipt remains an unknown outcome until the user explicitly retries.
    if envelope.intent == "resolve_operation" {
        let operation_id = envelope.operation_id.as_deref()
            .ok_or(SsrDraftSessionError::InvalidOperationId)?;
        let token = envelope.draft_token.as_deref()
            .ok_or(SsrDraftSessionError::OperationReceiptRequiresSession)?;
        let session = draft_store.load(token, &snapshot.page_id, snapshot.owner.as_ref())?
            .ok_or(PagesBrowserIntentError::DraftUnavailable)?;
        let receipt = draft_store.lookup_operation_receipt(
            token, &snapshot.page_id, snapshot.owner.as_ref(), operation_id, None,
        )?;
        return match receipt {
            Some(receipt) => replay_committed_operation(
                draft_store, token, &snapshot.page_id, snapshot.owner.as_ref(),
                operation_id, &receipt,
            ),
            None => operation_response_from_receipt(&session, operation_id, None, false),
        };
    }

    let operation_hash = envelope.operation_id.as_deref()
        .map(|_| browser_operation_request_hash(&envelope))
        .transpose()?;
    if envelope.operation_id.is_some()
        && (envelope.draft_token.is_none() || envelope.draft_generation.is_none())
    {
        return Err(SsrDraftSessionError::OperationReceiptRequiresSession.into());
    }

    // Load the draft session before any page fetch: a session already in
    // sync with the cached persisted revision lets the dispatch skip the
    // fetch entirely (audit §5.1). The page document is only needed to seed
    // a new controller — never to replace an existing session.
    let loaded_session = match envelope.draft_token.as_deref() {
        Some(token) => Some(
            draft_store
                .load(token, &snapshot.page_id, snapshot.owner.as_ref())?
                .ok_or(PagesBrowserIntentError::DraftUnavailable)?,
        ),
        None => None,
    };
    if let (Some(operation_id), Some(request_hash), Some(session)) = (
        envelope.operation_id.as_deref(),
        operation_hash.as_deref(),
        loaded_session.as_ref(),
    ) {
        let receipt = draft_store.lookup_operation_receipt(
            &session.token,
            &snapshot.page_id,
            snapshot.owner.as_ref(),
            operation_id,
            Some(request_hash),
        )?;
        if let Some(receipt) = receipt {
            return replay_committed_operation(
                draft_store, &session.token, &snapshot.page_id, snapshot.owner.as_ref(),
                operation_id, &receipt,
            );
        }
    }
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

    let mut newly_detected_conflict = false;
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
                match loaded {
                    Some(session) if session.controller.revision_id() == persisted_revision => (
                        session.controller,
                        session.runtime_context,
                        Some(session.token),
                        Some(session.generation),
                    ),
                    Some(mut session) => {
                        // A newer persisted revision says nothing about whether this
                        // session contains irreplaceable work. Never delete or rebase it.
                        newly_detected_conflict = mark_stale_draft_conflict(
                            &mut session.controller,
                            &persisted_revision,
                        )?;
                        (
                            session.controller,
                            session.runtime_context,
                            Some(session.token),
                            Some(session.generation),
                        )
                    }
                    None => (
                        controller_from_page(&page, &persisted_revision, &snapshot.default_locale)?,
                        generated_runtime_context(&page, &snapshot.default_locale),
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
    let mut save_failure = newly_detected_conflict.then(|| {
        PagesBrowserIntentProblem::from_save_failure(
            &PageBuilderAdminFacadeError::with_stable_code(
                "The saved page changed. Your draft is preserved; copy its project JSON before opening the saved page separately.",
                BROWSER_REVISION_CONFLICT_CODE,
            ),
        )
    });

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
            operation_id: None,
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
        let save = if matches!(&request.request, PageBuilderCapabilityRequest::Publish(_)) {
            Some(begin_consumer_save(&mut controller, &request)?)
        } else {
            None
        };
        let facade_snapshot = snapshot.clone();
        let facade = PagesBuilderFacade::new(move || facade_snapshot.clone(), |_page, _project| {});
        match facade.execute(request.request).await {
            Ok(response) => {
                if let PageBuilderCapabilityResponse::Publish(PublishPageBuilderResult {
                    revision_id,
                    ..
                }) = &response
                {
                    let (hash, attempt) = save.as_ref().ok_or_else(|| {
                        fly::FlyError::InvalidSaveState("unexpected save response".to_string())
                    })?;
                    controller.acknowledge_save_for_attempt(*hash, attempt, revision_id.clone())?;
                    // The publish advanced the persisted revision; record it so the
                    // following intents keep hitting the fetch-free fast path
                    // instead of re-fetching the page they just wrote.
                    persisted_revision_cache().put(
                        revision_key,
                        revision_id.clone(),
                        Instant::now(),
                    );
                    result = controller_snapshot(&controller)?;
                    // A successful conditional save resolves a conflict detected earlier
                    // in this dispatch (the persisted revision may have changed again).
                    save_failure = None;
                }
                if save.is_some() && !matches!(&response, PageBuilderCapabilityResponse::Publish(_)) {
                    return Err(fly::FlyError::InvalidSaveState(
                        "save returned a different capability response".to_string(),
                    ).into());
                }
                persistence = Some(response);
            }
            // A rejected save is not a failed dispatch (audit §6.4): the
            // intent already changed the working copy, and the browser has
            // applied it optimistically. Returning an HTTP error here would
            // drop the change from the draft session while the canvas keeps
            // showing it. Instead the change stays in the draft, marked as
            // unsaved, and the client is told to re-render from the server.
            Err(error) if save.is_some() => {
                let (_, attempt) = save.as_ref().ok_or_else(|| {
                    fly::FlyError::InvalidSaveState("missing save attempt".to_string())
                })?;
                record_save_failure(&mut controller, attempt, &error)?;
                // Whatever revision was cached is no longer trustworthy —
                // for a conflict it is known to be stale. The next intent
                // re-fetches the page but preserves the original session.
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

    let reload = reloads_render(
        context_update,
        page_switched,
        envelope.is_mutating(),
        persistence.as_ref(),
        save_failure.is_some(),
    );
    let mut response = PagesBrowserIntentResponse {
        reload,
        result,
        persistence,
        save_failure,
        draft_token: session_token.clone().unwrap_or_default(),
        draft_generation: session_generation.unwrap_or(0),
        runtime_context: runtime_context.clone(),
        operation_id: envelope.operation_id.clone(),
        operation_status: envelope.operation_id.as_ref().map(|_| "committed".to_string()),
        operation_replayed: false,
    };

    if let (Some(operation_id), Some(request_hash)) = (
        envelope.operation_id.as_deref(),
        operation_hash.as_deref(),
    ) {
        if !capabilities.edit {
            return Err(SsrDraftSessionError::OperationReceiptRequiresSession.into());
        }
        let token = session_token.as_deref()
            .ok_or(SsrDraftSessionError::OperationReceiptRequiresSession)?;
        let generation = session_generation
            .ok_or(SsrDraftSessionError::OperationReceiptRequiresSession)?;
        let receipt = compact_operation_receipt(&response)?;
        match draft_store.commit_with_operation_receipt(
            token,
            generation,
            controller,
            runtime_context,
            snapshot.owner.as_ref(),
            operation_id,
            request_hash,
            receipt,
        )? {
            SsrDraftOperationCommit::Committed(committed) => {
                response.draft_token = committed.token;
                response.draft_generation = committed.generation;
                Ok(response)
            }
            SsrDraftOperationCommit::Replayed(receipt) => {
                replay_committed_operation(
                    draft_store, token, &snapshot.page_id, snapshot.owner.as_ref(),
                    operation_id, &receipt,
                )
            }
        }
    } else if capabilities.edit {
        let committed = draft_store.commit_with_context(
            session_token.as_deref(),
            session_generation,
            controller,
            runtime_context,
            snapshot.owner.as_ref(),
        )?;
        response.draft_token = committed.token;
        response.draft_generation = committed.generation;
        Ok(response)
    } else {
        // Read-only callers never touch the draft store (audit §2.5).
        Ok(response)
    }
}

/// Index and id of the page of the document the editor is on, which is the page the server
/// renders into the canvas.
fn open_page(controller: &AdminCanvasController) -> (usize, Option<String>) {
    (
        controller.active_page_index(),
        controller.active_page_summary().and_then(|page| page.id),
    )
}

pub(crate) fn controller_from_page(
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

fn browser_operation_request_hash(
    envelope: &BrowserIntentEnvelope,
) -> Result<String, PagesBrowserIntentError> {
    let mut request = envelope.clone();
    request.operation_id = None;
    let bytes = serde_json::to_vec(&request)
        .map_err(|error| PagesBrowserIntentError::OperationFingerprint(error.to_string()))?;
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Store only the compact acknowledgement. The current controller is the
/// source of truth for document state; replay reloads that current snapshot.
fn compact_operation_receipt(
    response: &PagesBrowserIntentResponse,
) -> Result<Value, PagesBrowserIntentError> {
    let save_failure = response.save_failure.as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| PagesBrowserIntentError::OperationFingerprint(error.to_string()))?;
    let bounded_error = save_failure.as_ref()
        .and_then(Value::as_object)
        .and_then(|failure| failure.get("error"))
        .and_then(Value::as_str)
        .map(|error| error.chars().take(96).collect::<String>());
    let save_failure = match (save_failure, bounded_error) {
        (Some(mut value), Some(error)) => {
            if let Some(failure) = value.as_object_mut() {
                failure.insert("error".to_string(), Value::String(error));
            }
            Some(value)
        }
        (value, _) => value,
    };
    // The current draft session is the source of truth for revision and UI state.
    // A receipt only needs to remember whether this committed operation left a
    // persistence failure that must still be surfaced after a reload.
    Ok(json!({ "save_failure": save_failure }))
}

/// Observe the committed receipt before loading its response snapshot. A
/// snapshot read before the receipt lookup may predate a concurrent commit.
fn replay_committed_operation(
    draft_store: &dyn SsrDraftSessionStore,
    token: &str,
    page_id: &str,
    owner: Option<&DraftSessionOwner>,
    operation_id: &str,
    receipt: &Value,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentError> {
    let session = draft_store
        .load(token, page_id, owner)?
        .ok_or(PagesBrowserIntentError::DraftUnavailable)?;
    operation_response_from_receipt(&session, operation_id, Some(receipt), true)
}

fn operation_response_from_receipt(
    session: &SsrDraftSessionSnapshot,
    operation_id: &str,
    receipt: Option<&Value>,
    replayed: bool,
) -> Result<PagesBrowserIntentResponse, PagesBrowserIntentError> {
    let revision = session.controller.editor().revision();
    let result = BrowserIntentDispatchResult {
        page_id: session.controller.page_id().to_string(),
        revision_id: session.controller.revision_id().to_string(),
        project_hash: revision.project_hash.hex(),
        command_sequence: revision.command_sequence,
        dirty: revision.dirty,
        selected_component_id: session.controller.ui().state.selection.component_id.clone(),
        project_data: None,
        effects: Vec::new(),
    };
    let mut save_failure = match receipt.and_then(|value| value.get("save_failure")) {
        Some(value) if !value.is_null() => Some(
            serde_json::from_value(value.clone())
                .map_err(|error| PagesBrowserIntentError::OperationFingerprint(error.to_string()))?,
        ),
        _ => None,
    };
    if !result.dirty {
        save_failure = None;
    }
    Ok(PagesBrowserIntentResponse {
        result,
        persistence: None,
        save_failure,
        reload: replayed,
        draft_token: session.token.clone(),
        draft_generation: session.generation,
        runtime_context: session.runtime_context.clone(),
        operation_id: Some(operation_id.to_string()),
        operation_status: Some(if replayed { "committed" } else { "unknown" }.to_string()),
        operation_replayed: replayed,
    })
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

/// Whether the dispatch invalidated the server render the browser holds.
///
/// A capability answer is not a reason to re-render: a preview carries the document the client
/// asked for, and a reload would replace it with a render of a server state that cannot contain
/// it. Only an answer that changed what the server holds — a write — makes the held render stale.
fn reloads_render(
    context_update: bool,
    page_switched: bool,
    intent_mutated: bool,
    persistence: Option<&PageBuilderCapabilityResponse>,
    save_failed: bool,
) -> bool {
    let persisted_a_write = matches!(persistence, Some(PageBuilderCapabilityResponse::Publish(_)));
    context_update || page_switched || intent_mutated || persisted_a_write || save_failed
}

/// The one consumer request a dispatch may produce (audit §6.3).
///
/// Every intent yields at most one request today, and the response has room
/// for one `persistence` outcome. A second request is a contract change that
/// must be handled deliberately; failing the dispatch — before anything is
/// executed — beats silently dropping every request after the first.
#[derive(Debug, Clone, PartialEq)]
struct ConsumerRequest {
    request: PageBuilderCapabilityRequest,
    expected_hash: Option<String>,
    command_sequence: u64,
}

fn single_request(
    mut requests: Vec<ConsumerRequest>,
) -> Result<Option<ConsumerRequest>, PagesBrowserIntentError> {
    if requests.len() > 1 {
        return Err(PagesBrowserIntentError::UnsupportedRequestBatch {
            count: requests.len(),
        });
    }
    Ok(requests.pop())
}

/// Record a changed saved revision without deleting, rebasing, or acknowledging the draft.
/// Returns true only when this detection needs to be announced to the browser.
pub(crate) fn mark_stale_draft_conflict(
    controller: &mut AdminCanvasController,
    persisted_revision: &str,
) -> Result<bool, PagesBrowserIntentError> {
    if controller.revision_id() == persisted_revision {
        return Ok(false);
    }
    let was_conflicted = controller.editor().revision().save_conflict;
    if !was_conflicted {
        controller.mark_save_conflict(
            controller.revision_id().to_string(),
            persisted_revision.to_string(),
        )?;
    }
    Ok(!was_conflicted)
}

/// Validate the exact persistence effect before starting transport.
fn begin_consumer_save(
    controller: &mut AdminCanvasController,
    request: &ConsumerRequest,
) -> Result<(fly::ProjectHash, SaveAttempt), PagesBrowserIntentError> {
    let revision = controller.editor().revision();
    let hash = revision.project_hash;
    if request.expected_hash.as_deref() != Some(hash.hex().as_str())
        || request.command_sequence != revision.command_sequence
    {
        return Err(fly::FlyError::InvalidSaveState(
            "save effect does not match the current document snapshot".to_string(),
        ).into());
    }
    let attempt = controller.mark_save_started_with_id()?;
    Ok((hash, attempt))
}

/// Marks the working copy as "changes not on the server" after a rejected
/// save: a lost update is recorded as a conflict (retrying against the same
/// revision would conflict again), anything else as a failed save.
fn record_save_failure(
    controller: &mut AdminCanvasController,
    attempt: &SaveAttempt,
    error: &PageBuilderAdminFacadeError,
) -> Result<(), PagesBrowserIntentError> {
    let applied = if error.stable_code.as_deref() == Some(BROWSER_REVISION_CONFLICT_CODE) {
        let expected = controller.revision_id().to_string();
        controller.mark_save_conflict_for_attempt(attempt, expected, "a newer revision".to_string())?
    } else {
        controller.mark_save_failed_for_attempt(attempt)?
    };
    if !applied {
        return Err(fly::FlyError::InvalidSaveState(
            "save failure does not match the active attempt".to_string(),
        ).into());
    }
    Ok(())
}

fn request_effects(result: &BrowserIntentDispatchResult) -> Vec<ConsumerRequest> {
    result
        .effects
        .iter()
        .filter_map(|effect| match effect {
            BrowserIntentEffect::Request { request, expected_hash, command_sequence } => Some(ConsumerRequest {
                request: request.clone(),
                expected_hash: expected_hash.clone(),
                command_sequence: *command_sequence,
            }),
            BrowserIntentEffect::Announce { .. } => None,
        })
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum PagesBrowserIntentError {
    #[error("Pages document was not found")]
    PageNotFound,
    #[error(
        "Draft session is unavailable or expired; open the saved page in a private window using an address without fly_draft"
    )]
    DraftUnavailable,
    #[error("{}", crate::core::DOCUMENT_UNAVAILABLE_MESSAGE)]
    DocumentUnavailable,
    #[error("Page Builder browser request targets `{actual}`, but endpoint owns `{expected}`")]
    PageMismatch { expected: String, actual: String },
    #[error("Could not fingerprint the browser operation: {0}")]
    OperationFingerprint(String),
    #[error("invalid runtime preview context: {0}")]
    RuntimeContext(String),
    #[error("server preview failed: {0}")]
    Preview(String),
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
        assert_eq!(request_effects(&result), vec![ConsumerRequest {
            request,
            expected_hash: None,
            command_sequence: 1,
        }]);
    }

    fn save_request(revision_id: &str) -> ConsumerRequest {
        ConsumerRequest {
            request: PageBuilderCapabilityRequest::Publish(rustok_page_builder::dto::PublishPageBuilderInput {
                page_id: "home".to_string(),
                revision_id: revision_id.to_string(),
                project_data: json!({ "pages": [] }),
            }),
            expected_hash: None,
            command_sequence: 0,
        }
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

            operation_id: None,
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
    fn a_preview_answer_is_not_a_reason_to_reload() {
        // `reload` is derived from what the answer did to the server, not from whether an answer
        // exists. A preview is a read: a reload would replace the document the caller just asked
        // for with a render of the state the server already held.
        let preview = PageBuilderCapabilityResponse::Preview(
            rustok_page_builder::dto::PreviewPageBuilderResult {
                page_id: "home".to_string(),
                html: "<p>preview</p>".to_string(),
                runtime_scenario_id: None,
            },
        );
        assert!(
            !reloads_render(false, false, false, Some(&preview), false),
            "a preview answer must not reload: it would discard the document it carries"
        );
        assert!(!reloads_render(false, false, false, None, false));
        // The rule still reports every write and every mutation.
        let publish = PageBuilderCapabilityResponse::Publish(
            rustok_page_builder::dto::PublishPageBuilderResult {
                page_id: "home".to_string(),
                revision_id: "rev-2".to_string(),
                published: true,
            },
        );
        assert!(reloads_render(false, false, false, Some(&publish), false));
        assert!(reloads_render(false, false, true, Some(&preview), false));
        assert!(reloads_render(true, false, false, Some(&preview), false));
        assert!(reloads_render(false, true, false, Some(&preview), false));
        assert!(reloads_render(false, false, false, Some(&preview), true));
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
    fn stale_draft_keeps_its_document_and_generation_guard() {
        use rustok_page_builder_admin::InMemorySsrDraftSessionStore;
        let store = InMemorySsrDraftSessionStore::editor_default();
        let initial = store
            .commit_with_context(
                None,
                None,
                dirty_controller(),
                json!({"locale": "en"}),
                None,
            )
            .expect("create draft");
        let mut stale = store.load(&initial.token, "home", None).unwrap().unwrap();
        let original_hash = stale.controller.editor().revision().project_hash;
        assert!(!mark_stale_draft_conflict(&mut stale.controller, "rev-1").unwrap());
        assert!(mark_stale_draft_conflict(&mut stale.controller, "rev-2").unwrap());
        assert!(!mark_stale_draft_conflict(&mut stale.controller, "rev-2").unwrap());
        assert_eq!(stale.controller.revision_id(), "rev-1");
        assert_eq!(
            stale.controller.editor().revision().project_hash,
            original_hash
        );
        assert!(stale.controller.editor().revision().dirty);
        assert!(stale.controller.editor().revision().save_conflict);

        // A concurrent session writer wins the store CAS; stale handling must not
        // remove or overwrite the newer generation.
        let concurrent = store
            .commit_with_context(
                Some(&initial.token),
                Some(initial.generation),
                dirty_controller(),
                json!({"locale": "ru"}),
                None,
            )
            .unwrap();
        let rejected = store.commit_with_context(
            Some(&stale.token),
            Some(stale.generation),
            stale.controller,
            stale.runtime_context,
            None,
        );
        assert!(matches!(
            rejected,
            Err(SsrDraftSessionError::GenerationConflict { .. })
        ));
        let retained = store.load(&initial.token, "home", None).unwrap().unwrap();
        assert_eq!(retained.generation, concurrent.generation);
        assert_eq!(retained.runtime_context, json!({"locale": "ru"}));
        assert_eq!(
            retained.controller.editor().revision().project_hash,
            original_hash
        );
    }

    #[tokio::test]
    async fn missing_session_token_never_seeds_a_new_page() {
        use rustok_page_builder_admin::InMemorySsrDraftSessionStore;
        let store = InMemorySsrDraftSessionStore::editor_default();
        let mut envelope = form_envelope("select", json!({"component_id": "hero"}));
        envelope.draft_token = Some("gone-token".to_string());
        let snapshot = PagesBuilderSaveSnapshot {
            token: None,
            tenant_slug: None,
            page_id: "home".to_string(),
            default_locale: "en".to_string(),
            owner: None,
        };
        let result = dispatch_pages_browser_intent_with_store(
            snapshot,
            envelope,
            &store,
            CapabilityState::full(),
        )
        .await;
        assert!(matches!(
            result,
            Err(PagesBrowserIntentError::DraftUnavailable)
        ));
    }

    #[test]
    fn consumer_save_preserves_effect_identity_and_rejects_stale_effects() {
        let mut controller = dirty_controller();
        let mut envelope = form_envelope("save", json!({}));
        envelope.project_hash = Some(controller.editor().revision().project_hash.hex());
        let result = dispatch_browser_intent(&mut controller, envelope).expect("save intent");
        let request = single_request(request_effects(&result)).expect("one effect").expect("save effect");
        let mut stale = request.clone();
        stale.command_sequence = stale.command_sequence.saturating_add(1);
        assert!(begin_consumer_save(&mut controller, &stale).is_err());
        assert!(controller.active_save_attempt().is_none());

        let (hash, attempt) = begin_consumer_save(&mut controller, &request).expect("begin save");
        assert_eq!(controller.active_save_attempt().as_ref(), Some(&attempt));
        controller.acknowledge_save_for_attempt(hash, &attempt, "rev-2").expect("acknowledge");
        assert_eq!(controller.revision_id(), "rev-2");
        assert!(!controller.editor().revision().dirty);
        assert!(controller.active_save_attempt().is_none());
        assert!(record_save_failure(
            &mut controller, &attempt, &PageBuilderAdminFacadeError::new("late failure"),
        ).is_err());
        assert!(!controller.editor().revision().save_failed);
    }

    #[test]
    fn failed_save_keeps_the_edit_and_marks_the_draft_unsaved() {
        let mut controller = dirty_controller();
        let edited_hash = controller.editor().revision().project_hash;
        let attempt = controller.mark_save_started_with_id().expect("start save");
        record_save_failure(
            &mut controller,
            &attempt,
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

        let attempt = controller.mark_save_started_with_id().expect("start save");
        record_save_failure(
            &mut controller,
            &attempt,
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
        let attempt = controller.mark_save_started_with_id().expect("start save");
        record_save_failure(
            &mut controller,
            &attempt,
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
    fn receipt_replay_precedes_generation_conflict_and_resolution_never_dispatches() {
        let source = include_str!("browser_intent.rs");
        let resolution = source
            .find("if envelope.intent == \"resolve_operation\"")
            .expect("read-only receipt resolution");
        let dispatch = source
            .find("dispatch_browser_intent(")
            .expect("normal intent dispatch");
        assert!(resolution < dispatch, "receipt resolution must not execute the command");

        let receipt_lookup = source
            .find("if let (Some(operation_id), Some(request_hash), Some(session))")
            .expect("same-request receipt lookup");
        let generation_check = source
            .find("if let (Some(expected), Some(session))")
            .expect("draft generation check");
        assert!(
            receipt_lookup < generation_check,
            "a committed request must replay even when its original generation is now stale"
        );
    }

    #[test]
    fn operation_fingerprint_ignores_identity_but_binds_the_exact_command() {
        let mut envelope = form_envelope("insert_block", json!({ "block_id": "hero" }));
        envelope.draft_token = Some("draft-token".to_string());
        envelope.draft_generation = Some(4);
        envelope.operation_id = Some("operation-a".to_string());
        let first = browser_operation_request_hash(&envelope).expect("fingerprint");

        envelope.operation_id = Some("operation-b".to_string());
        assert_eq!(
            first,
            browser_operation_request_hash(&envelope).expect("same command fingerprint"),
            "the id is a lookup key, not part of the request fingerprint"
        );

        envelope.payload = json!({ "block_id": "footer" });
        assert_ne!(
            first,
            browser_operation_request_hash(&envelope).expect("changed command fingerprint")
        );
    }

    #[test]
    fn operation_receipt_projection_is_compact_and_replay_uses_current_session() {
        let response = PagesBrowserIntentResponse {
            reload: false,
            result: BrowserIntentDispatchResult {
                page_id: "home".to_string(),
                revision_id: "rev-2".to_string(),
                project_hash: "hash-2".to_string(),
                command_sequence: 7,
                dirty: true,
                selected_component_id: Some("hero".to_string()),
                project_data: Some(json!({ "pages": [{ "component": { "content": "large" } }] })),
                effects: Vec::new(),
            },
            persistence: None,
            save_failure: None,
            draft_token: "draft-token".to_string(),
            draft_generation: 4,
            runtime_context: json!({ "preview": { "large": "context" } }),
            operation_id: Some("operation-a".to_string()),
            operation_status: Some("committed".to_string()),
            operation_replayed: false,
        };
        let compact = compact_operation_receipt(&response).expect("compact receipt");
        assert!(compact.get("result").is_none());
        assert!(compact.get("runtime_context").is_none());
        assert!(compact.get("persistence").is_none());
        assert_eq!(compact["save_failure"], Value::Null);

        let session = SsrDraftSessionSnapshot {
            token: "draft-token".to_string(),
            generation: 9,
            controller: two_page_controller(),
            runtime_context: json!({ "latest": true }),
            owner: None,
        };
        let unknown = operation_response_from_receipt(&session, "operation-a", None, false)
            .expect("unknown operation response");
        assert_eq!(unknown.operation_status.as_deref(), Some("unknown"));
        assert!(!unknown.reload);
        assert_eq!(unknown.draft_generation, 9);
        assert!(unknown.result.project_data.is_none());

        let replay = operation_response_from_receipt(
            &session,
            "operation-a",
            Some(&compact),
            true,
        ).expect("replayed operation response");
        assert_eq!(replay.operation_status.as_deref(), Some("committed"));
        assert!(replay.operation_replayed);
        assert!(replay.reload);
        assert_eq!(replay.draft_generation, 9);
    }

    #[test]
    fn committed_receipt_replay_reloads_a_snapshot_read_before_the_commit() {
        let store = rustok_page_builder_admin::InMemorySsrDraftSessionStore::default();
        let first = store.commit(None, None, two_page_controller(), None)
            .expect("create draft");
        let old = store.load(&first.token, "home", None)
            .expect("load").expect("draft before commit");
        let context = json!({ "latest": true });
        let hash = "a".repeat(64);
        let receipt = json!({ "save_failure": null });
        let committed = store.commit_with_operation_receipt(
            &first.token, first.generation, first.controller.clone(),
            context.clone(), None, "operation-123", &hash, receipt,
        ).expect("commit operation");
        let SsrDraftOperationCommit::Committed(committed) = committed else {
            panic!("first operation must commit");
        };
        let receipt = store.lookup_operation_receipt(
            &first.token, "home", None, "operation-123", Some(&hash),
        ).expect("lookup").expect("committed receipt");
        let response = replay_committed_operation(
            &store, &old.token, "home", None, "operation-123", &receipt,
        ).expect("replay");
        assert!(response.draft_generation > old.generation);
        assert_eq!(response.draft_generation, committed.generation);
        assert_eq!(response.runtime_context, context);
        assert_eq!(response.operation_status.as_deref(), Some("committed"));
        assert!(response.operation_replayed && response.reload);
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
                operation_id: None,
                operation_status: None,
                operation_replayed: false,
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
