use crate::model::{PageDetail, PageMutationResult};
use crate::transport;
use leptos::prelude::*;
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderCapabilityResponse, PreviewPageBuilderInput,
    PublishPageBuilderInput,
};
use rustok_page_builder_admin::{
    AdminCanvasController, PageBuilderAdminFacade, PageBuilderAdminFacadeError,
    PageBuilderAdminFacadeFuture,
};
use serde_json::{Value, json};
use std::sync::Arc;

#[cfg(feature = "ssr")]
use async_trait::async_trait;
#[cfg(feature = "ssr")]
use fly::{PageSelection, RenderPolicy};
#[cfg(feature = "ssr")]
use rustok_api::{Action, Permission, PortActor, PortContext, Resource};
#[cfg(feature = "ssr")]
use rustok_page_builder::composition::compose_fly_page_builder_handlers;
#[cfg(feature = "ssr")]
use rustok_page_builder::preview_port::PageBuilderPreviewRenderingPort;
#[cfg(feature = "ssr")]
use rustok_page_builder::render::PageBuilderRenderer;
#[cfg(feature = "ssr")]
use rustok_page_builder::rollout::BuilderCapabilityFlags;
#[cfg(feature = "ssr")]
use rustok_page_builder::service::{
    PageBuilderProjectSaveResult, PageBuilderProjectStore, PageBuilderRequestAuth,
    PageBuilderServiceError, PageBuilderServiceResult,
};
#[cfg(feature = "ssr")]
use std::time::Duration;

const PAGE_PUBLISHED_DOCUMENT_IMMUTABLE: &str = "PAGE_PUBLISHED_DOCUMENT_IMMUTABLE";
const REVISION_CONFLICT: &str = "REVISION_CONFLICT";
#[cfg(feature = "ssr")]
const PAGE_BUILDER_PORT_DEADLINE: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagesBuilderSaveSnapshot {
    pub token: Option<String>,
    pub tenant_slug: Option<String>,
    pub page_id: String,
    pub default_locale: String,
}

type SnapshotProvider = Arc<dyn Fn() -> PagesBuilderSaveSnapshot + Send + Sync>;
type SavedHandler = Arc<dyn Fn(PageMutationResult, Value) + Send + Sync>;

#[derive(Clone)]
pub struct PagesBuilderFacade {
    snapshot: SnapshotProvider,
    on_saved: SavedHandler,
}

impl PagesBuilderFacade {
    pub fn new(
        snapshot: impl Fn() -> PagesBuilderSaveSnapshot + Send + Sync + 'static,
        on_saved: impl Fn(PageMutationResult, Value) + Send + Sync + 'static,
    ) -> Self {
        Self {
            snapshot: Arc::new(snapshot),
            on_saved: Arc::new(on_saved),
        }
    }
}

impl PageBuilderAdminFacade for PagesBuilderFacade {
    fn execute(&self, request: PageBuilderCapabilityRequest) -> PageBuilderAdminFacadeFuture {
        let snapshot = (self.snapshot)();
        let on_saved = Arc::clone(&self.on_saved);
        Box::pin(async move {
            match request {
                PageBuilderCapabilityRequest::Preview(input) => {
                    execute_preview(snapshot, input).await
                }
                PageBuilderCapabilityRequest::Publish(input) => {
                    execute_publish(snapshot, on_saved, input).await
                }
                request => Err(unsupported_request_error(&request)),
            }
        })
    }
}

fn unsupported_request_error(
    request: &PageBuilderCapabilityRequest,
) -> PageBuilderAdminFacadeError {
    PageBuilderAdminFacadeError::new(format!(
        "Pages consumer facade does not support Page Builder `{}` requests",
        request.capability()
    ))
}

fn pages_request_page_id(
    request: &PageBuilderCapabilityRequest,
) -> Result<&str, PageBuilderAdminFacadeError> {
    match request {
        PageBuilderCapabilityRequest::Preview(input) => Ok(&input.page_id),
        PageBuilderCapabilityRequest::Publish(input) => Ok(&input.page_id),
        request => Err(unsupported_request_error(request)),
    }
}

fn ensure_requested_page(
    snapshot: &PagesBuilderSaveSnapshot,
    requested_page_id: &str,
) -> Result<(), PageBuilderAdminFacadeError> {
    if requested_page_id == snapshot.page_id {
        Ok(())
    } else {
        Err(PageBuilderAdminFacadeError::new(format!(
            "Page Builder requested page `{requested_page_id}`, but Pages is editing `{}`",
            snapshot.page_id
        )))
    }
}

fn noop_saved_handler() -> SavedHandler {
    Arc::new(|_, _| {})
}

#[server(prefix = "/api/fn", endpoint = "pages/page-builder-capability")]
async fn pages_page_builder_capability(
    token: Option<String>,
    tenant_slug: Option<String>,
    page_id: String,
    default_locale: String,
    request: PageBuilderCapabilityRequest,
) -> Result<PageBuilderCapabilityResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        dispatch_pages_page_builder_capability(
            PagesBuilderSaveSnapshot {
                token,
                tenant_slug,
                page_id,
                default_locale,
            },
            noop_saved_handler(),
            request,
        )
        .await
        .map_err(|error| ServerFnError::ServerError(error.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant_slug, page_id, default_locale, request);
        Err(ServerFnError::new(
            "pages/page-builder-capability requires the `ssr` feature",
        ))
    }
}

async fn execute_preview(
    snapshot: PagesBuilderSaveSnapshot,
    input: PreviewPageBuilderInput,
) -> Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError> {
    let requested_page_id = input.page_id.clone();
    let requested_runtime_scenario_id = input.runtime.scenario_id.clone();
    let response = dispatch_pages_page_builder_capability(
        snapshot,
        noop_saved_handler(),
        PageBuilderCapabilityRequest::Preview(input),
    )
    .await?;
    match response {
        PageBuilderCapabilityResponse::Preview(result)
            if result.page_id == requested_page_id
                && result.runtime_scenario_id == requested_runtime_scenario_id =>
        {
            Ok(PageBuilderCapabilityResponse::Preview(result))
        }
        PageBuilderCapabilityResponse::Preview(result) if result.page_id != requested_page_id => {
            Err(PageBuilderAdminFacadeError::new(format!(
                "Page Builder preview returned page `{}`, but Pages requested `{requested_page_id}`",
                result.page_id
            )))
        }
        PageBuilderCapabilityResponse::Preview(result) => {
            Err(PageBuilderAdminFacadeError::new(format!(
                "Page Builder preview returned runtime scenario `{:?}`, but Pages requested `{:?}`",
                result.runtime_scenario_id, requested_runtime_scenario_id
            )))
        }
        response => Err(PageBuilderAdminFacadeError::new(format!(
            "Page Builder capability transport returned `{}` for a preview request",
            response.capability()
        ))),
    }
}

async fn execute_publish(
    snapshot: PagesBuilderSaveSnapshot,
    on_saved: SavedHandler,
    input: PublishPageBuilderInput,
) -> Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError> {
    let requested_page_id = input.page_id.clone();
    let response = dispatch_pages_page_builder_capability(
        snapshot,
        on_saved,
        PageBuilderCapabilityRequest::Publish(input),
    )
    .await?;
    match response {
        PageBuilderCapabilityResponse::Publish(result) if result.page_id == requested_page_id => {
            Ok(PageBuilderCapabilityResponse::Publish(result))
        }
        PageBuilderCapabilityResponse::Publish(result) => {
            Err(PageBuilderAdminFacadeError::new(format!(
                "Page Builder publish returned page `{}`, but Pages requested `{requested_page_id}`",
                result.page_id
            )))
        }
        response => Err(PageBuilderAdminFacadeError::new(format!(
            "Page Builder capability transport returned `{}` for a publish request",
            response.capability()
        ))),
    }
}

#[cfg(feature = "ssr")]
async fn dispatch_pages_page_builder_capability(
    snapshot: PagesBuilderSaveSnapshot,
    on_saved: SavedHandler,
    request: PageBuilderCapabilityRequest,
) -> Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError> {
    let requested_page_id = pages_request_page_id(&request)?.to_string();
    ensure_requested_page(&snapshot, &requested_page_id)?;

    let token = required_snapshot_value(snapshot.token, "access token")?;
    let tenant_slug = required_snapshot_value(snapshot.tenant_slug, "tenant")?;
    let verified_user = leptos_auth::api::fetch_current_user(token.clone(), tenant_slug.clone())
        .await
        .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?
        .ok_or_else(|| PageBuilderAdminFacadeError::new("Authenticated user was not found"))?;
    let permissions = page_builder_permissions_for_role(&verified_user.role);
    let actor_id = verified_user.id;
    let auth = PageBuilderRequestAuth::new(permissions);
    let context = match &request {
        PageBuilderCapabilityRequest::Preview(input) => PortContext::new(
            tenant_slug.clone(),
            PortActor::user(actor_id.clone()),
            snapshot.default_locale.clone(),
            format!("page-builder-preview:{}", input.page_id),
        )
        .with_deadline(PAGE_BUILDER_PORT_DEADLINE),
        PageBuilderCapabilityRequest::Publish(input) => PortContext::new(
            tenant_slug.clone(),
            PortActor::user(actor_id.clone()),
            snapshot.default_locale.clone(),
            format!("page-builder:{}:{}", input.page_id, input.revision_id),
        )
        .with_deadline(PAGE_BUILDER_PORT_DEADLINE)
        .with_idempotency_key(format!(
            "page-builder-save:{}:{}",
            input.page_id, input.revision_id
        )),
        request => return Err(unsupported_request_error(request)),
    };

    let renderer = PagesPageBuilderRenderer {
        token: token.clone(),
        tenant_slug: tenant_slug.clone(),
        actor_id: actor_id.clone(),
        page_id: snapshot.page_id.clone(),
    };
    let store = PagesPageBuilderProjectStore {
        token,
        tenant_slug,
        actor_id,
        page_id: snapshot.page_id,
        default_locale: snapshot.default_locale,
        on_saved,
    };
    let expected_capability = request.capability();
    let handlers =
        compose_fly_page_builder_handlers(store, renderer, BuilderCapabilityFlags::default())
            .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?;
    let response = handlers
        .handle(&context, &auth, request)
        .await
        .map_err(facade_service_error)?;
    if response.capability() != expected_capability {
        return Err(PageBuilderAdminFacadeError::new(format!(
            "Page Builder composition returned `{}` for a `{expected_capability}` request",
            response.capability()
        )));
    }
    Ok(response)
}

#[cfg(not(feature = "ssr"))]
async fn dispatch_pages_page_builder_capability(
    snapshot: PagesBuilderSaveSnapshot,
    on_saved: SavedHandler,
    request: PageBuilderCapabilityRequest,
) -> Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError> {
    let requested_page_id = pages_request_page_id(&request)?.to_string();
    ensure_requested_page(&snapshot, &requested_page_id)?;
    let expected_capability = request.capability();
    let published_project = match &request {
        PageBuilderCapabilityRequest::Publish(input) => Some(input.project_data.clone()),
        PageBuilderCapabilityRequest::Preview(_) => None,
        request => return Err(unsupported_request_error(request)),
    };
    let token = snapshot.token.clone();
    let tenant_slug = snapshot.tenant_slug.clone();
    let response = pages_page_builder_capability(
        token.clone(),
        tenant_slug.clone(),
        snapshot.page_id.clone(),
        snapshot.default_locale,
        request,
    )
    .await
    .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?;
    if response.capability() != expected_capability {
        return Err(PageBuilderAdminFacadeError::new(format!(
            "Page Builder capability endpoint returned `{}` for a `{expected_capability}` request",
            response.capability()
        )));
    }
    if let Some(project_data) = published_project {
        let saved_page = transport::fetch_page(token, tenant_slug, requested_page_id)
            .await
            .map_err(facade_transport_error)?
            .ok_or_else(|| {
                PageBuilderAdminFacadeError::new(
                    "Pages document was not found after Page Builder publish",
                )
            })?;
        on_saved(PageMutationResult::from(&saved_page), project_data);
    }
    Ok(response)
}

fn page_locale(page: &PageDetail, default_locale: String) -> String {
    page.body
        .as_ref()
        .map(|body| body.locale.clone())
        .or_else(|| {
            page.translation
                .as_ref()
                .map(|translation| translation.locale.clone())
        })
        .unwrap_or(default_locale)
}

fn persisted_revision(page: &PageDetail) -> Result<String, PageBuilderAdminFacadeError> {
    let revision = page_revision(page);
    if revision.starts_with("page:") {
        Err(PageBuilderAdminFacadeError::new(
            "Pages document save succeeded without a persisted body revision",
        ))
    } else {
        Ok(revision)
    }
}

#[allow(dead_code)]
fn facade_transport_error(error: transport::TransportError) -> PageBuilderAdminFacadeError {
    PageBuilderAdminFacadeError::new(error.to_string())
}

#[cfg(feature = "ssr")]
fn required_snapshot_value(
    value: Option<String>,
    label: &str,
) -> Result<String, PageBuilderAdminFacadeError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| PageBuilderAdminFacadeError::new(format!("Pages {label} is missing")))
}

#[cfg(feature = "ssr")]
fn page_builder_permissions_for_role(role: &str) -> Vec<Permission> {
    let capabilities = crate::access::pages_editor_permissions_for_role(Some(role));
    let mut permissions = vec![Permission::new(Resource::Pages, Action::Read)];
    if capabilities.publish {
        permissions.push(Permission::new(Resource::Pages, Action::Publish));
    }
    permissions
}

#[cfg(feature = "ssr")]
fn facade_service_error(error: PageBuilderServiceError) -> PageBuilderAdminFacadeError {
    let message = error.to_string();
    for code in [PAGE_PUBLISHED_DOCUMENT_IMMUTABLE, REVISION_CONFLICT] {
        if message.contains(code) {
            return PageBuilderAdminFacadeError::with_stable_code(message, code);
        }
    }
    PageBuilderAdminFacadeError::new(message)
}

#[cfg(feature = "ssr")]
#[derive(Clone)]
struct PagesPageBuilderProjectStore {
    token: String,
    tenant_slug: String,
    actor_id: String,
    page_id: String,
    default_locale: String,
    on_saved: SavedHandler,
}

#[cfg(feature = "ssr")]
impl PagesPageBuilderProjectStore {
    fn ensure_context(&self, context: &PortContext) -> PageBuilderServiceResult<()> {
        ensure_port_context(context, &self.tenant_slug, &self.actor_id, "store")
    }

    fn ensure_page_id(&self, page_id: &str) -> PageBuilderServiceResult<()> {
        if page_id == self.page_id {
            Ok(())
        } else {
            Err(PageBuilderServiceError::Validation(format!(
                "Page Builder requested page `{page_id}`, but Pages store owns `{}`",
                self.page_id
            )))
        }
    }
}

#[cfg(feature = "ssr")]
#[async_trait]
impl PageBuilderProjectStore for PagesPageBuilderProjectStore {
    async fn load_project(
        &self,
        context: &PortContext,
        page_id: &str,
    ) -> PageBuilderServiceResult<Option<Value>> {
        self.ensure_context(context)?;
        self.ensure_page_id(page_id)?;
        let page = transport::fetch_page(
            Some(self.token.clone()),
            Some(self.tenant_slug.clone()),
            page_id.to_string(),
        )
        .await
        .map_err(|error| PageBuilderServiceError::Runtime(error.to_string()))?;
        page.map(|page| {
            let seed = crate::core::edit_form_seed_from_page(&page, &self.default_locale);
            let project = crate::core::parse_project_data(&seed.project_data_text)
                .map_err(PageBuilderServiceError::Validation)?;
            canonicalize_builder_project(project)
                .map_err(|error| PageBuilderServiceError::Validation(error.to_string()))
        })
        .transpose()
    }

    async fn save_project(
        &self,
        context: &PortContext,
        page_id: &str,
        revision_id: &str,
        project_data: Value,
    ) -> PageBuilderServiceResult<PageBuilderProjectSaveResult> {
        self.ensure_context(context)?;
        self.ensure_page_id(page_id)?;
        let current_page = transport::fetch_page(
            Some(self.token.clone()),
            Some(self.tenant_slug.clone()),
            page_id.to_string(),
        )
        .await
        .map_err(|error| PageBuilderServiceError::Runtime(error.to_string()))?
        .ok_or_else(|| {
            PageBuilderServiceError::Runtime("Pages document no longer exists".into())
        })?;
        if current_page.status.eq_ignore_ascii_case("published") {
            return Err(PageBuilderServiceError::Validation(format!(
                "{PAGE_PUBLISHED_DOCUMENT_IMMUTABLE}: published page documents are immutable"
            )));
        }
        let current_revision = page_revision(&current_page);
        if revision_id != current_revision {
            return Err(PageBuilderServiceError::Validation(format!(
                "{REVISION_CONFLICT}: expected `{revision_id}`, current `{current_revision}`"
            )));
        }

        let project_data = canonicalize_builder_project(project_data)
            .map_err(|error| PageBuilderServiceError::Validation(error.to_string()))?;
        let locale = page_locale(&current_page, self.default_locale.clone());
        let saved_page = transport::save_page_document(
            Some(self.token.clone()),
            Some(self.tenant_slug.clone()),
            page_id.to_string(),
            current_revision,
            locale,
            project_data.clone(),
        )
        .await
        .map_err(|error| PageBuilderServiceError::Runtime(error.to_string()))?;
        let revision_id = persisted_revision(&saved_page)
            .map_err(|error| PageBuilderServiceError::Runtime(error.to_string()))?;
        let result = PageBuilderProjectSaveResult {
            page_id: saved_page.id.clone(),
            revision_id,
            published: saved_page.status.eq_ignore_ascii_case("published"),
        };
        (self.on_saved)(PageMutationResult::from(&saved_page), project_data);
        Ok(result)
    }
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone)]
struct PagesPageBuilderRenderer {
    token: String,
    tenant_slug: String,
    actor_id: String,
    page_id: String,
}

#[cfg(feature = "ssr")]
#[async_trait]
impl PageBuilderPreviewRenderingPort for PagesPageBuilderRenderer {
    async fn render_preview(
        &self,
        context: &PortContext,
        input: &PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<String> {
        ensure_port_context(context, &self.tenant_slug, &self.actor_id, "renderer")?;
        if input.page_id != self.page_id {
            return Err(PageBuilderServiceError::Validation(format!(
                "Page Builder preview requested page `{}`, but Pages renderer owns `{}`",
                input.page_id, self.page_id
            )));
        }
        transport::fetch_page(
            Some(self.token.clone()),
            Some(self.tenant_slug.clone()),
            self.page_id.clone(),
        )
        .await
        .map_err(|error| PageBuilderServiceError::Runtime(error.to_string()))?
        .ok_or_else(|| {
            PageBuilderServiceError::Runtime("Pages document no longer exists".into())
        })?;
        PageBuilderRenderer
            .render_runtime_document_html(
                input.project_data.clone(),
                PageSelection::First,
                RenderPolicy::default(),
                input.runtime.context.clone(),
            )
            .map_err(|error| PageBuilderServiceError::Runtime(error.to_string()))
    }
}

#[cfg(feature = "ssr")]
fn ensure_port_context(
    context: &PortContext,
    tenant_slug: &str,
    actor_id: &str,
    port: &str,
) -> PageBuilderServiceResult<()> {
    if context.tenant_id.as_str() != tenant_slug {
        return Err(PageBuilderServiceError::Forbidden(format!(
            "Page Builder context tenant `{}` does not match Pages {port} tenant `{tenant_slug}`",
            context.tenant_id
        )));
    }
    if context.actor.id.as_str() != actor_id {
        return Err(PageBuilderServiceError::Forbidden(format!(
            "Page Builder context actor `{}` does not match verified Pages {port} actor `{actor_id}`",
            context.actor.id
        )));
    }
    Ok(())
}

pub fn controller_from_project(
    page_id: &str,
    revision_id: &str,
    raw_project: &str,
) -> Result<AdminCanvasController, PageBuilderAdminFacadeError> {
    let project =
        crate::core::parse_project_data(raw_project).map_err(PageBuilderAdminFacadeError::new)?;
    let project = canonicalize_builder_project(project)?;
    AdminCanvasController::new(page_id, revision_id, project)
        .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))
}

pub fn page_revision(page: &PageDetail) -> String {
    page.body
        .as_ref()
        .map(|body| body.updated_at.clone())
        .filter(|revision| !revision.trim().is_empty())
        .unwrap_or_else(|| format!("page:{}:initial", page.id))
}

pub fn canonicalize_builder_project(
    mut project: Value,
) -> Result<Value, PageBuilderAdminFacadeError> {
    let project_object = project.as_object_mut().ok_or_else(|| {
        PageBuilderAdminFacadeError::new("Page Builder project root must be an object")
    })?;
    let pages = project_object
        .entry("pages".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let pages = pages.as_array_mut().ok_or_else(|| {
        PageBuilderAdminFacadeError::new("Page Builder project `pages` must be an array")
    })?;
    if pages.is_empty() {
        pages.push(default_page());
    }

    for (page_index, page) in pages.iter_mut().enumerate() {
        let page = page.as_object_mut().ok_or_else(|| {
            PageBuilderAdminFacadeError::new(format!(
                "Page Builder page at index {page_index} must be an object"
            ))
        })?;
        if page.get("component").is_none_or(Value::is_null) {
            page.insert("component".to_string(), default_root_component());
        }
        if page.get("id").is_none_or(Value::is_null) {
            page.insert(
                "id".to_string(),
                Value::String(format!("page-{page_index}")),
            );
        }
    }

    Ok(project)
}

fn default_page() -> Value {
    json!({
        "id": "main",
        "component": default_root_component()
    })
}

fn default_root_component() -> Value {
    json!({
        "id": "root",
        "type": "wrapper",
        "components": []
    })
}
