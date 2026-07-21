#[cfg(test)]
use crate::core::GRAPESJS_FORMAT;
use crate::core::{self, PageDraftFormInput};
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

            let seed = core::edit_form_seed_from_page(&current_page, &snapshot.default_locale);
            let project_data = canonicalize_builder_project(input.project_data)?;
            let draft = core::build_create_page_draft(
                PageDraftFormInput {
                    locale: &seed.locale,
                    title: &seed.title,
                    slug: &seed.slug,
                    channel_slugs: &seed.channel_slugs_text,
                    publish: seed.publish_now,
                },
                project_data.clone(),
            );
            if let Some(field) = core::missing_required_page_field(&draft) {
                return Err(PageBuilderAdminFacadeError::new(format!(
                    "Page Builder save requires Pages metadata field `{field:?}`"
                )));
            }

            let page = transport::update_page(
                snapshot.token.clone(),
                snapshot.tenant_slug.clone(),
                snapshot.page_id.clone(),
                draft,
            )
            .await
            .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?;
            let persisted_page =
                transport::fetch_page(snapshot.token, snapshot.tenant_slug, snapshot.page_id)
                    .await
                    .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?
                    .ok_or_else(|| {
                        PageBuilderAdminFacadeError::new(
                            "Pages document disappeared after a successful builder save",
                        )
                    })?;
            let persisted_revision = page_revision(&persisted_page);
            if persisted_revision.starts_with("page:") {
                return Err(PageBuilderAdminFacadeError::new(
                    "Pages builder save succeeded but no persisted body revision was returned",
                ));
            }

            let response = PublishPageBuilderResult {
                page_id: page.id.clone(),
                revision_id: persisted_revision,
                published: page.status.eq_ignore_ascii_case("published"),
            };
            on_saved(page, project_data);
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
        core::parse_project_data(raw_project).map_err(PageBuilderAdminFacadeError::new)?;
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

/// Normalizes only the current Fly document contract.
///
/// `pages[].component` is the sole component-tree authority. Historical frame
/// mirrors are not imported, synchronized or generated. Unknown project and
/// page fields remain untouched for forward-compatible codecs/providers.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_component_is_the_only_tree_authority() {
        let project = canonicalize_builder_project(json!({
            "providerMetadata": { "version": 3 },
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "current", "type": "section" }]
                },
                "pluginData": { "future": true }
            }]
        }))
        .expect("canonical project");

        assert_eq!(
            project["pages"][0]["component"]["components"][0]["id"],
            "current"
        );
        assert_eq!(project["pages"][0]["pluginData"]["future"], true);
        assert_eq!(project["providerMetadata"]["version"], 3);
        assert!(project["pages"][0].get("frames").is_none());
    }

    #[test]
    fn historical_frame_tree_is_not_imported() {
        let project = canonicalize_builder_project(json!({
            "pages": [{
                "id": "home",
                "frames": [{
                    "component": {
                        "id": "old-root",
                        "type": "wrapper",
                        "components": [{ "id": "old", "type": "section" }]
                    }
                }]
            }]
        }))
        .expect("canonical project");

        assert_eq!(project["pages"][0]["component"]["id"], "root");
        assert_eq!(
            project["pages"][0]["frames"][0]["component"]["id"],
            "old-root"
        );
    }

    #[test]
    fn empty_project_receives_an_editable_current_root() {
        let project = canonicalize_builder_project(json!({})).expect("canonical project");
        assert_eq!(project["pages"][0]["component"]["id"], "root");
        assert!(project["pages"][0].get("frames").is_none());
    }

    #[test]
    fn page_revision_uses_body_timestamp_or_stable_initial_marker() {
        let mut page = PageDetail {
            id: "home".to_string(),
            status: "draft".to_string(),
            template: "default".to_string(),
            channel_slugs: Vec::new(),
            translation: None,
            body: None,
        };
        assert_eq!(page_revision(&page), "page:home:initial");
        page.body = Some(crate::model::PageBody {
            locale: "en".to_string(),
            content: String::new(),
            format: GRAPESJS_FORMAT.to_string(),
            content_json: None,
            updated_at: "rev-2".to_string(),
        });
        assert_eq!(page_revision(&page), "rev-2");
    }
}
