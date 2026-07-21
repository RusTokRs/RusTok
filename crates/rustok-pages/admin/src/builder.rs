use crate::model::{PageDetail, PageMutationResult};
use crate::transport;
use leptos::prelude::*;
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderCapabilityResponse, PreviewPageBuilderInput,
    PublishPageBuilderInput, PublishPageBuilderResult,
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
use rustok_page_builder::render::PageBuilderRenderer;
#[cfg(feature = "ssr")]
use rustok_page_builder::rollout::BuilderCapabilityFlags;
#[cfg(feature = "ssr")]
use rustok_page_builder::service::{
    PageBuilderProjectSaveResult, PageBuilderProjectStore, PageBuilderRenderingAdapter,
    PageBuilderRequestAuth, PageBuilderServiceError, PageBuilderServiceResult,
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
                    ensure_requested_page(&snapshot, &input.page_id)?;
                    execute_preview(snapshot, input).await
                }
                PageBuilderCapabilityRequest::Publish(input) => {
                    ensure_requested_page(&snapshot, &input.page_id)?;
                    execute_publish(snapshot, on_saved, input).await
                }
                request => Err(PageBuilderAdminFacadeError::new(format!(
                    "Pages consumer facade does not support Page Builder `{}` requests",
                    request.capability()
                ))),
            }
        })
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

#[server(prefix = "/api/fn", endpoint = "pages/page-builder-preview")]
async fn pages_page_builder_preview(
    token: Option<String>,
    tenant_slug: Option<String>,
    page_id: String,
    default_locale: String,
    project_data: Value,
) -> Result<PageBuilderCapabilityResponse, ServerFnError> {
    execute_preview(
        PagesBuilderSaveSnapshot {
            token,
            tenant_slug,
            page_id: page_id.clone(),
            default_locale,
        },
        PreviewPageBuilderInput::new(page_id, project_data),
    )
    .await
    .map_err(|error| ServerFnError::ServerError(error.to_string()))
}

#[cfg(feature = "ssr")]
async fn execute_preview(
    snapshot: PagesBuilderSaveSnapshot,
    input: PreviewPageBuilderInput,
) -> Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError> {
    let token = required_snapshot_value(snapshot.token, "access token")?;
    let tenant_slug = required_snapshot_value(snapshot.tenant_slug, "tenant")?;
    let verified_user = leptos_auth::api::fetch_current_user(token.clone(), tenant_slug.clone())
        .await
        .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?
        .ok_or_else(|| PageBuilderAdminFacadeError::new("Authenticated user was not found"))?;
    let permissions = page_builder_permissions_for_role(&verified_user.role);
    let actor_id = verified_user.id;
    let auth = PageBuilderRequestAuth::new(permissions);
    let context = PortContext::new(
        tenant_slug.clone(),
        PortActor::user(actor_id.clone()),
        snapshot.default_locale.clone(),
        format!("page-builder-preview:{}", input.page_id),
    )
    .with_deadline(PAGE_BUILDER_PORT_DEADLINE);

    let renderer = PagesPageBuilderRenderer {
        tenant_slug: tenant_slug.clone(),
        actor_id: actor_id.clone(),
    };
    let store = PagesPageBuilderProjectStore {
        token,
        tenant_slug,
        actor_id,
        page_id: snapshot.page_id,
        default_locale: snapshot.default_locale,
        on_saved: Arc::new(|_, _| {}),
    };
    let handlers = compose_fly_page_builder_handlers(
        store,
        renderer,
        BuilderCapabilityFlags::default(),
    )
    .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?;
    match handlers
        .handle(
            &context,
            &auth,
            PageBuilderCapabilityRequest::Preview(input),
        )
        .await
        .map_err(facade_service_error)?
    {
        response @ PageBuilderCapabilityResponse::Preview(_) => Ok(response),
        _ => Err(PageBuilderAdminFacadeError::new(
            "Page Builder composition returned an unexpected preview response",
        )),
    }
}

#[cfg(not(feature = "ssr"))]
async fn execute_preview(
    snapshot: PagesBuilderSaveSnapshot,
    input: PreviewPageBuilderInput,
) -> Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError> {
    let requested_page_id = input.page_id.clone();
    let response = pages_page_builder_preview(
        snapshot.token,
        snapshot.tenant_slug,
        snapshot.page_id,
        snapshot.default_locale,
        input.project_data,
    )
    .await
    .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?;
    match response {
        PageBuilderCapabilityResponse::Preview(result) if result.page_id == requested_page_id => {
            Ok(PageBuilderCapabilityResponse::Preview(result))
        }
        PageBuilderCapabilityResponse::Preview(result) => Err(PageBuilderAdminFacadeError::new(
            format!(
                "Page Builder preview returned page `{}`, but Pages requested `{requested_page_id}`",
                result.page_id
            ),
        )),
        response => Err(PageBuilderAdminFacadeError::new(format!(
            "Page Builder preview transport returned `{}`",
            response.capability()
        ))),
    }
}

#[cfg(feature = "ssr")]
async fn execute_publish(
    snapshot: PagesBuilderSaveSnapshot,
    on_saved: SavedHandler,
    input: PublishPageBuilderInput,
) -> Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError> {
    let token = required_snapshot_value(snapshot.token, "access token")?;
    let tenant_slug = required_snapshot_value(snapshot.tenant_slug, "tenant")?;
    let verified_user = leptos_auth::api::fetch_current_user(token.clone(), tenant_slug.clone())
        .await
        .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?
        .ok_or_else(|| PageBuilderAdminFacadeError::new("Authenticated user was not found"))?;
    let permissions = page_builder_permissions_for_role(&verified_user.role);
    let actor_id = verified_user.id;
    let auth = PageBuilderRequestAuth::new(permissions);
    let context = PortContext::new(
        tenant_slug.clone(),
        PortActor::user(actor_id.clone()),
        snapshot.default_locale.clone(),
        format!("page-builder:{}:{}", input.page_id, input.revision_id),
    )
    .with_deadline(PAGE_BUILDER_PORT_DEADLINE)
    .with_idempotency_key(format!(
        "page-builder-save:{}:{}",
        input.page_id, input.revision_id
    ));

    let renderer = PagesPageBuilderRenderer {
        tenant_slug: tenant_slug.clone(),
        actor_id: actor_id.clone(),
    };
    let store = PagesPageBuilderProjectStore {
        token,
        tenant_slug,
        actor_id,
        page_id: snapshot.page_id,
        default_locale: snapshot.default_locale,
        on_saved,
    };
    let handlers = compose_fly_page_builder_handlers(
        store,
        renderer,
        BuilderCapabilityFlags::default(),
    )
    .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?;
    match handlers
        .handle(
            &context,
            &auth,
            PageBuilderCapabilityRequest::Publish(input),
        )
        .await
        .map_err(facade_service_error)?
    {
        response @ PageBuilderCapabilityResponse::Publish(_) => Ok(response),
        _ => Err(PageBuilderAdminFacadeError::new(
            "Page Builder composition returned an unexpected capability response",
        )),
    }
}

#[cfg(not(feature = "ssr"))]
async fn execute_publish(
    snapshot: PagesBuilderSaveSnapshot,
    on_saved: SavedHandler,
    input: PublishPageBuilderInput,
) -> Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError> {
    let current_page = transport::fetch_page(
        snapshot.token.clone(),
        snapshot.tenant_slug.clone(),
        snapshot.page_id.clone(),
    )
    .await
    .map_err(facade_transport_error)?
    .ok_or_else(|| PageBuilderAdminFacadeError::new("Pages document no longer exists"))?;
    ensure_page_is_editable(&current_page, &input.revision_id)?;

    let current_revision = page_revision(&current_page);
    let project_data = canonicalize_builder_project(input.project_data)?;
    let locale = page_locale(&current_page, snapshot.default_locale);
    let saved_page = transport::save_page_document(
        snapshot.token,
        snapshot.tenant_slug,
        snapshot.page_id,
        current_revision,
        locale,
        project_data.clone(),
    )
    .await
    .map_err(facade_transport_error)?;
    let persisted_revision = persisted_revision(&saved_page)?;
    on_saved(PageMutationResult::from(&saved_page), project_data);
    Ok(PageBuilderCapabilityResponse::Publish(
        PublishPageBuilderResult {
            page_id: saved_page.id.clone(),
            revision_id: persisted_revision,
            published: saved_page.status.eq_ignore_ascii_case("published"),
        },
    ))
}

fn ensure_page_is_editable(
    page: &PageDetail,
    requested_revision: &str,
) -> Result<(), PageBuilderAdminFacadeError> {
    if page.status.eq_ignore_ascii_case("published") {
        return Err(PageBuilderAdminFacadeError::with_stable_code(
            "Published page documents are immutable. Unpublish the page before editing, then publish the new revision explicitly.",
            PAGE_PUBLISHED_DOCUMENT_IMMUTABLE,
        ));
    }
    let current_revision = page_revision(page);
    if requested_revision != current_revision {
        return Err(PageBuilderAdminFacadeError::with_stable_code(
            format!(
                "Page Builder revision conflict: expected `{requested_revision}`, current `{current_revision}`"
            ),
            REVISION_CONFLICT,
        ));
    }
    Ok(())
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
        .ok_or_else(|| PageBuilderServiceError::Runtime("Pages document no longer exists".into()))?;
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
    tenant_slug: String,
    actor_id: String,
}

#[cfg(feature = "ssr")]
#[async_trait]
impl PageBuilderRenderingAdapter for PagesPageBuilderRenderer {
    async fn render_preview(
        &self,
        context: &PortContext,
        project_data: &Value,
    ) -> PageBuilderServiceResult<String> {
        ensure_port_context(context, &self.tenant_slug, &self.actor_id, "renderer")?;
        PageBuilderRenderer
            .render_document_html(
                project_data.clone(),
                PageSelection::First,
                RenderPolicy::default(),
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
