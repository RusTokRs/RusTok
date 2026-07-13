use crate::core::{self, PageDraftFormInput, GRAPESJS_FORMAT};
use crate::model::PageMutationResult;
use crate::transport;
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderCapabilityResponse, PublishPageBuilderResult,
};
use rustok_page_builder_admin::{
    AdminCanvasController, PageBuilderAdminFacade, PageBuilderAdminFacadeError,
    PageBuilderAdminFacadeFuture,
};
use serde_json::{json, Map, Value};
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagesBuilderSaveSnapshot {
    pub token: Option<String>,
    pub tenant_slug: Option<String>,
    pub page_id: String,
    pub locale: String,
    pub title: String,
    pub slug: String,
    pub channel_slugs: String,
    pub publish: bool,
}

type SnapshotProvider = Rc<dyn Fn() -> PagesBuilderSaveSnapshot>;
type SavedHandler = Rc<dyn Fn(PageMutationResult, Value)>;

#[derive(Clone)]
pub struct PagesBuilderFacade {
    snapshot: SnapshotProvider,
    on_saved: SavedHandler,
}

impl PagesBuilderFacade {
    pub fn new(
        snapshot: impl Fn() -> PagesBuilderSaveSnapshot + 'static,
        on_saved: impl Fn(PageMutationResult, Value) + 'static,
    ) -> Self {
        Self {
            snapshot: Rc::new(snapshot),
            on_saved: Rc::new(on_saved),
        }
    }
}

impl PageBuilderAdminFacade for PagesBuilderFacade {
    fn execute(&self, request: PageBuilderCapabilityRequest) -> PageBuilderAdminFacadeFuture {
        let snapshot = (self.snapshot)();
        let on_saved = Rc::clone(&self.on_saved);
        Box::pin(async move {
            let PageBuilderCapabilityRequest::Publish(input) = request else {
                return Err(PageBuilderAdminFacadeError::new(
                    "Pages consumer facade currently accepts only Page Builder publish requests",
                ));
            };
            if input.page_id != snapshot.page_id {
                return Err(PageBuilderAdminFacadeError::new(format!(
                    "Page Builder requested page `{}`, but Pages is editing `{}`",
                    input.page_id, snapshot.page_id
                )));
            }
            if input.schema_version != GRAPESJS_FORMAT {
                return Err(PageBuilderAdminFacadeError::new(format!(
                    "Unsupported Page Builder schema `{}`; expected `{GRAPESJS_FORMAT}`",
                    input.schema_version
                )));
            }

            let project_data = canonicalize_builder_project(input.project_data)?;
            let draft = core::build_create_page_draft(
                PageDraftFormInput {
                    locale: &snapshot.locale,
                    title: &snapshot.title,
                    slug: &snapshot.slug,
                    channel_slugs: &snapshot.channel_slugs,
                    publish: snapshot.publish,
                },
                project_data.clone(),
            );
            if let Some(field) = core::missing_required_page_field(&draft) {
                return Err(PageBuilderAdminFacadeError::new(format!(
                    "Page Builder save requires Pages metadata field `{field:?}`"
                )));
            }

            let page = transport::update_page(
                snapshot.token,
                snapshot.tenant_slug,
                snapshot.page_id,
                draft,
            )
            .await
            .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))?;
            let response = PublishPageBuilderResult {
                page_id: page.id.clone(),
                revision_id: page.updated_at.clone(),
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
    let project = core::parse_project_data(raw_project)
        .map_err(PageBuilderAdminFacadeError::new)?;
    let project = canonicalize_builder_project(project)?;
    AdminCanvasController::new(page_id, revision_id, project)
        .map_err(|error| PageBuilderAdminFacadeError::new(error.to_string()))
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
            let component = take_frame_component(page).unwrap_or_else(default_root_component);
            page.insert("component".to_string(), component);
        }
        if page.get("id").is_none_or(Value::is_null) {
            page.insert("id".to_string(), Value::String(format!("page-{page_index}")));
        }
    }

    Ok(project)
}

fn take_frame_component(page: &mut Map<String, Value>) -> Option<Value> {
    page.get_mut("frames")
        .and_then(Value::as_array_mut)
        .and_then(|frames| frames.first_mut())
        .and_then(Value::as_object_mut)
        .and_then(|frame| frame.remove("component"))
}

fn default_page() -> Value {
    json!({
        "id": "main",
        "component": default_root_component(),
        "frames": []
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
    fn canonicalization_moves_legacy_frame_component_to_page_root() {
        let project = canonicalize_builder_project(json!({
            "pages": [{
                "id": "home",
                "frames": [{
                    "id": "frame-home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": []
                    }
                }]
            }]
        }))
        .expect("canonical project");

        assert_eq!(project["pages"][0]["component"]["id"], "root");
        assert!(project["pages"][0]["frames"][0]
            .get("component")
            .is_none());
        assert_eq!(project["pages"][0]["frames"][0]["id"], "frame-home");
    }

    #[test]
    fn empty_project_receives_editable_root() {
        let project = canonicalize_builder_project(json!({})).expect("canonical project");
        assert_eq!(project["pages"][0]["component"]["id"], "root");
    }
}
