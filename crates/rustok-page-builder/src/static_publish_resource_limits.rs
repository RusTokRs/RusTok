use fly::{ComponentChildren, ComponentNode, ProjectDocument};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Write};

pub const PAGE_BUILDER_STATIC_PUBLISH_RESOURCE_LIMITS_FORMAT: &str =
    "page_builder_static_publish_resource_limits_v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageBuilderStaticPublishResourceLimits {
    pub format: String,
    pub max_project_bytes: usize,
    pub max_pages: usize,
    pub max_components: usize,
    pub max_component_depth: usize,
    pub max_assets: usize,
    pub max_style_rules: usize,
}

impl Default for PageBuilderStaticPublishResourceLimits {
    fn default() -> Self {
        Self {
            format: PAGE_BUILDER_STATIC_PUBLISH_RESOURCE_LIMITS_FORMAT.to_string(),
            max_project_bytes: 16 * 1024 * 1024,
            max_pages: 128,
            max_components: 50_000,
            max_component_depth: 128,
            max_assets: 4_096,
            max_style_rules: 20_000,
        }
    }
}

impl PageBuilderStaticPublishResourceLimits {
    pub fn verify_integrity(&self) -> Result<(), PageBuilderStaticPublishResourceLimitError> {
        if self.format != PAGE_BUILDER_STATIC_PUBLISH_RESOURCE_LIMITS_FORMAT {
            return Err(PageBuilderStaticPublishResourceLimitError::Integrity(
                "unsupported static publish resource-limit format".to_string(),
            ));
        }
        if self.max_project_bytes == 0
            || self.max_pages == 0
            || self.max_components == 0
            || self.max_component_depth == 0
            || self.max_assets == 0
            || self.max_style_rules == 0
        {
            return Err(PageBuilderStaticPublishResourceLimitError::Integrity(
                "static publish resource limits must be positive".to_string(),
            ));
        }
        Ok(())
    }

    pub fn limits_hash(&self) -> Result<String, PageBuilderStaticPublishResourceLimitError> {
        self.verify_integrity()?;
        stable_hash(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageBuilderStaticPublishResourceObservation {
    pub project_bytes: usize,
    pub page_count: usize,
    pub component_count: usize,
    pub max_component_depth: usize,
    pub asset_count: usize,
    pub style_rule_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageBuilderStaticPublishResourceEvidence {
    pub format: String,
    pub limits_hash: String,
    pub observed: PageBuilderStaticPublishResourceObservation,
}

impl PageBuilderStaticPublishResourceEvidence {
    pub fn verify_integrity(&self) -> Result<(), PageBuilderStaticPublishResourceLimitError> {
        let limits = PageBuilderStaticPublishResourceLimits::default();
        limits.verify_integrity()?;
        if self.format != limits.format {
            return Err(PageBuilderStaticPublishResourceLimitError::Integrity(
                "static publish resource evidence format mismatch".to_string(),
            ));
        }
        if !is_sha256(&self.limits_hash) || self.limits_hash != limits.limits_hash()? {
            return Err(PageBuilderStaticPublishResourceLimitError::Integrity(
                "static publish resource evidence hash mismatch".to_string(),
            ));
        }
        if self.observed.project_bytes > limits.max_project_bytes
            || self.observed.page_count > limits.max_pages
            || self.observed.component_count > limits.max_components
            || self.observed.max_component_depth > limits.max_component_depth
            || self.observed.asset_count > limits.max_assets
            || self.observed.style_rule_count > limits.max_style_rules
        {
            return Err(PageBuilderStaticPublishResourceLimitError::Integrity(
                "static publish resource evidence exceeds the bound policy".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageBuilderStaticPublishResourceDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PageBuilderStaticPublishResourceLimitError {
    #[error("static publish resource-limit encoding failed: {0}")]
    Encode(String),
    #[error("static publish resource-limit integrity failed: {0}")]
    Integrity(String),
    #[error("static publish resource limits rejected project")]
    Rejected {
        diagnostics: Vec<PageBuilderStaticPublishResourceDiagnostic>,
    },
}

impl PageBuilderStaticPublishResourceLimitError {
    pub fn diagnostics(&self) -> &[PageBuilderStaticPublishResourceDiagnostic] {
        match self {
            Self::Rejected { diagnostics } => diagnostics,
            Self::Encode(_) | Self::Integrity(_) => &[],
        }
    }
}

pub fn validate_static_publish_resource_limits(
    document: &ProjectDocument,
) -> Result<PageBuilderStaticPublishResourceEvidence, PageBuilderStaticPublishResourceLimitError> {
    let limits = PageBuilderStaticPublishResourceLimits::default();
    limits.verify_integrity()?;

    let project_bytes = serialized_project_bytes(document, limits.max_project_bytes)?;
    let page_count = document.project.pages.len();
    let asset_count = document.project.assets.len();
    let style_rule_count = document.project.styles.len();
    let (component_count, max_component_depth) = component_observation(document, &limits);
    let observed = PageBuilderStaticPublishResourceObservation {
        project_bytes,
        page_count,
        component_count,
        max_component_depth,
        asset_count,
        style_rule_count,
    };

    let mut diagnostics = Vec::new();
    reject_excess(
        &mut diagnostics,
        observed.project_bytes,
        limits.max_project_bytes,
        "landing_project_bytes_exceeded",
        "project",
        "serialized project bytes",
    );
    reject_excess(
        &mut diagnostics,
        observed.page_count,
        limits.max_pages,
        "landing_page_count_exceeded",
        "pages",
        "page count",
    );
    reject_excess(
        &mut diagnostics,
        observed.component_count,
        limits.max_components,
        "landing_component_count_exceeded",
        "pages[].component",
        "component count",
    );
    reject_excess(
        &mut diagnostics,
        observed.max_component_depth,
        limits.max_component_depth,
        "landing_component_depth_exceeded",
        "pages[].component",
        "component depth",
    );
    reject_excess(
        &mut diagnostics,
        observed.asset_count,
        limits.max_assets,
        "landing_asset_count_exceeded",
        "assets",
        "asset count",
    );
    reject_excess(
        &mut diagnostics,
        observed.style_rule_count,
        limits.max_style_rules,
        "landing_style_rule_count_exceeded",
        "styles",
        "style-rule count",
    );

    if !diagnostics.is_empty() {
        return Err(PageBuilderStaticPublishResourceLimitError::Rejected { diagnostics });
    }

    Ok(PageBuilderStaticPublishResourceEvidence {
        format: limits.format.clone(),
        limits_hash: limits.limits_hash()?,
        observed,
    })
}

fn serialized_project_bytes(
    document: &ProjectDocument,
    maximum: usize,
) -> Result<usize, PageBuilderStaticPublishResourceLimitError> {
    let mut counter = BoundedByteCounter::new(maximum);
    match serde_json::to_writer(&mut counter, &document.project) {
        Ok(()) => Ok(counter.bytes),
        Err(_) if counter.exceeded => Ok(maximum.saturating_add(1)),
        Err(error) => Err(PageBuilderStaticPublishResourceLimitError::Encode(
            error.to_string(),
        )),
    }
}

struct BoundedByteCounter {
    bytes: usize,
    maximum: usize,
    exceeded: bool,
}

impl BoundedByteCounter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: 0,
            maximum,
            exceeded: false,
        }
    }
}

impl Write for BoundedByteCounter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let next = self.bytes.saturating_add(buffer.len());
        if next > self.maximum {
            self.exceeded = true;
            return Err(io::Error::other(
                "static publish project byte limit exceeded",
            ));
        }
        self.bytes = next;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn component_observation(
    document: &ProjectDocument,
    limits: &PageBuilderStaticPublishResourceLimits,
) -> (usize, usize) {
    let mut component_count = 0usize;
    let mut max_component_depth = 0usize;

    for page in &document.project.pages {
        let Some(root) = page.component.as_ref() else {
            continue;
        };
        let mut stack = vec![(root, 1usize)];
        while let Some((node, depth)) = stack.pop() {
            component_count = component_count.saturating_add(1);
            max_component_depth = max_component_depth.max(depth);
            if component_count > limits.max_components
                || max_component_depth > limits.max_component_depth
            {
                return (component_count, max_component_depth);
            }
            if let ComponentNode::Object(component) = node
                && let ComponentChildren::Nodes(children) = &component.components
            {
                for child in children.iter().rev() {
                    stack.push((child, depth.saturating_add(1)));
                }
            }
        }
    }

    (component_count, max_component_depth)
}

fn reject_excess(
    diagnostics: &mut Vec<PageBuilderStaticPublishResourceDiagnostic>,
    observed: usize,
    maximum: usize,
    code: &str,
    path: &str,
    label: &str,
) {
    if observed > maximum {
        diagnostics.push(PageBuilderStaticPublishResourceDiagnostic {
            code: code.to_string(),
            path: path.to_string(),
            message: format!("{label} {observed} exceeds reviewed maximum {maximum}"),
        });
    }
}

fn stable_hash(
    value: &impl Serialize,
) -> Result<String, PageBuilderStaticPublishResourceLimitError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| PageBuilderStaticPublishResourceLimitError::Encode(error.to_string()))?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::GrapesJsCodec;
    use serde_json::{Value, json};

    fn document(project: Value) -> ProjectDocument {
        GrapesJsCodec::decode_value(project).expect("project document")
    }

    #[test]
    fn resource_evidence_is_stable_and_policy_bound() {
        let document = document(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "title", "type": "text", "content": "Home" }]
                }
            }],
            "assets": [{ "id": "hero", "src": "/hero.webp", "type": "image" }],
            "styles": [{ "selectors": ["#title"], "style": { "color": "black" } }]
        }));

        let first = validate_static_publish_resource_limits(&document).expect("resource evidence");
        let second = validate_static_publish_resource_limits(&document).expect("resource evidence");
        assert_eq!(first, second);
        assert_eq!(first.observed.page_count, 1);
        assert_eq!(first.observed.component_count, 2);
        assert_eq!(first.observed.max_component_depth, 2);
        first
            .verify_integrity()
            .expect("resource evidence integrity");
    }

    #[test]
    fn resource_limits_reject_excess_pages() {
        let pages = (0..=PageBuilderStaticPublishResourceLimits::default().max_pages)
            .map(|index| json!({ "id": format!("page-{index}") }))
            .collect::<Vec<_>>();
        let document = document(json!({ "pages": pages }));
        let error =
            validate_static_publish_resource_limits(&document).expect_err("page limit rejection");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "landing_page_count_exceeded")
        );
    }

    #[test]
    fn resource_limits_reject_excess_component_depth() {
        let limit = PageBuilderStaticPublishResourceLimits::default().max_component_depth;
        let mut component = json!({ "id": "leaf", "type": "text", "content": "leaf" });
        for index in 0..limit {
            component = json!({
                "id": format!("node-{index}"),
                "type": "wrapper",
                "components": [component]
            });
        }
        let document = document(json!({
            "pages": [{ "id": "home", "component": component }]
        }));
        let error =
            validate_static_publish_resource_limits(&document).expect_err("depth limit rejection");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "landing_component_depth_exceeded")
        );
    }
}
