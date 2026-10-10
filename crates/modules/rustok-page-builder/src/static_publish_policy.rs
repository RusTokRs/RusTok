use fly::{
    AssetDescriptor, AssetKind, ComponentChildren, ComponentNode, ComponentObject,
    FLY_PAGE_METADATA_FIELD, ProjectDocument, StyleRuleDescriptor, StyleRuleScope,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const PAGE_BUILDER_STATIC_PUBLISH_POLICY_FORMAT: &str = "page_builder_static_publish_policy_v1";

const ALLOWED_TAGS: &[&str] = &[
    "a",
    "article",
    "aside",
    "audio",
    "blockquote",
    "br",
    "button",
    "div",
    "em",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "img",
    "input",
    "label",
    "li",
    "main",
    "nav",
    "ol",
    "option",
    "p",
    "picture",
    "section",
    "select",
    "small",
    "source",
    "span",
    "strong",
    "textarea",
    "ul",
    "video",
];

// Fly's built-in `link` component is intentionally not listed: it renders as a safe `<a>`.
const DANGEROUS_COMPONENT_TYPES: &[&str] = &[
    "applet", "base", "embed", "iframe", "meta", "object", "script", "style", "template",
];

const FORBIDDEN_ATTRIBUTES: &[&str] = &[
    "background",
    "ping",
    "srcdoc",
    "style",
    "xlink:href",
];

/// `srcset` is deliberately absent from [`FORBIDDEN_ATTRIBUTES`]: responsive image
/// candidates are validated per URL by [`validate_srcset`], with exactly the rule this
/// policy applies to image `src` values minus `data:` payloads (their base64 bodies
/// collide with the candidate comma grammar). A blanket ban would refuse the same
/// image that `src` accepts — the incoherence this policy already refused for CSS
/// references. An operator who re-adds `"srcset"` to `forbidden_attributes` still
/// gets the blanket ban: the configuration is honoured over the default.

const URL_ATTRIBUTES: &[&str] = &[
    "action",
    "cite",
    "formaction",
    "href",
    "poster",
    "src",
    "usemap",
];

const ALLOWED_DATA_IMAGE_PREFIXES: &[&str] = &[
    "data:image/avif;base64,",
    "data:image/gif;base64,",
    "data:image/jpeg;base64,",
    "data:image/png;base64,",
    "data:image/webp;base64,",
];

/// CSS tokens refused in a published static landing.
///
/// `url(` is deliberately **not** listed. A blanket ban looked like a "published landings must be
/// self-contained" rule, but this policy already permits `https://` and relative URLs for
/// resource attributes (see [`UrlKind::Resource`] and [`UrlKind::ResourceImage`]), so the same
/// image was accepted through `src` and refused through `background-image`. That is not a
/// stricter policy, it is an incoherent one — and it surfaced as a late, confusing publish
/// failure after the author had already seen the background render in the editor.
///
/// References are instead validated per URL, with exactly the rule this policy applies to image
/// resources, by [`strip_validated_css_urls`]. Everything listed below has no legitimate use in a
/// declaration and stays banned outright.
///
/// An operator who re-adds `"url("` to `forbidden_css_tokens` still gets the blanket ban: the
/// configuration is honoured over the default.
const FORBIDDEN_CSS_TOKENS: &[&str] = &[
    "-moz-binding",
    "@import",
    "behavior:",
    "data:",
    "expression(",
    "javascript:",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageBuilderStaticPublishPolicy {
    pub format: String,
    pub max_url_bytes: usize,
    pub max_attribute_name_bytes: usize,
    pub max_attribute_value_bytes: usize,
    pub max_css_property_bytes: usize,
    pub max_css_value_bytes: usize,
    pub max_content_bytes: usize,
    pub max_media_query_bytes: usize,
    pub max_srcset_candidates: usize,
    pub max_sizes_entries: usize,
    pub allowed_tags: Vec<String>,
    pub dangerous_component_types: Vec<String>,
    pub forbidden_attributes: Vec<String>,
    pub url_attributes: Vec<String>,
    pub allowed_data_image_prefixes: Vec<String>,
    pub forbidden_css_tokens: Vec<String>,
}

impl Default for PageBuilderStaticPublishPolicy {
    fn default() -> Self {
        Self {
            format: PAGE_BUILDER_STATIC_PUBLISH_POLICY_FORMAT.to_string(),
            max_url_bytes: 2_048,
            max_attribute_name_bytes: 128,
            max_attribute_value_bytes: 16 * 1_024,
            max_css_property_bytes: 128,
            max_css_value_bytes: 16 * 1_024,
            max_content_bytes: 1024 * 1024,
            max_media_query_bytes: 256,
            max_srcset_candidates: 16,
            max_sizes_entries: 16,
            allowed_tags: strings(ALLOWED_TAGS),
            dangerous_component_types: strings(DANGEROUS_COMPONENT_TYPES),
            forbidden_attributes: strings(FORBIDDEN_ATTRIBUTES),
            url_attributes: strings(URL_ATTRIBUTES),
            allowed_data_image_prefixes: strings(ALLOWED_DATA_IMAGE_PREFIXES),
            forbidden_css_tokens: strings(FORBIDDEN_CSS_TOKENS),
        }
    }
}

impl PageBuilderStaticPublishPolicy {
    pub fn verify_integrity(&self) -> Result<(), PageBuilderStaticPublishPolicyError> {
        if self.format != PAGE_BUILDER_STATIC_PUBLISH_POLICY_FORMAT {
            return Err(PageBuilderStaticPublishPolicyError::Integrity(
                "unsupported static publish policy format".to_string(),
            ));
        }
        if self.max_url_bytes == 0
            || self.max_attribute_name_bytes == 0
            || self.max_attribute_value_bytes == 0
            || self.max_css_property_bytes == 0
            || self.max_css_value_bytes == 0
            || self.max_content_bytes == 0
            || self.max_media_query_bytes == 0
            || self.max_srcset_candidates == 0
            || self.max_sizes_entries == 0
        {
            return Err(PageBuilderStaticPublishPolicyError::Integrity(
                "static publish policy limits must be positive".to_string(),
            ));
        }
        require_normalized_unique(&self.allowed_tags, "allowed_tags")?;
        require_normalized_unique(&self.dangerous_component_types, "dangerous_component_types")?;
        require_normalized_unique(&self.forbidden_attributes, "forbidden_attributes")?;
        require_normalized_unique(&self.url_attributes, "url_attributes")?;
        require_normalized_unique(
            &self.allowed_data_image_prefixes,
            "allowed_data_image_prefixes",
        )?;
        require_normalized_unique(&self.forbidden_css_tokens, "forbidden_css_tokens")?;
        for attribute in &self.url_attributes {
            if UrlKind::for_attribute(attribute).is_none() {
                return Err(PageBuilderStaticPublishPolicyError::Integrity(format!(
                    "static publish policy URL attribute `{attribute}` has no URL kind"
                )));
            }
        }
        Ok(())
    }

    pub fn policy_hash(&self) -> Result<String, PageBuilderStaticPublishPolicyError> {
        self.verify_integrity()?;
        stable_hash(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageBuilderStaticPublishPolicyEvidence {
    pub format: String,
    pub policy_hash: String,
}

impl PageBuilderStaticPublishPolicyEvidence {
    pub fn verify_integrity(&self) -> Result<(), PageBuilderStaticPublishPolicyError> {
        let policy = PageBuilderStaticPublishPolicy::default();
        if self.format != policy.format {
            return Err(PageBuilderStaticPublishPolicyError::Integrity(
                "static publish policy evidence format mismatch".to_string(),
            ));
        }
        if !is_sha256(&self.policy_hash) || self.policy_hash != policy.policy_hash()? {
            return Err(PageBuilderStaticPublishPolicyError::Integrity(
                "static publish policy evidence hash mismatch".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageBuilderStaticPublishPolicyDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PageBuilderStaticPublishPolicyError {
    #[error("static publish policy encoding failed: {0}")]
    Encode(String),
    #[error("static publish policy integrity failed: {0}")]
    Integrity(String),
    #[error("static publish policy rejected project")]
    Rejected {
        diagnostics: Vec<PageBuilderStaticPublishPolicyDiagnostic>,
    },
}

impl PageBuilderStaticPublishPolicyError {
    pub fn diagnostics(&self) -> &[PageBuilderStaticPublishPolicyDiagnostic] {
        match self {
            Self::Rejected { diagnostics } => diagnostics,
            Self::Encode(_) | Self::Integrity(_) => &[],
        }
    }
}

pub fn validate_static_publish_document(
    document: &ProjectDocument,
) -> Result<PageBuilderStaticPublishPolicyEvidence, PageBuilderStaticPublishPolicyError> {
    let policy = PageBuilderStaticPublishPolicy::default();
    policy.verify_integrity()?;

    let mut diagnostics = Vec::new();
    for (page_index, page) in document.project.pages.iter().enumerate() {
        if let Some(root) = page.component.as_ref() {
            validate_component_node(
                root,
                &format!("pages[{page_index}].component"),
                &policy,
                &mut diagnostics,
            );
        }
    }
    validate_style_rules(document, &policy, &mut diagnostics);
    validate_assets(document, &policy, &mut diagnostics);
    validate_page_metadata(document, &policy, &mut diagnostics);

    if !diagnostics.is_empty() {
        return Err(PageBuilderStaticPublishPolicyError::Rejected { diagnostics });
    }

    Ok(PageBuilderStaticPublishPolicyEvidence {
        format: policy.format.clone(),
        policy_hash: policy.policy_hash()?,
    })
}

fn validate_component_node(
    node: &ComponentNode,
    path: &str,
    policy: &PageBuilderStaticPublishPolicy,
    diagnostics: &mut Vec<PageBuilderStaticPublishPolicyDiagnostic>,
) {
    match node {
        ComponentNode::Object(component) => {
            validate_component(component, path, policy, diagnostics);
            match &component.components {
                ComponentChildren::Nodes(children) => {
                    for (index, child) in children.iter().enumerate() {
                        validate_component_node(
                            child,
                            &format!("{path}.components[{index}]"),
                            policy,
                            diagnostics,
                        );
                    }
                }
                ComponentChildren::Opaque(Value::Null) => {}
                ComponentChildren::Opaque(_) => reject(
                    diagnostics,
                    "landing_component_children_opaque",
                    format!("{path}.components"),
                    "component children use an opaque shape that the static renderer would omit",
                ),
            }
        }
        ComponentNode::Opaque(value) => validate_opaque_node(value, path, policy, diagnostics),
    }
}

fn validate_opaque_node(
    value: &Value,
    path: &str,
    policy: &PageBuilderStaticPublishPolicy,
    diagnostics: &mut Vec<PageBuilderStaticPublishPolicyDiagnostic>,
) {
    match value {
        Value::String(content) => validate_text_content(content, path, policy, diagnostics),
        Value::Number(_) | Value::Bool(_) => {}
        Value::Null | Value::Array(_) | Value::Object(_) => reject(
            diagnostics,
            "landing_opaque_node_not_renderable",
            path,
            "opaque component node is not a renderer-supported scalar",
        ),
    }
}

fn validate_component(
    component: &ComponentObject,
    path: &str,
    policy: &PageBuilderStaticPublishPolicy,
    diagnostics: &mut Vec<PageBuilderStaticPublishPolicyDiagnostic>,
) {
    if let Some(tag) = component.tag_name.as_deref() {
        let normalized = tag.trim().to_ascii_lowercase();
        if !policy
            .allowed_tags
            .iter()
            .any(|allowed| allowed == &normalized)
        {
            reject(
                diagnostics,
                "landing_tag_not_allowed",
                format!("{path}.tagName"),
                format!("explicit tag `{tag}` is not allowed in a static public artifact"),
            );
        }
    }

    let component_type = component.component_type().trim().to_ascii_lowercase();
    if policy
        .dangerous_component_types
        .iter()
        .any(|forbidden| forbidden == &component_type)
    {
        reject(
            diagnostics,
            "landing_component_type_forbidden",
            format!("{path}.type"),
            format!("component type `{component_type}` is forbidden in a static public artifact"),
        );
    }

    if let Some(content) = component.extensions.get("content") {
        match content.as_str() {
            Some(content) => {
                validate_text_content(content, &format!("{path}.content"), policy, diagnostics)
            }
            None => reject(
                diagnostics,
                "landing_content_not_string",
                format!("{path}.content"),
                "component content must be a string when present",
            ),
        }
    }

    let mut attributes = component.attributes.iter().collect::<Vec<_>>();
    attributes.sort_by(|left, right| left.0.cmp(right.0));
    for (raw_name, value) in attributes {
        validate_attribute(
            raw_name,
            value,
            &format!("{path}.attributes.{raw_name}"),
            policy,
            diagnostics,
        );
    }

    if let Some(style) = component.style.as_ref() {
        match style.as_object() {
            Some(style) => {
                validate_css_declarations(style, &format!("{path}.style"), policy, diagnostics)
            }
            None => reject(
                diagnostics,
                "landing_style_not_object",
                format!("{path}.style"),
                "component style must be an object",
            ),
        }
    }
}

fn validate_text_content(
    content: &str,
    path: &str,
    policy: &PageBuilderStaticPublishPolicy,
    diagnostics: &mut Vec<PageBuilderStaticPublishPolicyDiagnostic>,
) {
    if content.len() > policy.max_content_bytes {
        reject(
            diagnostics,
            "landing_content_too_large",
            path,
            format!(
                "component content exceeds {} bytes",
                policy.max_content_bytes
            ),
        );
    }
    if contains_disallowed_control(content) {
        reject(
            diagnostics,
            "landing_content_control_character",
            path,
            "component content contains a disallowed control character",
        );
    }
    if contains_tag_like_markup(content) {
        reject(
            diagnostics,
            "landing_content_markup_forbidden",
            path,
            "component content contains markup that the static renderer would strip",
        );
    }
}

fn validate_attribute(
    raw_name: &str,
    value: &Value,
    path: &str,
    policy: &PageBuilderStaticPublishPolicy,
    diagnostics: &mut Vec<PageBuilderStaticPublishPolicyDiagnostic>,
) {
    if raw_name != raw_name.trim() {
        reject(
            diagnostics,
            "landing_attribute_name_invalid",
            path,
            format!("attribute name `{raw_name}` contains surrounding whitespace"),
        );
        return;
    }
    let name = raw_name.to_ascii_lowercase();
    if name.is_empty()
        || name.len() > policy.max_attribute_name_bytes
        || !name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':')
        })
    {
        reject(
            diagnostics,
            "landing_attribute_name_invalid",
            path,
            format!("attribute name `{raw_name}` is invalid"),
        );
        return;
    }
    if name.starts_with("on") {
        reject(
            diagnostics,
            "landing_event_handler_forbidden",
            path,
            format!("event handler attribute `{raw_name}` is forbidden"),
        );
        return;
    }
    if policy
        .forbidden_attributes
        .iter()
        .any(|forbidden| forbidden == &name)
    {
        reject(
            diagnostics,
            "landing_attribute_forbidden",
            path,
            format!("attribute `{raw_name}` is forbidden by static publish policy"),
        );
        return;
    }
    if matches!(value, Value::Bool(false)) {
        reject(
            diagnostics,
            "landing_false_boolean_attribute_omitted",
            path,
            format!("false boolean attribute `{raw_name}` would be omitted by the renderer"),
        );
        return;
    }

    let scalar = match scalar_string(value) {
        Some(value) => value,
        None => {
            reject(
                diagnostics,
                "landing_attribute_not_scalar",
                path,
                format!("attribute `{raw_name}` must contain a scalar value"),
            );
            return;
        }
    };
    if scalar.len() > policy.max_attribute_value_bytes {
        reject(
            diagnostics,
            "landing_attribute_value_too_large",
            path,
            format!(
                "attribute `{raw_name}` exceeds {} bytes",
                policy.max_attribute_value_bytes
            ),
        );
    }
    if contains_disallowed_control(&scalar) {
        reject(
            diagnostics,
            "landing_attribute_control_character",
            path,
            format!("attribute `{raw_name}` contains a disallowed control character"),
        );
    }

    if name == "srcset" {
        if !value.is_string() {
            reject(
                diagnostics,
                "landing_attribute_not_scalar",
                path,
                format!("attribute `{raw_name}` must contain a scalar value"),
            );
            return;
        }
        if let Err(reason) = validate_srcset(&scalar, policy) {
            reject(
                diagnostics,
                "landing_srcset_rejected",
                path,
                format!("srcset is rejected: {reason}"),
            );
        }
        return;
    }
    if name == "sizes" {
        if !value.is_string() {
            reject(
                diagnostics,
                "landing_attribute_not_scalar",
                path,
                format!("attribute `{raw_name}` must contain a scalar value"),
            );
            return;
        }
        if let Err(reason) = validate_sizes(&scalar, policy) {
            reject(
                diagnostics,
                "landing_sizes_rejected",
                path,
                format!("sizes is rejected: {reason}"),
            );
        }
        return;
    }

    if policy.url_attributes.iter().any(|url| url == &name) {
        if !value.is_string() {
            reject(
                diagnostics,
                "landing_url_attribute_not_string",
                path,
                format!("URL attribute `{raw_name}` must contain a string"),
            );
            return;
        }
        let Some(kind) = UrlKind::for_attribute(&name) else {
            return;
        };
        if let Err(reason) = validate_url(&scalar, kind, policy) {
            reject(
                diagnostics,
                "landing_url_rejected",
                path,
                format!("URL attribute `{raw_name}` is rejected: {reason}"),
            );
        }
    }
}

fn validate_style_rules(
    document: &ProjectDocument,
    policy: &PageBuilderStaticPublishPolicy,
    diagnostics: &mut Vec<PageBuilderStaticPublishPolicyDiagnostic>,
) {
    for (index, raw) in document.project.styles.iter().enumerate() {
        let path = format!("styles[{index}]");
        let Some(rule) = StyleRuleDescriptor::from_value(raw.clone()) else {
            reject(
                diagnostics,
                "landing_style_rule_invalid",
                path,
                "style rule is not a renderer-supported object",
            );
            continue;
        };
        match rule.component_id.as_deref() {
            Some(component_id) if !component_id.is_empty() => {
                if !document.contains_component(component_id) {
                    reject(
                        diagnostics,
                        "landing_style_rule_orphaned",
                        format!("{path}.selectors"),
                        format!("style rule references missing component `{component_id}`"),
                    );
                }
            }
            _ => reject(
                diagnostics,
                "landing_style_rule_unbound",
                format!("{path}.selectors"),
                "style rule is not bound to a component and would be omitted by the renderer",
            ),
        }
        if rule.declarations.is_empty() {
            reject(
                diagnostics,
                "landing_style_rule_empty",
                format!("{path}.style"),
                "empty style rule would be omitted by the renderer",
            );
        }
        validate_css_declarations(
            &rule.declarations,
            &format!("{path}.style"),
            policy,
            diagnostics,
        );

        let Some(object) = raw.as_object() else {
            continue;
        };
        match &rule.scope {
            StyleRuleScope::Base => {
                if object.contains_key("mediaText") {
                    reject(
                        diagnostics,
                        "landing_media_query_orphaned",
                        format!("{path}.mediaText"),
                        "mediaText requires atRuleType=media",
                    );
                }
                if object
                    .get("atRuleType")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| !kind.trim().is_empty())
                {
                    reject(
                        diagnostics,
                        "landing_at_rule_unsupported",
                        format!("{path}.atRuleType"),
                        "only media style rules are supported for static publication",
                    );
                }
            }
            StyleRuleScope::Media { query } => {
                if !safe_media_query(query, policy) {
                    reject(
                        diagnostics,
                        "landing_media_query_rejected",
                        format!("{path}.mediaText"),
                        "media query is rejected by static publish policy",
                    );
                }
            }
        }
    }
}

fn validate_css_declarations(
    declarations: &serde_json::Map<String, Value>,
    path: &str,
    policy: &PageBuilderStaticPublishPolicy,
    diagnostics: &mut Vec<PageBuilderStaticPublishPolicyDiagnostic>,
) {
    let mut entries = declarations.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(right.0));
    for (name, value) in entries {
        let declaration_path = format!("{path}.{name}");
        if name.is_empty()
            || name.len() > policy.max_css_property_bytes
            || name.starts_with("--")
            || !name
                .chars()
                .all(|character| character.is_ascii_alphabetic() || character == '-')
        {
            reject(
                diagnostics,
                "landing_css_property_rejected",
                declaration_path,
                format!("CSS property `{name}` is rejected"),
            );
            continue;
        }
        let Some(value) = scalar_string(value) else {
            reject(
                diagnostics,
                "landing_css_value_not_scalar",
                declaration_path,
                format!("CSS property `{name}` must contain a scalar value"),
            );
            continue;
        };
        if value.len() > policy.max_css_value_bytes {
            reject(
                diagnostics,
                "landing_css_value_too_large",
                declaration_path.clone(),
                format!(
                    "CSS property `{name}` exceeds {} bytes",
                    policy.max_css_value_bytes
                ),
            );
        }
        if !safe_css_value(&value, policy) {
            reject(
                diagnostics,
                "landing_css_value_rejected",
                declaration_path,
                format!("CSS property `{name}` contains a forbidden token or delimiter"),
            );
        }
    }
}

fn validate_assets(
    document: &ProjectDocument,
    policy: &PageBuilderStaticPublishPolicy,
    diagnostics: &mut Vec<PageBuilderStaticPublishPolicyDiagnostic>,
) {
    let mut ids = BTreeSet::new();
    for (index, raw) in document.project.assets.iter().enumerate() {
        let path = format!("assets[{index}]");
        let Some(asset) = AssetDescriptor::from_value(raw.clone()) else {
            reject(
                diagnostics,
                "landing_asset_invalid",
                path,
                "asset entry has no supported public source",
            );
            continue;
        };
        if !ids.insert(asset.id.clone()) {
            reject(
                diagnostics,
                "landing_asset_duplicate",
                format!("{path}.id"),
                format!("asset id `{}` is duplicated", asset.id),
            );
        }
        let kind = if asset.kind == AssetKind::Image {
            UrlKind::ResourceImage
        } else {
            UrlKind::Resource
        };
        if let Err(reason) = validate_url(&asset.source, kind, policy) {
            reject(
                diagnostics,
                "landing_asset_url_rejected",
                format!("{path}.src"),
                format!("asset `{}` source is rejected: {reason}", asset.id),
            );
        }
    }
}

fn validate_page_metadata(
    document: &ProjectDocument,
    policy: &PageBuilderStaticPublishPolicy,
    diagnostics: &mut Vec<PageBuilderStaticPublishPolicyDiagnostic>,
) {
    for (page_index, page) in document.project.pages.iter().enumerate() {
        let Some(metadata) = page.extensions.get(FLY_PAGE_METADATA_FIELD) else {
            continue;
        };
        let Some(metadata) = metadata.as_object() else {
            reject(
                diagnostics,
                "landing_page_metadata_invalid",
                format!("pages[{page_index}].{FLY_PAGE_METADATA_FIELD}"),
                "Fly page metadata must be an object",
            );
            continue;
        };
        for (field, kind) in [
            ("canonical_url", UrlKind::Canonical),
            ("open_graph_image", UrlKind::ResourceImage),
        ] {
            let Some(value) = metadata.get(field) else {
                continue;
            };
            for (suffix, candidate) in localized_string_values(value) {
                let path = format!("pages[{page_index}].{FLY_PAGE_METADATA_FIELD}.{field}{suffix}");
                match candidate {
                    Some(candidate) => {
                        if let Err(reason) = validate_url(candidate, kind, policy) {
                            reject(
                                diagnostics,
                                "landing_metadata_url_rejected",
                                path,
                                format!("metadata URL `{field}` is rejected: {reason}"),
                            );
                        }
                    }
                    None => reject(
                        diagnostics,
                        "landing_metadata_url_invalid",
                        path,
                        format!("metadata URL `{field}` must be a string or localized strings"),
                    ),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum UrlKind {
    Navigation,
    Resource,
    ResourceImage,
    FormAction,
    Canonical,
    Fragment,
}

impl UrlKind {
    fn for_attribute(attribute: &str) -> Option<Self> {
        match attribute {
            "href" => Some(Self::Navigation),
            "src" | "poster" => Some(Self::ResourceImage),
            "action" | "formaction" => Some(Self::FormAction),
            "cite" => Some(Self::Canonical),
            "usemap" => Some(Self::Fragment),
            _ => None,
        }
    }
}

fn validate_url(
    value: &str,
    kind: UrlKind,
    policy: &PageBuilderStaticPublishPolicy,
) -> Result<(), &'static str> {
    let value = value.trim();
    if value.is_empty() {
        return Err("value is empty");
    }
    if value.len() > policy.max_url_bytes {
        return Err("value exceeds the URL byte limit");
    }
    if value.starts_with("//") {
        return Err("protocol-relative URLs are forbidden");
    }
    if value.contains('\\') {
        return Err("backslashes are forbidden");
    }
    if value.chars().any(char::is_control) {
        return Err("control characters are forbidden");
    }

    let normalized = value.to_ascii_lowercase();
    let https = normalized.starts_with("https://");
    let relative = relative_url_allowed(value);
    let fragment = normalized.starts_with('#');
    let data_image = policy
        .allowed_data_image_prefixes
        .iter()
        .any(|prefix| normalized.starts_with(prefix));

    let allowed = match kind {
        UrlKind::Navigation => {
            fragment
                || relative
                || https
                || normalized.starts_with("mailto:")
                || normalized.starts_with("tel:")
        }
        UrlKind::Resource => relative || https,
        UrlKind::ResourceImage => relative || https || data_image,
        UrlKind::FormAction => relative,
        UrlKind::Canonical => relative || https,
        UrlKind::Fragment => fragment,
    };
    if allowed {
        Ok(())
    } else {
        Err("scheme or URL shape is not allowed")
    }
}

fn relative_url_allowed(value: &str) -> bool {
    if value.starts_with('#') {
        return false;
    }
    let scheme_boundary = value.find(['/', '?', '#']).unwrap_or(value.len());
    !value[..scheme_boundary].contains(':')
}

/// Validates one `srcset` value as a bounded candidate list.
///
/// Every candidate URL passes the same rule as an image `src`
/// ([`UrlKind::ResourceImage`]) except that `data:` payloads are refused: their
/// base64 bodies contain commas, which collide with the candidate grammar. The
/// per-candidate check is the safety boundary; the counts here are only limits.
fn validate_srcset(
    value: &str,
    policy: &PageBuilderStaticPublishPolicy,
) -> Result<(), &'static str> {
    let mut candidates = 0usize;
    let mut descriptors = BTreeSet::new();
    let mut family: Option<&'static str> = None;
    for raw_part in value.split(',') {
        let part = raw_part.trim();
        if part.is_empty() {
            return Err("value contains an empty candidate");
        }
        candidates += 1;
        if candidates > policy.max_srcset_candidates {
            return Err("value exceeds the candidate limit");
        }
        let mut tokens = part.split_whitespace();
        let Some(url) = tokens.next() else {
            return Err("candidate URL is empty");
        };
        let descriptor = tokens.next();
        if tokens.next().is_some() {
            return Err("candidate has unexpected trailing tokens");
        }
        if url.starts_with("data:") {
            return Err("data: URLs are not allowed in srcset candidates");
        }
        if let Err(reason) = validate_url(url, UrlKind::ResourceImage, policy) {
            return Err(reason);
        }
        let (candidate_family, normalized) = match descriptor {
            None => ("x", "1x".to_string()),
            Some(descriptor) => {
                let family = parse_srcset_descriptor(descriptor)
                    .ok_or("candidate descriptor must be <N>w or <N>x")?;
                (family, descriptor.to_string())
            }
        };
        match family {
            None => family = Some(candidate_family),
            Some(seen) if seen == candidate_family => {}
            Some(_) => return Err("width and density descriptors must not be mixed"),
        }
        if !descriptors.insert(normalized) {
            return Err("candidate descriptors must be unique");
        }
    }
    if candidates == 0 {
        return Err("value is empty");
    }
    Ok(())
}

/// Returns the descriptor family (`w` or `x`) for a syntactically valid descriptor.
fn parse_srcset_descriptor(descriptor: &str) -> Option<&'static str> {
    if let Some(rest) = descriptor.strip_suffix('w') {
        if !rest.is_empty()
            && rest.len() <= 6
            && rest.bytes().all(|byte| byte.is_ascii_digit())
            && rest.parse::<u32>().is_ok_and(|width| width >= 1)
        {
            return Some("w");
        }
        return None;
    }
    let rest = descriptor.strip_suffix('x')?;
    let (whole, fraction) = match rest.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (rest, None),
    };
    if whole.is_empty() || whole.len() > 3 || !whole.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if let Some(fraction) = fraction {
        if fraction.is_empty()
            || fraction.len() > 3
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
    }
    Some("x")
}

/// Validates one `sizes` value as a bounded list of `[media-condition ]<length>`.
///
/// The grammar is a deliberately strict subset of the CSS `sizes` attribute:
/// parenthesized media-condition groups joined by `and`/`or`, and one unit length
/// per entry. Anything outside the subset is rejected rather than guessed at.
fn validate_sizes(
    value: &str,
    policy: &PageBuilderStaticPublishPolicy,
) -> Result<(), &'static str> {
    let mut entries = 0usize;
    for raw_part in value.split(',') {
        let part = raw_part.trim();
        if part.is_empty() {
            return Err("value contains an empty entry");
        }
        entries += 1;
        if entries > policy.max_sizes_entries {
            return Err("value exceeds the entry limit");
        }
        validate_sizes_entry(part, policy)?;
    }
    if entries == 0 {
        return Err("value is empty");
    }
    Ok(())
}

fn validate_sizes_entry(
    entry: &str,
    policy: &PageBuilderStaticPublishPolicy,
) -> Result<(), &'static str> {
    let split_at = entry.rfind(char::is_whitespace);
    let (condition, length) = match split_at {
        Some(index) => (entry[..index].trim(), entry[index..].trim()),
        None => ("", entry),
    };
    if length.is_empty() {
        return Err("entry is missing its source size");
    }
    if !is_sizes_length(length) {
        return Err("entry source size must be a number with a length unit");
    }
    validate_sizes_condition(condition, policy)
}

fn is_sizes_length(value: &str) -> bool {
    const UNITS: &[&str] = &["px", "em", "rem", "vw", "vh", "ch", "vmin", "vmax", "%"];
    for unit in UNITS {
        if let Some(rest) = value.strip_suffix(unit) {
            return is_sizes_number(rest);
        }
    }
    false
}

fn is_sizes_number(value: &str) -> bool {
    let (whole, fraction) = match value.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (value, None),
    };
    if whole.is_empty() || whole.len() > 6 || !whole.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    match fraction {
        Some(fraction) => {
            !fraction.is_empty()
                && fraction.len() <= 3
                && fraction.bytes().all(|byte| byte.is_ascii_digit())
        }
        None => true,
    }
}

fn validate_sizes_condition(
    condition: &str,
    policy: &PageBuilderStaticPublishPolicy,
) -> Result<(), &'static str> {
    if condition.is_empty() {
        return Ok(());
    }
    if condition.len() > policy.max_media_query_bytes {
        return Err("media condition exceeds the byte limit");
    }
    let mut rest = condition.trim();
    if let Some(tail) = rest.strip_prefix("not") {
        if !tail.starts_with(|character: char| character.is_whitespace()) {
            return Err("media condition `not` must be followed by whitespace");
        }
        rest = tail.trim_start();
    }
    loop {
        let Some(after_open) = rest.strip_prefix('(') else {
            return Err("media condition groups must be parenthesized");
        };
        let Some(close_offset) = after_open.find(')') else {
            return Err("media condition has an unbalanced group");
        };
        validate_sizes_condition_group(&after_open[..close_offset])?;
        let tail = after_open[close_offset + 1..].trim_start();
        if tail.is_empty() {
            return Ok(());
        }
        let (word, after_word) = split_leading_word(tail);
        if word != "and" && word != "or" {
            return Err("media condition groups must be joined by `and` or `or`");
        }
        rest = after_word.trim_start();
        if rest.is_empty() {
            return Err("media condition must not end with a joiner");
        }
    }
}

fn split_leading_word(value: &str) -> (&str, &str) {
    let end = value
        .find(|character: char| !character.is_ascii_lowercase())
        .unwrap_or(value.len());
    (&value[..end], &value[end..])
}

fn validate_sizes_condition_group(inner: &str) -> Result<(), &'static str> {
    let inner = inner.trim();
    if inner.is_empty() {
        return Err("media condition group is empty");
    }
    let (feature, value) = match inner.split_once(':') {
        Some((feature, value)) => (feature.trim(), Some(value.trim())),
        None => (inner, None),
    };
    if feature.is_empty()
        || feature.len() > 32
        || !feature.starts_with(|character: char| character.is_ascii_lowercase())
        || !feature
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("media condition feature name is not allowed");
    }
    if let Some(value) = value {
        validate_sizes_condition_value(value)?;
    }
    Ok(())
}

fn validate_sizes_condition_value(value: &str) -> Result<(), &'static str> {
    const UNITS: &[&str] = &["px", "em", "rem", "%", "vw", "vh", "ch", "dpi"];
    if value.is_empty() {
        return Err("media condition value is empty");
    }
    for unit in UNITS {
        if let Some(rest) = value.strip_suffix(unit) {
            if is_sizes_number(rest) {
                return Ok(());
            }
            return Err("media condition numeric value is invalid");
        }
    }
    if value.bytes().all(|byte| byte.is_ascii_lowercase() || byte == b'-')
        && value.starts_with(|character: char| character.is_ascii_lowercase())
    {
        return Ok(());
    }
    Err("media condition value must be a keyword or a number with a known unit")
}

fn safe_css_value(value: &str, policy: &PageBuilderStaticPublishPolicy) -> bool {
    if value.chars().any(char::is_control) {
        return false;
    }

    // An operator who keeps `url(` in the configured list still gets the old blanket ban:
    // explicit configuration outranks the default.
    let bans_all_references = policy
        .forbidden_css_tokens
        .iter()
        .any(|token| token == "url(");
    let scanned = if bans_all_references {
        value.to_string()
    } else {
        // Validate and blank out references *before* the delimiter checks. A permitted
        // `data:image/png;base64,...` legally contains `;`, which the check below would
        // otherwise reject — the same ordering trap `fly::render::safe_style` had to fix.
        match strip_validated_css_urls(value, policy) {
            Some(stripped) => stripped,
            None => return false,
        }
    };

    let compact = scanned
        .to_ascii_lowercase()
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    if policy
        .forbidden_css_tokens
        .iter()
        .any(|token| compact.contains(token))
    {
        return false;
    }
    // A reference that survived stripping is malformed or disguised (`blurl(`, `u r l(`).
    if compact.contains("url(") {
        return false;
    }
    !scanned.contains('\\')
        && !scanned.contains('<')
        && !scanned.contains('>')
        && !scanned.contains(';')
        && !scanned.contains('{')
        && !scanned.contains('}')
        && !scanned.contains("/*")
        && !scanned.contains("*/")
}

/// Replace each well-formed `url(...)` with a placeholder, rejecting the declaration when any
/// referenced URL fails the image-resource rule this policy already applies to `src`.
fn strip_validated_css_urls(
    value: &str,
    policy: &PageBuilderStaticPublishPolicy,
) -> Option<String> {
    let lowered = value.to_ascii_lowercase();
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0usize;

    while let Some(offset) = lowered[cursor..].find("url(") {
        let start = cursor + offset;
        // `blurl(` is not a reference; leave it in place so the caller's scan rejects it.
        let preceded_by_identifier = value[..start]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '-');
        if preceded_by_identifier {
            output.push_str(&value[cursor..start + 4]);
            cursor = start + 4;
            continue;
        }

        let open = start + 4;
        // An unterminated reference is refused rather than guessed at.
        let close = value[open..].find(')')? + open;
        let raw = value[open..close].trim();
        let unquoted = raw
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .or_else(|| {
                raw.strip_prefix('\'')
                    .and_then(|rest| rest.strip_suffix('\''))
            })
            .unwrap_or(raw);

        // `;` is legal inside a data URL, but these would still let a reference escape the
        // declaration or open a comment.
        if unquoted.is_empty()
            || unquoted.contains('{')
            || unquoted.contains('}')
            || unquoted.contains('<')
            || unquoted.contains('>')
            || unquoted.contains("/*")
            || unquoted.contains("*/")
            || validate_url(unquoted, UrlKind::ResourceImage, policy).is_err()
        {
            return None;
        }

        output.push_str(&value[cursor..start]);
        output.push_str("url-ok");
        cursor = close + 1;
    }

    output.push_str(&value[cursor..]);
    Some(output)
}

fn safe_media_query(query: &str, policy: &PageBuilderStaticPublishPolicy) -> bool {
    let normalized = query.trim().to_ascii_lowercase();
    !normalized.is_empty()
        && normalized.len() <= policy.max_media_query_bytes
        && !normalized.contains('{')
        && !normalized.contains('}')
        && !normalized.contains(';')
        && !normalized.contains("url(")
        && !normalized.contains("expression(")
        && !normalized.contains("@import")
        && !normalized.contains('\\')
        && normalized
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "() :.-_%/,".contains(character))
}

fn scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn localized_string_values(value: &Value) -> Vec<(String, Option<&str>)> {
    if let Some(value) = value.as_str() {
        return vec![(String::new(), Some(value))];
    }
    let Some(values) = value
        .as_object()
        .and_then(|wrapper| wrapper.get("$localized"))
        .and_then(Value::as_object)
    else {
        return vec![(String::new(), None)];
    };
    if values.is_empty() {
        return vec![(".$localized".to_string(), None)];
    }
    let mut values = values
        .iter()
        .map(|(locale, value)| (format!(".$localized.{locale}"), value.as_str()))
        .collect::<Vec<_>>();
    values.sort_by(|left, right| left.0.cmp(&right.0));
    values
}

fn contains_tag_like_markup(value: &str) -> bool {
    let bytes = value.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'<' {
            continue;
        }
        let Some(next) = bytes.get(index + 1).copied() else {
            continue;
        };
        if (next.is_ascii_alphabetic() || matches!(next, b'/' | b'!' | b'?'))
            && bytes[index + 1..].contains(&b'>')
        {
            return true;
        }
    }
    false
}

fn contains_disallowed_control(value: &str) -> bool {
    value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

fn reject(
    diagnostics: &mut Vec<PageBuilderStaticPublishPolicyDiagnostic>,
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    diagnostics.push(PageBuilderStaticPublishPolicyDiagnostic {
        code: code.into(),
        path: path.into(),
        message: message.into(),
    });
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn require_normalized_unique(
    values: &[String],
    field: &str,
) -> Result<(), PageBuilderStaticPublishPolicyError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if value.is_empty() || value != &value.trim().to_ascii_lowercase() {
            return Err(PageBuilderStaticPublishPolicyError::Integrity(format!(
                "static publish policy `{field}` contains a non-normalized value"
            )));
        }
        if !seen.insert(value.as_str()) {
            return Err(PageBuilderStaticPublishPolicyError::Integrity(format!(
                "static publish policy `{field}` contains a duplicate value"
            )));
        }
    }
    Ok(())
}

fn stable_hash(value: &impl Serialize) -> Result<String, PageBuilderStaticPublishPolicyError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| PageBuilderStaticPublishPolicyError::Encode(error.to_string()))?;
    Ok(hex_sha256(&bytes))
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::GrapesJsCodec;
    use serde_json::json;

    fn document(component: Value) -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": "Home",
                    "slug": "home",
                    "canonical_url": "/home",
                    "open_graph_image": "https://cdn.example.com/og.webp"
                },
                "component": component
            }]
        }))
        .expect("document")
    }

    #[test]
    fn policy_accepts_fly_link_components_and_has_stable_evidence() {
        let document = document(json!({
            "id": "root",
            "type": "wrapper",
            "components": [{
                "id": "link",
                "type": "link",
                "tagName": "a",
                "attributes": { "href": "mailto:hello@example.com", "rel": "noopener" },
                "style": { "margin-top": "12px" },
                "content": "Contact us"
            }]
        }));
        let first = validate_static_publish_document(&document).expect("policy evidence");
        let second = validate_static_publish_document(&document).expect("policy evidence");
        assert_eq!(first, second);
        first.verify_integrity().expect("evidence integrity");
    }

    #[test]
    fn policy_rejects_event_handlers_javascript_urls_css_urls_and_false_attributes() {
        let document = document(json!({
            "id": "root",
            "type": "wrapper",
            "components": [{
                "id": "link",
                "type": "link",
                "tagName": "a",
                "attributes": {
                    "onclick": "alert(1)",
                    "href": "javascript:alert(1)",
                    "hidden": false
                },
                "style": { "background-image": "url(javascript:alert(1))" },
                "content": "Safe text"
            }]
        }));
        let error = validate_static_publish_document(&document).expect_err("unsafe project");
        let codes = error
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("landing_event_handler_forbidden"));
        assert!(codes.contains("landing_url_rejected"));
        assert!(codes.contains("landing_css_value_rejected"));
        assert!(codes.contains("landing_false_boolean_attribute_omitted"));
    }

    #[test]
    fn policy_rejects_opaque_markup_and_non_renderable_nodes() {
        let document = document(json!({
            "id": "root",
            "type": "wrapper",
            "components": [
                "Hello <strong>world</strong>",
                { "unexpected": true },
                null
            ]
        }));
        let error = validate_static_publish_document(&document).expect_err("opaque project");
        let codes = error
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("landing_content_markup_forbidden"));
        assert!(codes.contains("landing_opaque_node_not_renderable"));
    }

    #[test]
    fn policy_rejects_empty_localized_metadata_urls() {
        let mut document = document(json!({ "id": "root", "type": "wrapper" }));
        document.project.pages[0]
            .extensions
            .get_mut(FLY_PAGE_METADATA_FIELD)
            .and_then(Value::as_object_mut)
            .expect("metadata")
            .insert("canonical_url".to_string(), json!({ "$localized": {} }));
        let error = validate_static_publish_document(&document).expect_err("empty localized URL");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "landing_metadata_url_invalid")
        );
    }

    #[test]
    fn css_references_follow_the_same_rule_as_image_attributes() {
        // The point of the change: the same image must not be accepted through `src` and refused
        // through `background-image`.
        let policy = PageBuilderStaticPublishPolicy::default();
        for url in [
            "/hero.png",
            "https://cdn.example/hero.png",
            "data:image/png;base64,iVBORw0KGgo=",
        ] {
            assert!(
                validate_url(url, UrlKind::ResourceImage, &policy).is_ok(),
                "attribute rule rejected {url}"
            );
            assert!(
                safe_css_value(&format!("url({url})"), &policy),
                "css rule rejected {url} that the attribute rule accepts"
            );
        }
    }

    #[test]
    fn hostile_css_references_are_still_refused() {
        let policy = PageBuilderStaticPublishPolicy::default();
        for value in [
            "url(javascript:alert(1))",
            "url(http://insecure.example/a.png)",
            "url(//protocol.relative/a.png)",
            "url(data:text/html;base64,PHNjcmlwdD4=)",
            "url()",
            "url(/unterminated.png",
            "url(/a.png);color:red",
            "u r l(https://cdn.example/a.png)",
            "blurl(https://cdn.example/a.png)",
        ] {
            assert!(!safe_css_value(value, &policy), "accepted hostile {value}");
        }
    }

    #[test]
    fn an_operator_may_keep_the_blanket_reference_ban() {
        // Explicit configuration outranks the coherent default.
        let mut policy = PageBuilderStaticPublishPolicy::default();
        policy.forbidden_css_tokens.push("url(".to_string());
        assert!(!safe_css_value(
            "url(https://cdn.example/hero.png)",
            &policy
        ));
        assert!(safe_css_value("#ffffff", &policy));
    }

    #[test]
    fn ordinary_declarations_are_unaffected() {
        let policy = PageBuilderStaticPublishPolicy::default();
        for value in [
            "#ffffff",
            "12px",
            "1px solid #333",
            "linear-gradient(red, blue)",
        ] {
            assert!(safe_css_value(value, &policy), "rejected {value}");
        }
        for value in ["expression(alert(1))", "@import 'x'", "color:red;}", "a<b"] {
            assert!(!safe_css_value(value, &policy), "accepted {value}");
        }
    }

    #[test]
    fn validated_srcset_and_sizes_pass_the_static_policy() {
        let policy = PageBuilderStaticPublishPolicy::default();
        assert!(validate_srcset(
            "https://cdn.example.com/hero-480.webp 480w, https://cdn.example.com/hero-960.webp 960w, /hero.webp 1200w",
            &policy
        )
        .is_ok());
        assert!(validate_srcset("hero.webp 1x, hero@2x.webp 2x", &policy).is_ok());
        assert!(validate_srcset("/hero.webp", &policy).is_ok(), "bare candidate");
        assert!(validate_sizes("(min-width: 480px) 45vw, 100vw", &policy).is_ok());
        assert!(validate_sizes("(orientation: landscape) and (min-width: 40em) 300px, 100vw", &policy)
            .is_ok());
        assert!(validate_sizes("100vw", &policy).is_ok(), "condition-free entry");

        let document = document(json!({
            "id": "root",
            "type": "wrapper",
            "components": [{
                "id": "hero",
                "type": "image",
                "tagName": "img",
                "attributes": {
                    "src": "https://cdn.example.com/hero.webp",
                    "srcset": "https://cdn.example.com/hero-480.webp 480w, https://cdn.example.com/hero-960.webp 960w",
                    "sizes": "(min-width: 480px) 45vw, 100vw",
                    "alt": "Hero"
                }
            }]
        }));
        validate_static_publish_document(&document).expect("responsive image document");
    }

    #[test]
    fn srcset_rejects_unsafe_and_malformed_candidates() {
        let policy = PageBuilderStaticPublishPolicy::default();
        for value in [
            "javascript:alert(1) 1x",
            "data:image/png;base64,AAAA 1x",
            "https://cdn.example.com/hero.webp 480w, javascript:alert(1) 960w",
            "https://cdn.example.com/a.webp 480w, https://cdn.example.com/b.webp 480w",
            "https://cdn.example.com/a.webp 480w, https://cdn.example.com/b.webp 2x",
            "https://cdn.example.com/a.webp 480w, https://cdn.example.com/b.webp",
            "https://cdn.example.com/hero.webp 0w",
            "https://cdn.example.com/hero.webp 1.5w",
            "https://cdn.example.com/hero.webp 1x 2x",
            "https://cdn.example.com/hero.webp 1e2x",
            "",
            ", https://cdn.example.com/hero.webp 1x",
            "//cdn.example.com/hero.webp 1x",
        ] {
            assert!(
                validate_srcset(value, &policy).is_err(),
                "accepted hostile srcset {value}"
            );
        }
        let mut bounded = PageBuilderStaticPublishPolicy::default();
        bounded.max_srcset_candidates = 1;
        assert!(validate_srcset("/a.webp 1x, /b.webp 2x", &bounded).is_err());
    }

    #[test]
    fn sizes_rejects_unsafe_and_malformed_values() {
        let policy = PageBuilderStaticPublishPolicy::default();
        for value in [
            "calc(100% - 20px)",
            "min-width: 480px 300px",
            "(min-width: 480px) auto",
            "(min-width: 480px) 300pt",
            "(min-width: 480px",
            "min-width: 480px) 300px",
            "() 300px",
            "(min-width: 480px) and 300px",
            "(min-width: 480px) and",
            "not screen (min-width: 480px) 300px",
            "(min-width: expression(alert(1))) 300px",
            "100vw 200px",
            "",
        ] {
            assert!(
                validate_sizes(value, &policy).is_err(),
                "accepted hostile sizes {value}"
            );
        }
        let mut bounded = PageBuilderStaticPublishPolicy::default();
        bounded.max_sizes_entries = 1;
        assert!(validate_sizes("100vw, 50vw", &bounded).is_err());
    }

    #[test]
    fn an_operator_may_keep_the_blanket_srcset_ban() {
        // Explicit configuration outranks the coherent default.
        let mut policy = PageBuilderStaticPublishPolicy::default();
        policy.forbidden_attributes.push("srcset".to_string());
        assert!(
            validate_srcset(
                "https://cdn.example.com/hero-480.webp 480w",
                &PageBuilderStaticPublishPolicy::default()
            )
            .is_ok(),
            "the coherent default validates the same value"
        );
        let mut diagnostics = Vec::new();
        let attributes = json!({
            "src": "https://cdn.example.com/hero.webp",
            "srcset": "https://cdn.example.com/hero-480.webp 480w"
        })
        .as_object()
        .cloned()
        .expect("attributes");
        for (raw_name, value) in &attributes {
            validate_attribute(raw_name, value, "attributes.test", &policy, &mut diagnostics);
        }
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "landing_attribute_forbidden"),
            "operator blanket ban must refuse srcset"
        );
    }

    #[test]
    fn srcset_rejections_carry_dedicated_diagnostics() {
        let document = document(json!({
            "id": "root",
            "type": "wrapper",
            "components": [{
                "id": "hero",
                "type": "image",
                "tagName": "img",
                "attributes": {
                    "src": "https://cdn.example.com/hero.webp",
                    "srcset": "javascript:alert(1) 1x",
                    "sizes": "calc(100% - 20px)"
                }
            }]
        }));
        let error = validate_static_publish_document(&document).expect_err("unsafe responsive image");
        let codes = error
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("landing_srcset_rejected"));
        assert!(codes.contains("landing_sizes_rejected"));
    }
}
