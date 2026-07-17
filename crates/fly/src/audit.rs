use crate::{ComponentNode, PageLocator, PageMetadata, ProjectDocument};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AuditSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditDiagnostic {
    pub severity: AuditSeverity,
    pub code: String,
    pub path: String,
    pub component_id: Option<String>,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AuditReport {
    pub diagnostics: Vec<AuditDiagnostic>,
    pub component_count: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
}

impl AuditReport {
    pub fn is_clean(&self) -> bool {
        self.error_count == 0 && self.warning_count == 0
    }

    pub fn blocking(&self) -> impl Iterator<Item = &AuditDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == AuditSeverity::Error)
    }

    fn push(&mut self, diagnostic: AuditDiagnostic) {
        match diagnostic.severity {
            AuditSeverity::Error => self.error_count += 1,
            AuditSeverity::Warning => self.warning_count += 1,
            AuditSeverity::Info => self.info_count += 1,
        }
        self.diagnostics.push(diagnostic);
    }
}

pub fn audit_page(document: &ProjectDocument, locator: &PageLocator) -> AuditReport {
    let mut report = AuditReport::default();
    let Ok(page) = document.page(locator) else {
        report.push(diagnostic(
            AuditSeverity::Error,
            "audit_page_not_found",
            "pages",
            None,
            "page could not be resolved for audit",
            None,
        ));
        return report;
    };

    audit_metadata(&PageMetadata::from_page(page), &mut report);
    let Some(root) = page.component.as_ref() else {
        report.push(diagnostic(
            AuditSeverity::Error,
            "audit_missing_root",
            "page.component",
            None,
            "page has no renderable component root",
            Some("Add a wrapper component before publishing."),
        ));
        return report;
    };

    let mut state = AuditState::default();
    audit_node(root, "page.component", false, &mut state, &mut report);
    audit_label_associations(&state, &mut report);
    audit_document_structure(&state, &mut report);
    report
}

#[derive(Default)]
struct AuditState {
    dom_ids: BTreeMap<String, Vec<(String, Option<String>)>>,
    labels_for: BTreeSet<String>,
    form_fields: Vec<FormField>,
    heading_levels: Vec<(u8, String, Option<String>)>,
    main_landmarks: usize,
    nav_landmarks: usize,
    h1_count: usize,
}

struct FormField {
    path: String,
    component_id: Option<String>,
    dom_id: Option<String>,
    accessible_name: bool,
    input_type: Option<String>,
}

fn audit_node(
    node: &ComponentNode,
    path: &str,
    inside_label: bool,
    state: &mut AuditState,
    report: &mut AuditReport,
) {
    let Some(component) = node.as_object() else {
        return;
    };
    report.component_count += 1;
    let component_id = component.id.clone();
    let tag = semantic_tag(component.component_type(), component.tag_name.as_deref());
    let content = component
        .extensions
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let aria_label = string_attribute(component, "aria-label");
    let title = string_attribute(component, "title");
    let accessible_name = !content.is_empty()
        || aria_label
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || title
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());

    if let Some(dom_id) = string_attribute(component, "id") {
        state
            .dom_ids
            .entry(dom_id)
            .or_default()
            .push((path.to_string(), component_id.clone()));
    }

    if tag == "main" {
        state.main_landmarks += 1;
    }
    if tag == "nav" {
        state.nav_landmarks += 1;
        if !accessible_name {
            report.push(diagnostic(
                AuditSeverity::Warning,
                "navigation_missing_label",
                path,
                component_id.clone(),
                "navigation landmark has no accessible label",
                Some("Set an aria-label such as Primary navigation."),
            ));
        }
    }

    if let Some(level) = heading_level(tag) {
        if level == 1 {
            state.h1_count += 1;
        }
        state
            .heading_levels
            .push((level, path.to_string(), component_id.clone()));
        if !accessible_name {
            report.push(diagnostic(
                AuditSeverity::Error,
                "empty_heading",
                path,
                component_id.clone(),
                "heading has no text or accessible label",
                Some("Add concise heading content."),
            ));
        }
    }

    match tag {
        "img" => audit_image(component, path, component_id.clone(), report),
        "a" => audit_link(
            component,
            accessible_name,
            path,
            component_id.clone(),
            report,
        ),
        "button" => {
            if !accessible_name {
                report.push(diagnostic(
                    AuditSeverity::Error,
                    "button_missing_name",
                    path,
                    component_id.clone(),
                    "button has no accessible name",
                    Some("Add button text or aria-label."),
                ));
            }
        }
        "label" => {
            if let Some(for_id) = string_attribute(component, "for") {
                state.labels_for.insert(for_id);
            }
            if !accessible_name && component.children().is_empty() {
                report.push(diagnostic(
                    AuditSeverity::Warning,
                    "empty_form_label",
                    path,
                    component_id.clone(),
                    "form label has no text",
                    Some("Add a visible label or aria-label to the field."),
                ));
            }
        }
        "input" | "textarea" | "select" => {
            let input_type = string_attribute(component, "type");
            let hidden = input_type.as_deref() == Some("hidden");
            if !hidden {
                state.form_fields.push(FormField {
                    path: path.to_string(),
                    component_id: component_id.clone(),
                    dom_id: string_attribute(component, "id"),
                    accessible_name: inside_label || accessible_name,
                    input_type,
                });
            }
            if string_attribute(component, "name").is_none() && !hidden {
                report.push(diagnostic(
                    AuditSeverity::Warning,
                    "form_field_missing_name",
                    path,
                    component_id.clone(),
                    "form field has no name attribute",
                    Some("Set a stable name so submissions can identify the value."),
                ));
            }
        }
        "video" | "audio" => {
            let autoplay = bool_attribute(component, "autoplay");
            let muted = bool_attribute(component, "muted");
            if autoplay && !muted {
                report.push(diagnostic(
                    AuditSeverity::Error,
                    "unmuted_autoplay_media",
                    path,
                    component_id.clone(),
                    "autoplay media is not muted",
                    Some("Disable autoplay or enable muted."),
                ));
            }
            if !bool_attribute(component, "controls") {
                report.push(diagnostic(
                    AuditSeverity::Warning,
                    "media_missing_controls",
                    path,
                    component_id.clone(),
                    "media has no visible controls",
                    Some("Enable controls unless custom accessible controls are provided."),
                ));
            }
        }
        _ => {}
    }

    let child_inside_label = inside_label || tag == "label";
    for (index, child) in component.children().iter().enumerate() {
        audit_node(
            child,
            &format!("{path}.components[{index}]"),
            child_inside_label,
            state,
            report,
        );
    }
}

fn audit_metadata(metadata: &PageMetadata, report: &mut AuditReport) {
    if metadata.title.is_none() {
        report.push(diagnostic(
            AuditSeverity::Warning,
            "page_missing_title",
            "page.flyPageMeta.title",
            None,
            "page has no SEO title",
            Some("Set a unique, descriptive title."),
        ));
    }
    if metadata.description.is_none() {
        report.push(diagnostic(
            AuditSeverity::Info,
            "page_missing_description",
            "page.flyPageMeta.description",
            None,
            "page has no SEO description",
            Some("Add a short search and sharing description."),
        ));
    }
    if metadata.slug.is_none() {
        report.push(diagnostic(
            AuditSeverity::Info,
            "page_missing_slug",
            "page.flyPageMeta.slug",
            None,
            "page has no explicit slug",
            Some("Set a stable URL slug."),
        ));
    }
}

fn audit_image(
    component: &crate::ComponentObject,
    path: &str,
    component_id: Option<String>,
    report: &mut AuditReport,
) {
    match string_attribute(component, "alt") {
        None => report.push(diagnostic(
            AuditSeverity::Error,
            "image_missing_alt",
            path,
            component_id,
            "image has no alt attribute",
            Some("Describe the image, or use an empty alt for a decorative image."),
        )),
        Some(alt) if alt.trim().is_empty() => {
            if string_attribute(component, "role").as_deref() != Some("presentation") {
                report.push(diagnostic(
                    AuditSeverity::Info,
                    "decorative_image_without_role",
                    path,
                    component_id,
                    "image has empty alt but is not explicitly marked decorative",
                    Some("Set role=presentation when the image is purely decorative."),
                ));
            }
        }
        Some(_) => {}
    }
}

fn audit_link(
    component: &crate::ComponentObject,
    accessible_name: bool,
    path: &str,
    component_id: Option<String>,
    report: &mut AuditReport,
) {
    if !accessible_name {
        report.push(diagnostic(
            AuditSeverity::Error,
            "link_missing_name",
            path,
            component_id.clone(),
            "link has no accessible name",
            Some("Add link text or aria-label."),
        ));
    }
    match string_attribute(component, "href") {
        None => report.push(diagnostic(
            AuditSeverity::Warning,
            "link_missing_href",
            path,
            component_id.clone(),
            "link has no href",
            Some("Set a destination or use a button for an action."),
        )),
        Some(href) if href.trim().to_ascii_lowercase().starts_with("javascript:") => {
            report.push(diagnostic(
                AuditSeverity::Error,
                "unsafe_link_href",
                path,
                component_id.clone(),
                "link uses a javascript URL that will be removed by the renderer",
                Some("Use an http, https, relative, hash, mailto, or tel URL."),
            ))
        }
        Some(_) => {}
    }
    if string_attribute(component, "target").as_deref() == Some("_blank") {
        let rel = string_attribute(component, "rel").unwrap_or_default();
        if !rel.split_whitespace().any(|token| token == "noopener") {
            report.push(diagnostic(
                AuditSeverity::Warning,
                "blank_link_missing_noopener",
                path,
                component_id,
                "new-tab link is missing rel=noopener",
                Some("Add noopener to the rel attribute."),
            ));
        }
    }
}

fn audit_label_associations(state: &AuditState, report: &mut AuditReport) {
    for field in &state.form_fields {
        let associated = field.accessible_name
            || field
                .dom_id
                .as_ref()
                .is_some_and(|dom_id| state.labels_for.contains(dom_id));
        if !associated {
            report.push(diagnostic(
                AuditSeverity::Error,
                "form_field_missing_label",
                &field.path,
                field.component_id.clone(),
                format!(
                    "{} field has no associated label or aria-label",
                    field.input_type.as_deref().unwrap_or("form")
                ),
                Some("Wrap the field in a label, use label[for], or set aria-label."),
            ));
        }
    }
}

fn audit_document_structure(state: &AuditState, report: &mut AuditReport) {
    for (dom_id, occurrences) in &state.dom_ids {
        if occurrences.len() > 1 {
            for (path, component_id) in occurrences {
                report.push(diagnostic(
                    AuditSeverity::Error,
                    "duplicate_dom_id",
                    path,
                    component_id.clone(),
                    format!("DOM id `{dom_id}` is used {} times", occurrences.len()),
                    Some("Assign a unique id to each element."),
                ));
            }
        }
    }

    if state.main_landmarks == 0 {
        report.push(diagnostic(
            AuditSeverity::Info,
            "missing_main_landmark",
            "page.component",
            None,
            "page has no main landmark",
            Some("Use a main tag for the primary content region."),
        ));
    } else if state.main_landmarks > 1 {
        report.push(diagnostic(
            AuditSeverity::Warning,
            "multiple_main_landmarks",
            "page.component",
            None,
            format!("page contains {} main landmarks", state.main_landmarks),
            Some("Keep one primary main landmark."),
        ));
    }

    if state.h1_count == 0 {
        report.push(diagnostic(
            AuditSeverity::Warning,
            "missing_h1",
            "page.component",
            None,
            "page has no level-one heading",
            Some("Add one descriptive h1."),
        ));
    } else if state.h1_count > 1 {
        report.push(diagnostic(
            AuditSeverity::Info,
            "multiple_h1",
            "page.component",
            None,
            format!("page contains {} level-one headings", state.h1_count),
            Some("Confirm that each h1 represents a distinct top-level section."),
        ));
    }

    let mut previous = None;
    for (level, path, component_id) in &state.heading_levels {
        if previous.is_some_and(|previous| *level > previous + 1) {
            report.push(diagnostic(
                AuditSeverity::Warning,
                "heading_level_skipped",
                path,
                component_id.clone(),
                format!("heading level jumps to h{level}"),
                Some("Use sequential heading levels without skipping."),
            ));
        }
        previous = Some(*level);
    }
}

fn semantic_tag(component_type: &str, tag_name: Option<&str>) -> &'static str {
    let tag = tag_name
        .unwrap_or(match component_type {
            "heading" => "h2",
            "text" => "p",
            "link" => "a",
            "button" | "submit" => "button",
            "image" => "img",
            "video" => "video",
            "form" => "form",
            "label" => "label",
            "input" | "checkbox" => "input",
            "textarea" => "textarea",
            "select" => "select",
            "section" => "section",
            _ => "div",
        })
        .to_ascii_lowercase();
    match tag.as_str() {
        "main" => "main",
        "nav" => "nav",
        "h1" => "h1",
        "h2" => "h2",
        "h3" => "h3",
        "h4" => "h4",
        "h5" => "h5",
        "h6" => "h6",
        "img" => "img",
        "a" => "a",
        "button" => "button",
        "label" => "label",
        "input" => "input",
        "textarea" => "textarea",
        "select" => "select",
        "video" => "video",
        "audio" => "audio",
        "form" => "form",
        _ => "div",
    }
}

fn heading_level(tag: &str) -> Option<u8> {
    tag.strip_prefix('h')?
        .parse::<u8>()
        .ok()
        .filter(|level| (1..=6).contains(level))
}

fn string_attribute(component: &crate::ComponentObject, name: &str) -> Option<String> {
    component
        .attributes
        .get(name)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn bool_attribute(component: &crate::ComponentObject, name: &str) -> bool {
    component
        .attributes
        .get(name)
        .is_some_and(|value| match value {
            Value::Bool(value) => *value,
            Value::String(value) => {
                !matches!(value.to_ascii_lowercase().as_str(), "false" | "0" | "off")
            }
            _ => false,
        })
}

fn diagnostic(
    severity: AuditSeverity,
    code: impl Into<String>,
    path: impl Into<String>,
    component_id: Option<String>,
    message: impl Into<String>,
    suggestion: Option<&str>,
) -> AuditDiagnostic {
    AuditDiagnostic {
        severity,
        code: code.into(),
        path: path.into(),
        component_id,
        message: message.into(),
        suggestion: suggestion.map(ToString::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    #[test]
    fn audit_finds_accessibility_and_structure_issues() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        { "id": "heading", "type": "heading", "tagName": "h3", "content": "Jumped heading" },
                        { "id": "image", "type": "image", "attributes": { "src": "/hero.png", "id": "duplicate" } },
                        { "id": "link", "type": "link", "attributes": { "target": "_blank", "id": "duplicate" } },
                        { "id": "input", "type": "input", "attributes": { "type": "email" } },
                        { "id": "video", "type": "video", "attributes": { "autoplay": true } }
                    ]
                }
            }]
        }))
        .expect("document");
        let report = audit_page(&document, &PageLocator::by_id("home"));
        for code in [
            "image_missing_alt",
            "link_missing_name",
            "link_missing_href",
            "form_field_missing_label",
            "unmuted_autoplay_media",
            "duplicate_dom_id",
            "missing_h1",
        ] {
            assert!(
                report.diagnostics.iter().any(|item| item.code == code),
                "missing audit code {code}"
            );
        }
        assert!(report.error_count > 0);
    }

    #[test]
    fn accessible_page_is_clean_of_errors() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": { "title": "Home", "description": "Description", "slug": "home" },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "tagName": "main",
                    "components": [
                        { "id": "heading", "type": "heading", "tagName": "h1", "content": "Welcome" },
                        { "id": "image", "type": "image", "attributes": { "src": "/hero.png", "alt": "Team working" } },
                        { "id": "link", "type": "link", "content": "Contact", "attributes": { "href": "#contact" } },
                        { "id": "label", "type": "label", "content": "Email", "attributes": { "for": "email" } },
                        { "id": "input", "type": "input", "attributes": { "id": "email", "name": "email", "type": "email" } }
                    ]
                }
            }]
        }))
        .expect("document");
        let report = audit_page(&document, &PageLocator::by_id("home"));
        assert_eq!(report.error_count, 0, "{:?}", report.diagnostics);
    }
}
