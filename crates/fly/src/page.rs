use crate::{
    ComponentNode, ComponentObject, FlyError, FlyResult, ProjectDocument, ProjectPage,
    StyleRuleDescriptor,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PageLocator {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

impl PageLocator {
    pub fn by_id(id: impl Into<String>) -> Self {
        Self {
            id: Some(id.into()),
            index: None,
        }
    }

    pub const fn by_index(index: usize) -> Self {
        Self {
            id: None,
            index: Some(index),
        }
    }

    pub fn resolve(&self, document: &ProjectDocument) -> FlyResult<usize> {
        if let Some(id) = self.id.as_deref() {
            return document
                .project
                .pages
                .iter()
                .position(|page| page.id.as_deref() == Some(id))
                .ok_or_else(|| FlyError::PageNotFound(id.to_string()));
        }
        if let Some(index) = self.index {
            return (index < document.project.pages.len())
                .then_some(index)
                .ok_or_else(|| FlyError::PageNotFound(index.to_string()));
        }
        Err(FlyError::InvalidPageLocator)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageSummary {
    pub index: usize,
    pub id: Option<String>,
    pub name: String,
    pub component_count: usize,
    pub has_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PagePatch {
    #[serde(default)]
    pub fields: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PageCommand {
    Add {
        index: usize,
        page: Box<ProjectPage>,
    },
    Remove {
        locator: PageLocator,
    },
    Move {
        locator: PageLocator,
        index: usize,
    },
    Patch {
        locator: PageLocator,
        patch: PagePatch,
    },
}

impl ProjectDocument {
    pub fn page_summaries(&self) -> Vec<PageSummary> {
        self.project
            .pages
            .iter()
            .enumerate()
            .map(|(index, page)| page_summary(index, page))
            .collect()
    }

    pub fn page(&self, locator: &PageLocator) -> FlyResult<&ProjectPage> {
        let index = locator.resolve(self)?;
        self.project
            .pages
            .get(index)
            .ok_or_else(|| FlyError::PageNotFound(index.to_string()))
    }

    pub fn page_mut(&mut self, locator: &PageLocator) -> FlyResult<&mut ProjectPage> {
        let index = locator.resolve(self)?;
        self.project
            .pages
            .get_mut(index)
            .ok_or_else(|| FlyError::PageNotFound(index.to_string()))
    }
}

pub fn blank_page(id: impl Into<String>, name: impl Into<String>) -> ProjectPage {
    let id = id.into();
    let name = name.into();
    ProjectPage {
        id: (!id.trim().is_empty()).then_some(id),
        component: Some(ComponentNode::Object(Box::new(ComponentObject {
            component_type: Some("wrapper".to_string()),
            ..ComponentObject::default()
        }))),
        frames: None,
        extensions: Map::from_iter([("name".to_string(), Value::String(name))]),
    }
}

pub fn apply_page_command(document: &mut ProjectDocument, command: &PageCommand) -> FlyResult<()> {
    match command {
        PageCommand::Add { index, page } => {
            if *index > document.project.pages.len() {
                return Err(FlyError::InvalidPageIndex {
                    index: *index,
                    len: document.project.pages.len(),
                });
            }
            ensure_page_id_available(document, page.id.as_deref(), None)?;
            document.project.pages.insert(*index, page.as_ref().clone());
            Ok(())
        }
        PageCommand::Remove { locator } => {
            let index = locator.resolve(document)?;
            if document.project.pages.len() <= 1 {
                return Err(FlyError::LastPageRemoval);
            }
            let page = document.project.pages.remove(index);
            remove_known_page_style_rules(document, &page);
            Ok(())
        }
        PageCommand::Move { locator, index } => {
            let source_index = locator.resolve(document)?;
            let len = document.project.pages.len();
            if *index > len {
                return Err(FlyError::InvalidPageIndex { index: *index, len });
            }
            let mut target_index = *index;
            if source_index < target_index {
                target_index = target_index.saturating_sub(1);
            }
            let page = document.project.pages.remove(source_index);
            document.project.pages.insert(target_index, page);
            Ok(())
        }
        PageCommand::Patch { locator, patch } => {
            let index = locator.resolve(document)?;
            let proposed_id = patch
                .fields
                .get("id")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            if proposed_id.is_some() {
                ensure_page_id_available(document, proposed_id.as_deref(), Some(index))?;
            }
            let page = document
                .project
                .pages
                .get_mut(index)
                .ok_or_else(|| FlyError::PageNotFound(index.to_string()))?;
            apply_page_patch(page, patch.clone());
            Ok(())
        }
    }
}

fn page_summary(index: usize, page: &ProjectPage) -> PageSummary {
    let mut component_count = 0usize;
    if let Some(root) = page.component.as_ref() {
        root.visit(0, "page.component", &mut |_, _, _| {
            component_count = component_count.saturating_add(1)
        });
    }
    let name = page
        .extensions
        .get("name")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| page.id.clone())
        .unwrap_or_else(|| format!("Page {}", index + 1));
    PageSummary {
        index,
        id: page.id.clone(),
        name,
        component_count,
        has_root: page.component.is_some(),
    }
}

fn apply_page_patch(page: &mut ProjectPage, patch: PagePatch) {
    for field in patch.remove_fields {
        match field.as_str() {
            "id" => page.id = None,
            "frames" => page.frames = None,
            _ => {
                page.extensions.remove(&field);
            }
        }
    }
    for (field, value) in patch.fields {
        match field.as_str() {
            "id" => page.id = value.as_str().map(ToString::to_string),
            "frames" => page.frames = Some(value),
            _ => {
                page.extensions.insert(field, value);
            }
        }
    }
}

fn ensure_page_id_available(
    document: &ProjectDocument,
    id: Option<&str>,
    except_index: Option<usize>,
) -> FlyResult<()> {
    let Some(id) = id.filter(|id| !id.trim().is_empty()) else {
        return Ok(());
    };
    if document
        .project
        .pages
        .iter()
        .enumerate()
        .any(|(index, page)| {
            index != except_index.unwrap_or(usize::MAX) && page.id.as_deref() == Some(id)
        })
    {
        return Err(FlyError::DuplicatePageId(id.to_string()));
    }
    Ok(())
}

fn remove_known_page_style_rules(document: &mut ProjectDocument, page: &ProjectPage) {
    let mut component_ids = Vec::new();
    if let Some(root) = page.component.as_ref() {
        root.collect_ids(&mut component_ids);
    }
    let component_ids = component_ids.into_iter().collect::<BTreeSet<_>>();
    document.project.styles.retain(|raw| {
        StyleRuleDescriptor::from_value(raw.clone()).is_none_or(|rule| {
            rule.component_id
                .as_ref()
                .is_none_or(|component_id| !component_ids.contains(component_id))
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GrapesJsCodec, StyleRuleCatalog, StyleRuleScope};
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "styles": [{
                "selectors": [{ "name": "hero-a", "type": 2 }],
                "style": { "padding": "24px" },
                "flyComponentId": "hero-a"
            }],
            "pages": [{
                "id": "a",
                "name": "Page A",
                "component": {
                    "id": "root-a",
                    "type": "wrapper",
                    "components": [{ "id": "hero-a", "type": "section" }]
                }
            }, {
                "id": "b",
                "name": "Page B",
                "component": { "id": "root-b", "type": "wrapper" }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn summaries_expose_page_identity_and_component_count() {
        let summaries = document().page_summaries();
        assert_eq!(summaries[0].name, "Page A");
        assert_eq!(summaries[0].component_count, 2);
    }

    #[test]
    fn page_commands_add_move_patch_and_remove() {
        let mut document = document();
        apply_page_command(
            &mut document,
            &PageCommand::Add {
                index: 2,
                page: Box::new(blank_page("c", "Page C")),
            },
        )
        .expect("add");
        apply_page_command(
            &mut document,
            &PageCommand::Move {
                locator: PageLocator::by_id("c"),
                index: 0,
            },
        )
        .expect("move");
        apply_page_command(
            &mut document,
            &PageCommand::Patch {
                locator: PageLocator::by_id("c"),
                patch: PagePatch {
                    fields: Map::from_iter([(
                        "name".to_string(),
                        Value::String("Landing".to_string()),
                    )]),
                    ..PagePatch::default()
                },
            },
        )
        .expect("patch");
        assert_eq!(document.page_summaries()[0].name, "Landing");
        apply_page_command(
            &mut document,
            &PageCommand::Remove {
                locator: PageLocator::by_id("a"),
            },
        )
        .expect("remove");
        assert_eq!(document.project.pages.len(), 2);
        assert!(
            StyleRuleCatalog::from_document(&document)
                .component_rule("hero-a", &StyleRuleScope::Base)
                .is_none()
        );
    }

    #[test]
    fn duplicate_page_ids_are_rejected() {
        let mut document = document();
        let error = apply_page_command(
            &mut document,
            &PageCommand::Add {
                index: 2,
                page: Box::new(blank_page("a", "Duplicate")),
            },
        )
        .expect_err("duplicate id");
        assert!(matches!(error, FlyError::DuplicatePageId(_)));
    }

    #[test]
    fn last_page_cannot_be_removed() {
        let mut document = document();
        document.project.pages.truncate(1);
        assert!(matches!(
            apply_page_command(
                &mut document,
                &PageCommand::Remove {
                    locator: PageLocator::by_index(0),
                },
            ),
            Err(FlyError::LastPageRemoval)
        ));
    }
}
