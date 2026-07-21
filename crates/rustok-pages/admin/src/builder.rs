use crate::model::{PageDetail, PageMutationResult};
use crate::transport;
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderCapabilityResponse, PublishPageBuilderResult,
};
use rustok_page_builder_admin::{
    AdminCanvasController, PageBuilderAdminFacade, PageBuilderAdminFacadeError,
    PageBuilderAdminFacadeFuture,
};
use serde_json::{Value, json};
use std::sync::Arc;

const PAGE_PUBLISHED_DOCUMENT_IMMUTABLE: &str = "PAGE_PUBLISHED_DOCUMENT_IMMUTABLE";

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
            let PageBuilderCapabilityRequest::Publish(input) = request else {
                return Err(PageBuilderAdminFacadeError::new(
                    "Pages consumer facade accepts only Page Builder publish requests",
                ));
            };
            if input.page_id != snapshot.page_id {
                return Err(PageBuilderAdminFacadeError::new(format!(
                    "Page Builder requested page `{}`, but Pages is editing `{}`",
                    input.page_id, snapshot.page_id
                )));
            }

            let current_page = transport::fetch_page(
                snapshot.token.clone(),
                snapshot.tenant_slug.clone(),
                snapshot.page_id.clone(),
            )
            .await
            .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?
            .ok_or_else(|| PageBuilderAdminFacadeError::new("Pages document no longer exists"))?;
            if current_page.status.eq_ignore_ascii_case("published") {
                return Err(PageBuilderAdminFacadeError::with_stable_code(
                    "Published page documents are immutable. Unpublish the page before editing, then publish the new revision explicitly.",
                    PAGE_PUBLISHED_DOCUMENT_IMMUTABLE,
                ));
            }

            let current_revision = page_revision(&current_page);
            if input.revision_id != current_revision {
                return Err(PageBuilderAdminFacadeError::with_stable_code(
                    format!(
                        "Page Builder revision conflict: expected `{}`, current `{current_revision}`",
                        input.revision_id
                    ),
                    "REVISION_CONFLICT",
                ));
            }

            let project_data = canonicalize_builder_project(input.project_data)?;
            let locale = current_page
                .body
                .as_ref()
                .map(|body| body.locale.clone())
                .or_else(|| {
                    current_page
                        .translation
                        .as_ref()
                        .map(|translation| translation.locale.clone())
                })
                .unwrap_or(snapshot.default_locale);
            let saved_page = transport::save_page_document(
                snapshot.token,
                snapshot.tenant_slug,
                snapshot.page_id,
                current_revision,
                locale,
                project_data.clone(),
            )
            .await
            .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?;
            let persisted_revision = page_revision(&saved_page);
            if persisted_revision.starts_with("page:") {
                return Err(PageBuilderAdminFacadeError::new(
                    "Pages document save succeeded without a persisted body revision",
                ));
            }

            let response = PublishPageBuilderResult {
                page_id: saved_page.id.clone(),
                revision_id: persisted_revision,
                published: saved_page.status.eq_ignore_ascii_case("published"),
            };
            on_saved(PageMutationResult::from(&saved_page), project_data);
            Ok(PageBuilderCapabilityResponse::Publish(response))
        })
    }
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
