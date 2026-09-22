use crate::{
    ComponentNode, ComponentObject, ComponentPatch, EditorCommand, FLY_LANDING_SECTION_FIELD,
    FlyError, LandingSectionKind, ProjectDocument, TraitOption, TraitSchema, TraitTarget,
    TraitValueKind,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const FLY_LANDING_PROPERTY_FIELD: &str = "flyLandingProperty";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LandingPropertySchema {
    pub id: String,
    pub section_kind: LandingSectionKind,
    pub group: String,
    pub label: String,
    pub role: String,
    #[serde(default)]
    pub occurrence: usize,
    pub component_type: String,
    pub value_type: TraitValueKind,
    pub target: TraitTarget,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub options: Vec<TraitOption>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

impl LandingPropertySchema {
    pub fn read(&self, component: &ComponentObject) -> Option<Value> {
        self.as_trait_schema().read(component)
    }

    pub fn patch_from_text(&self, raw: &str) -> Result<ComponentPatch, FlyError> {
        self.as_trait_schema().patch_from_text(raw)
    }

    fn as_trait_schema(&self) -> TraitSchema {
        TraitSchema {
            id: self.id.clone(),
            label: self.label.clone(),
            value_type: self.value_type,
            target: self.target.clone(),
            required: self.required,
            applies_to: vec![self.component_type.clone()],
            options: self.options.clone(),
            placeholder: self.placeholder.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LandingPropertyTargetSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_id: Option<String>,
    pub component_type: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LandingPropertySnapshot {
    pub schema: LandingPropertySchema,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<LandingPropertyTargetSnapshot>,
}

impl LandingPropertySnapshot {
    pub fn command_from_text(&self, raw: &str) -> Result<EditorCommand, LandingPropertyEditError> {
        let target = self
            .target
            .as_ref()
            .ok_or_else(|| LandingPropertyEditError::Unavailable {
                property_id: self.schema.id.clone(),
            })?;
        let component_id = target.component_id.clone().ok_or_else(|| {
            LandingPropertyEditError::MissingStableComponentId {
                property_id: self.schema.id.clone(),
                path: target.path.clone(),
            }
        })?;
        let patch = self
            .schema
            .patch_from_text(raw)
            .map_err(LandingPropertyEditError::InvalidValue)?;
        Ok(EditorCommand::Patch {
            component_id,
            patch,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LandingSectionPropertySnapshot {
    pub page_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    pub section_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_component_id: Option<String>,
    pub section_kind: LandingSectionKind,
    #[serde(default)]
    pub properties: Vec<LandingPropertySnapshot>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LandingPropertyIssueKind {
    InvalidRoleMarker,
    MissingRole,
    UnexpectedRoleOccurrence,
    UnknownRole,
    ComponentTypeMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LandingPropertyIssue {
    pub page_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    pub section_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property_id: Option<String>,
    pub role: String,
    pub kind: LandingPropertyIssueKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_component_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_component_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LandingPropertyValidationReport {
    pub valid: bool,
    #[serde(default)]
    pub sections: Vec<LandingSectionPropertySnapshot>,
    #[serde(default)]
    pub issues: Vec<LandingPropertyIssue>,
}

impl LandingPropertyValidationReport {
    pub fn for_document(document: &ProjectDocument) -> Self {
        let mut sections = Vec::new();
        let mut issues = Vec::new();
        for (page_index, page) in document.project.pages.iter().enumerate() {
            if let Some(root) = page.component.as_ref() {
                inspect_component_tree(
                    root,
                    page_index,
                    page.id.as_deref(),
                    &format!("project.pages[{page_index}].component"),
                    &mut sections,
                    &mut issues,
                );
            }
        }
        Self {
            valid: issues.is_empty(),
            sections,
            issues,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LandingPropertyEditError {
    #[error("landing property `{property_id}` has no editable target")]
    Unavailable { property_id: String },
    #[error("landing property `{property_id}` target at `{path}` has no stable component id")]
    MissingStableComponentId { property_id: String, path: String },
    #[error(transparent)]
    InvalidValue(#[from] FlyError),
}

pub fn landing_property_schemas(kind: LandingSectionKind) -> Vec<LandingPropertySchema> {
    match kind {
        LandingSectionKind::Hero => vec![
            content_property(
                kind, "headline", "content", "Headline", "headline", 0, "heading", true,
            ),
            content_property(kind, "body", "content", "Body", "body", 0, "text", true),
            content_property(
                kind,
                "primary_action.label",
                "primary_action",
                "Primary action label",
                "primary_action",
                0,
                "button",
                true,
            ),
            url_property(
                kind,
                "primary_action.url",
                "primary_action",
                "Primary action URL",
                "primary_action",
                0,
                "button",
                "href",
                true,
            ),
        ],
        LandingSectionKind::TwoColumns => vec![
            content_property(
                kind, "headline", "content", "Headline", "headline", 0, "heading", true,
            ),
            content_property(kind, "body", "content", "Body", "body", 0, "text", true),
            content_property(
                kind,
                "primary_action.label",
                "primary_action",
                "Primary action label",
                "primary_action",
                0,
                "button",
                true,
            ),
            url_property(
                kind,
                "primary_action.url",
                "primary_action",
                "Primary action URL",
                "primary_action",
                0,
                "button",
                "href",
                true,
            ),
            url_property(
                kind,
                "media.source",
                "media",
                "Image source",
                "media",
                0,
                "image",
                "src",
                true,
            ),
            text_attribute_property(
                kind,
                "media.alt",
                "media",
                "Alternative text",
                "media",
                0,
                "image",
                "alt",
                true,
            ),
        ],
        LandingSectionKind::FeatureGrid => {
            let mut schemas = vec![content_property(
                kind, "headline", "content", "Headline", "headline", 0, "heading", true,
            )];
            for index in 0..3 {
                schemas.push(content_property(
                    kind,
                    &format!("features.{index}.title"),
                    "features",
                    &format!("Feature {} title", index + 1),
                    "feature_title",
                    index,
                    "heading",
                    true,
                ));
                schemas.push(content_property(
                    kind,
                    &format!("features.{index}.body"),
                    "features",
                    &format!("Feature {} body", index + 1),
                    "feature_body",
                    index,
                    "text",
                    true,
                ));
            }
            schemas
        }
        LandingSectionKind::CallToAction => vec![
            content_property(
                kind, "headline", "content", "Headline", "headline", 0, "heading", true,
            ),
            content_property(kind, "body", "content", "Body", "body", 0, "text", true),
            content_property(
                kind,
                "primary_action.label",
                "primary_action",
                "Primary action label",
                "primary_action",
                0,
                "button",
                true,
            ),
            url_property(
                kind,
                "primary_action.url",
                "primary_action",
                "Primary action URL",
                "primary_action",
                0,
                "button",
                "href",
                true,
            ),
        ],
        LandingSectionKind::ContactForm => vec![
            content_property(
                kind, "headline", "content", "Headline", "headline", 0, "heading", true,
            ),
            content_property(kind, "body", "content", "Body", "body", 0, "text", true),
            url_property(
                kind,
                "form.action",
                "form",
                "Form action",
                "form",
                0,
                "form",
                "action",
                false,
            ),
            select_attribute_property(
                kind,
                "form.method",
                "form",
                "Form method",
                "form",
                0,
                "form",
                "method",
                &[("GET", "get"), ("POST", "post")],
            ),
            text_attribute_property(
                kind,
                "form.name_placeholder",
                "form",
                "Name placeholder",
                "name_field",
                0,
                "input",
                "placeholder",
                true,
            ),
            text_attribute_property(
                kind,
                "form.email_placeholder",
                "form",
                "Email placeholder",
                "email_field",
                0,
                "input",
                "placeholder",
                true,
            ),
            text_attribute_property(
                kind,
                "form.message_placeholder",
                "form",
                "Message placeholder",
                "message_field",
                0,
                "textarea",
                "placeholder",
                true,
            ),
            content_property(
                kind,
                "form.submit_label",
                "form",
                "Submit label",
                "submit",
                0,
                "submit",
                true,
            ),
        ],
    }
}

fn inspect_component_tree(
    node: &ComponentNode,
    page_index: usize,
    page_id: Option<&str>,
    path: &str,
    sections: &mut Vec<LandingSectionPropertySnapshot>,
    issues: &mut Vec<LandingPropertyIssue>,
) {
    let Some(component) = node.as_object() else {
        return;
    };
    if let Some(marker) = component
        .extensions
        .get(FLY_LANDING_SECTION_FIELD)
        .and_then(Value::as_str)
        .and_then(LandingSectionKind::from_marker)
    {
        sections.push(inspect_section(
            component, page_index, page_id, path, marker, issues,
        ));
    }
    for (index, child) in component.children().iter().enumerate() {
        inspect_component_tree(
            child,
            page_index,
            page_id,
            &format!("{path}.components[{index}]"),
            sections,
            issues,
        );
    }
}

fn inspect_section(
    section: &ComponentObject,
    page_index: usize,
    page_id: Option<&str>,
    section_path: &str,
    kind: LandingSectionKind,
    issues: &mut Vec<LandingPropertyIssue>,
) -> LandingSectionPropertySnapshot {
    let schemas = landing_property_schemas(kind);
    let mut targets = BTreeMap::<String, Vec<PropertyTarget<'_>>>::new();
    collect_property_targets(
        section,
        section_path,
        &mut targets,
        issues,
        page_index,
        page_id,
    );

    let expected_roles = schemas
        .iter()
        .map(|schema| schema.role.as_str())
        .collect::<BTreeSet<_>>();
    for (role, role_targets) in &targets {
        if !expected_roles.contains(role.as_str()) {
            for target in role_targets {
                issues.push(issue(
                    page_index,
                    page_id,
                    section_path,
                    None,
                    role,
                    LandingPropertyIssueKind::UnknownRole,
                    None,
                    Some(target.component.component_type()),
                    Some(target.path.as_str()),
                ));
            }
        }
    }

    let mut properties = Vec::with_capacity(schemas.len());
    for schema in schemas {
        let target = targets
            .get(&schema.role)
            .and_then(|targets| targets.get(schema.occurrence));
        let snapshot_target = match target {
            None => {
                issues.push(issue(
                    page_index,
                    page_id,
                    section_path,
                    Some(&schema.id),
                    &schema.role,
                    LandingPropertyIssueKind::MissingRole,
                    Some(&schema.component_type),
                    None,
                    None,
                ));
                None
            }
            Some(target) if target.component.component_type() != schema.component_type => {
                issues.push(issue(
                    page_index,
                    page_id,
                    section_path,
                    Some(&schema.id),
                    &schema.role,
                    LandingPropertyIssueKind::ComponentTypeMismatch,
                    Some(&schema.component_type),
                    Some(target.component.component_type()),
                    Some(target.path.as_str()),
                ));
                None
            }
            Some(target) => Some(LandingPropertyTargetSnapshot {
                component_id: target.component.id.clone(),
                component_type: target.component.component_type().to_string(),
                path: target.path.to_string(),
                value: schema.read(target.component),
            }),
        };
        properties.push(LandingPropertySnapshot {
            schema,
            target: snapshot_target,
        });
    }

    let expected_counts = expected_role_counts(&properties);
    for (role, role_targets) in &targets {
        if let Some(expected) = expected_counts.get(role) {
            for target in role_targets.iter().skip(*expected) {
                issues.push(issue(
                    page_index,
                    page_id,
                    section_path,
                    None,
                    role,
                    LandingPropertyIssueKind::UnexpectedRoleOccurrence,
                    None,
                    Some(target.component.component_type()),
                    Some(target.path.as_str()),
                ));
            }
        }
    }

    LandingSectionPropertySnapshot {
        page_index,
        page_id: page_id.map(ToString::to_string),
        section_path: section_path.to_string(),
        section_component_id: section.id.clone(),
        section_kind: kind,
        properties,
    }
}

struct PropertyTarget<'a> {
    component: &'a ComponentObject,
    path: String,
}

fn collect_property_targets<'a>(
    section: &'a ComponentObject,
    section_path: &str,
    targets: &mut BTreeMap<String, Vec<PropertyTarget<'a>>>,
    issues: &mut Vec<LandingPropertyIssue>,
    page_index: usize,
    page_id: Option<&str>,
) {
    collect_property_target(
        section,
        section_path,
        targets,
        issues,
        page_index,
        page_id,
        section_path,
    );
}

fn collect_property_target<'a>(
    component: &'a ComponentObject,
    path: &str,
    targets: &mut BTreeMap<String, Vec<PropertyTarget<'a>>>,
    issues: &mut Vec<LandingPropertyIssue>,
    page_index: usize,
    page_id: Option<&str>,
    section_path: &str,
) {
    if let Some(marker) = component.extensions.get(FLY_LANDING_PROPERTY_FIELD) {
        match marker
            .as_str()
            .map(str::trim)
            .filter(|marker| !marker.is_empty())
        {
            Some(role) => targets
                .entry(role.to_string())
                .or_default()
                .push(PropertyTarget {
                    component,
                    path: path.to_string(),
                }),
            None => issues.push(issue(
                page_index,
                page_id,
                section_path,
                None,
                "",
                LandingPropertyIssueKind::InvalidRoleMarker,
                None,
                Some(component.component_type()),
                Some(path),
            )),
        }
    }
    for (index, child) in component.children().iter().enumerate() {
        if let Some(child) = child.as_object() {
            if child.extensions.contains_key(FLY_LANDING_SECTION_FIELD) {
                continue;
            }
            let child_path = format!("{path}.components[{index}]");
            collect_property_target(
                child,
                &child_path,
                targets,
                issues,
                page_index,
                page_id,
                section_path,
            );
        }
    }
}

fn expected_role_counts(properties: &[LandingPropertySnapshot]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for property in properties {
        counts
            .entry(property.schema.role.clone())
            .and_modify(|count: &mut usize| *count = (*count).max(property.schema.occurrence + 1))
            .or_insert(property.schema.occurrence + 1);
    }
    counts
}

#[allow(clippy::too_many_arguments)]
fn issue(
    page_index: usize,
    page_id: Option<&str>,
    section_path: &str,
    property_id: Option<&str>,
    role: &str,
    kind: LandingPropertyIssueKind,
    expected_component_type: Option<&str>,
    actual_component_type: Option<&str>,
    target_path: Option<&str>,
) -> LandingPropertyIssue {
    LandingPropertyIssue {
        page_index,
        page_id: page_id.map(ToString::to_string),
        section_path: section_path.to_string(),
        property_id: property_id.map(ToString::to_string),
        role: role.to_string(),
        kind,
        expected_component_type: expected_component_type.map(ToString::to_string),
        actual_component_type: actual_component_type.map(ToString::to_string),
        target_path: target_path.map(ToString::to_string),
    }
}

#[allow(clippy::too_many_arguments)]
fn content_property(
    kind: LandingSectionKind,
    id: &str,
    group: &str,
    label: &str,
    role: &str,
    occurrence: usize,
    component_type: &str,
    required: bool,
) -> LandingPropertySchema {
    property(
        kind,
        id,
        group,
        label,
        role,
        occurrence,
        component_type,
        TraitValueKind::Multiline,
        TraitTarget::Field {
            name: "content".to_string(),
        },
        required,
        Vec::new(),
    )
}

#[allow(clippy::too_many_arguments)]
fn url_property(
    kind: LandingSectionKind,
    id: &str,
    group: &str,
    label: &str,
    role: &str,
    occurrence: usize,
    component_type: &str,
    attribute: &str,
    required: bool,
) -> LandingPropertySchema {
    property(
        kind,
        id,
        group,
        label,
        role,
        occurrence,
        component_type,
        TraitValueKind::Url,
        TraitTarget::Attribute {
            name: attribute.to_string(),
        },
        required,
        Vec::new(),
    )
}

#[allow(clippy::too_many_arguments)]
fn text_attribute_property(
    kind: LandingSectionKind,
    id: &str,
    group: &str,
    label: &str,
    role: &str,
    occurrence: usize,
    component_type: &str,
    attribute: &str,
    required: bool,
) -> LandingPropertySchema {
    property(
        kind,
        id,
        group,
        label,
        role,
        occurrence,
        component_type,
        TraitValueKind::Text,
        TraitTarget::Attribute {
            name: attribute.to_string(),
        },
        required,
        Vec::new(),
    )
}

#[allow(clippy::too_many_arguments)]
fn select_attribute_property(
    kind: LandingSectionKind,
    id: &str,
    group: &str,
    label: &str,
    role: &str,
    occurrence: usize,
    component_type: &str,
    attribute: &str,
    options: &[(&str, &str)],
) -> LandingPropertySchema {
    property(
        kind,
        id,
        group,
        label,
        role,
        occurrence,
        component_type,
        TraitValueKind::Select,
        TraitTarget::Attribute {
            name: attribute.to_string(),
        },
        false,
        options
            .iter()
            .map(|(label, value)| TraitOption {
                label: (*label).to_string(),
                value: (*value).to_string(),
            })
            .collect(),
    )
}

#[allow(clippy::too_many_arguments)]
fn property(
    kind: LandingSectionKind,
    id: &str,
    group: &str,
    label: &str,
    role: &str,
    occurrence: usize,
    component_type: &str,
    value_type: TraitValueKind,
    target: TraitTarget,
    required: bool,
    options: Vec<TraitOption>,
) -> LandingPropertySchema {
    LandingPropertySchema {
        id: format!("fly.landing.{}.{}", kind.as_str(), id),
        section_kind: kind,
        group: group.to_string(),
        label: label.to_string(),
        role: role.to_string(),
        occurrence,
        component_type: component_type.to_string(),
        value_type,
        target,
        required,
        options,
        placeholder: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FlyEditor, GrapesJsCodec, RegistrySet};
    use serde_json::json;

    fn section(kind: LandingSectionKind) -> Value {
        let registries = RegistrySet::with_builtins();
        serde_json::to_value(
            &registries
                .blocks
                .get(kind.block_id())
                .expect("landing block")
                .component,
        )
        .expect("section JSON")
    }

    fn document(kind: LandingSectionKind) -> ProjectDocument {
        let mut section = section(kind);
        assign_ids(&mut section, "node");
        GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [section]
                }
            }]
        }))
        .expect("document")
    }

    fn assign_ids(value: &mut Value, prefix: &str) {
        fn walk(value: &mut Value, prefix: &str, sequence: &mut usize) {
            let Some(object) = value.as_object_mut() else {
                return;
            };
            object.insert(
                "id".to_string(),
                Value::String(format!("{prefix}-{}", *sequence)),
            );
            *sequence += 1;
            if let Some(children) = object.get_mut("components").and_then(Value::as_array_mut) {
                for child in children {
                    walk(child, prefix, sequence);
                }
            }
        }
        walk(value, prefix, &mut 0);
    }

    #[test]
    fn every_builtin_section_exposes_a_valid_property_contract() {
        for kind in LandingSectionKind::ALL {
            let report = LandingPropertyValidationReport::for_document(&document(kind));
            assert!(report.valid, "{kind}: {:?}", report.issues);
            assert_eq!(report.sections.len(), 1);
            assert!(
                report.sections[0]
                    .properties
                    .iter()
                    .all(|property| property.target.is_some())
            );
        }
    }

    #[test]
    fn property_snapshot_builds_the_standard_editor_patch_command() {
        let project = document(LandingSectionKind::Hero);
        let report = LandingPropertyValidationReport::for_document(&project);
        let headline = report.sections[0]
            .properties
            .iter()
            .find(|property| property.schema.id.ends_with(".headline"))
            .expect("headline property");
        let target_id = headline
            .target
            .as_ref()
            .and_then(|target| target.component_id.clone())
            .expect("target id");
        let command = headline
            .command_from_text("A new headline")
            .expect("command");
        let mut editor = FlyEditor::new(project, RegistrySet::with_builtins());
        editor.apply(command).expect("patch");
        assert_eq!(
            editor
                .document()
                .component(&target_id)
                .expect("patched headline")
                .extensions["content"],
            json!("A new headline")
        );
        assert_eq!(
            headline
                .schema
                .patch_from_text("A new headline")
                .expect("patch")
                .fields["content"],
            json!("A new headline")
        );
    }

    #[test]
    fn missing_or_unknown_roles_are_reported_without_mutating_the_document() {
        let mut value = section(LandingSectionKind::Hero);
        value["components"][0]["components"][0]
            .as_object_mut()
            .expect("heading")
            .remove(FLY_LANDING_PROPERTY_FIELD);
        value["components"][0]["components"][1][FLY_LANDING_PROPERTY_FIELD] = json!("unknown");
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper", "components": [value] }
            }]
        }))
        .expect("document");
        let report = LandingPropertyValidationReport::for_document(&document);
        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == LandingPropertyIssueKind::MissingRole)
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == LandingPropertyIssueKind::UnknownRole)
        );
    }
}
