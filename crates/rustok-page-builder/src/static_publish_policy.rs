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
    "srcset",
    "style",
    "xlink:href",
];

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

const FORBIDDEN_CSS_TOKENS: &[&str] = &[
    "-moz-binding",
    "@import",
    "behavior:",
    "data:",
    "expression(",
    "javascript:",
    "url(",
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
        let kind = UrlKind::for_attribute(&name).expect("validated policy URL attribute kind");
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

        let object = raw.as_object().expect("parsed style rule object");
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

fn safe_css_value(value: &str, policy: &PageBuilderStaticPublishPolicy) -> bool {
    let normalized = value.to_ascii_lowercase();
    let compact = normalized
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    !policy
        .forbidden_css_tokens
        .iter()
        .any(|token| compact.contains(token))
        && !value.contains('\\')
        && !value.contains('<')
        && !value.contains('>')
        && !value.contains(';')
        && !value.contains('{')
        && !value.contains('}')
        && !value.contains("/*")
        && !value.contains("*/")
        && !value.chars().any(char::is_control)
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
                "style": { "background-image": "url(https://evil.example/a.png)" },
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
}
