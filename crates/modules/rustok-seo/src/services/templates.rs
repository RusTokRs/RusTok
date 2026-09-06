use crate::dto::{SeoFieldSource, SeoMetaTranslationRecord, SeoTemplateRuleSet};

use super::TargetState;

#[derive(Debug, Clone, Default)]
pub(super) struct GeneratedSeoRecord {
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical_url: Option<String>,
    pub keywords: Option<String>,
    pub robots: Option<Vec<String>>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub twitter_title: Option<String>,
    pub twitter_description: Option<String>,
}

pub(super) fn normalize_rule_set(mut rules: SeoTemplateRuleSet) -> SeoTemplateRuleSet {
    rules.title = normalize_template(rules.title);
    rules.meta_description = normalize_template(rules.meta_description);
    rules.canonical_url = normalize_template(rules.canonical_url);
    rules.keywords = normalize_template(rules.keywords);
    rules.robots = normalize_template(rules.robots);
    rules.open_graph_title = normalize_template(rules.open_graph_title);
    rules.open_graph_description = normalize_template(rules.open_graph_description);
    rules.twitter_title = normalize_template(rules.twitter_title);
    rules.twitter_description = normalize_template(rules.twitter_description);
    rules
}

pub(super) fn render_generated_record(
    state: &TargetState,
    default_rules: &SeoTemplateRuleSet,
    override_rules: Option<&SeoTemplateRuleSet>,
) -> GeneratedSeoRecord {
    let rules = merge_rules(default_rules, override_rules);
    GeneratedSeoRecord {
        title: render_template(rules.title.as_deref(), state),
        description: render_template(rules.meta_description.as_deref(), state),
        canonical_url: render_template(rules.canonical_url.as_deref(), state)
            .map(normalize_canonical_value),
        keywords: render_template(rules.keywords.as_deref(), state),
        robots: render_template(rules.robots.as_deref(), state).map(parse_robot_tokens),
        og_title: render_template(rules.open_graph_title.as_deref(), state),
        og_description: render_template(rules.open_graph_description.as_deref(), state),
        twitter_title: render_template(rules.twitter_title.as_deref(), state),
        twitter_description: render_template(rules.twitter_description.as_deref(), state),
    }
}

pub(super) fn generated_translation(
    record: &GeneratedSeoRecord,
    locale: String,
) -> SeoMetaTranslationRecord {
    SeoMetaTranslationRecord {
        locale,
        title: record.title.clone(),
        description: record.description.clone(),
        keywords: record.keywords.clone(),
        og_title: record.og_title.clone(),
        og_description: record.og_description.clone(),
        og_image: None,
    }
}

pub(super) fn source_label(source: SeoFieldSource, fallback_source: &str) -> String {
    match source {
        SeoFieldSource::Explicit => "explicit".to_string(),
        SeoFieldSource::Generated => "generated".to_string(),
        SeoFieldSource::Fallback => format!("{fallback_source}_fallback"),
    }
}

fn merge_rules(
    defaults: &SeoTemplateRuleSet,
    overrides: Option<&SeoTemplateRuleSet>,
) -> SeoTemplateRuleSet {
    let mut merged = defaults.clone();
    if let Some(overrides) = overrides {
        if overrides.title.is_some() {
            merged.title = overrides.title.clone();
        }
        if overrides.meta_description.is_some() {
            merged.meta_description = overrides.meta_description.clone();
        }
        if overrides.canonical_url.is_some() {
            merged.canonical_url = overrides.canonical_url.clone();
        }
        if overrides.keywords.is_some() {
            merged.keywords = overrides.keywords.clone();
        }
        if overrides.robots.is_some() {
            merged.robots = overrides.robots.clone();
        }
        if overrides.open_graph_title.is_some() {
            merged.open_graph_title = overrides.open_graph_title.clone();
        }
        if overrides.open_graph_description.is_some() {
            merged.open_graph_description = overrides.open_graph_description.clone();
        }
        if overrides.twitter_title.is_some() {
            merged.twitter_title = overrides.twitter_title.clone();
        }
        if overrides.twitter_description.is_some() {
            merged.twitter_description = overrides.twitter_description.clone();
        }
    }
    merged
}

fn normalize_template(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn render_template(template: Option<&str>, state: &TargetState) -> Option<String> {
    let template = template.map(str::trim).filter(|value| !value.is_empty())?;
    let mut rendered = String::with_capacity(template.len() + 32);
    let mut cursor = 0;
    while let Some(relative_start) = template[cursor..].find("{{") {
        let start = cursor + relative_start;
        rendered.push_str(&template[cursor..start]);
        let placeholder_start = start + 2;
        let Some(relative_end) = template[placeholder_start..].find("}}") else {
            rendered.push_str(&template[start..]);
            cursor = template.len();
            break;
        };
        let end = placeholder_start + relative_end;
        let key = template[placeholder_start..end].trim();
        if let Some(value) = state.template_fields.get(key) {
            rendered.push_str(value);
        }
        cursor = end + 2;
    }
    if cursor < template.len() {
        rendered.push_str(&template[cursor..]);
    }
    let normalized = rendered
        .replace("\r\n", "\n")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn parse_robot_tokens(value: String) -> Vec<String> {
    value
        .split([',', '\n'])
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect()
}

fn normalize_canonical_value(value: String) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with('/')
    {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rustok_seo_targets::SeoTargetSlug;
    use uuid::Uuid;

    use super::*;
    use crate::dto::SeoOpenGraph;

    fn state() -> TargetState {
        let mut template_fields = BTreeMap::new();
        template_fields.insert("title".to_string(), "Demo product".to_string());
        template_fields.insert("handle".to_string(), "demo-product".to_string());
        template_fields.insert("description".to_string(), "Short description".to_string());
        TargetState {
            target_kind: SeoTargetSlug::new("product").expect("slug"),
            target_id: Uuid::nil(),
            requested_locale: Some("en".to_string()),
            effective_locale: "en".to_string(),
            title: "Fallback title".to_string(),
            description: Some("Fallback description".to_string()),
            canonical_path: "/products/demo-product".to_string(),
            alternates: Vec::new(),
            open_graph: SeoOpenGraph::default(),
            structured_data: serde_json::json!({"@type":"Product"}),
            fallback_source: "product".to_string(),
            template_fields,
        }
    }

    #[test]
    fn render_generated_record_replaces_known_placeholders() {
        let generated = render_generated_record(
            &state(),
            &SeoTemplateRuleSet {
                title: Some("{{title}} | Store".to_string()),
                meta_description: Some("Buy {{title}} today".to_string()),
                canonical_url: Some("/catalog/{{handle}}".to_string()),
                ..SeoTemplateRuleSet::default()
            },
            None,
        );

        assert_eq!(generated.title.as_deref(), Some("Demo product | Store"));
        assert_eq!(
            generated.description.as_deref(),
            Some("Buy Demo product today")
        );
        assert_eq!(
            generated.canonical_url.as_deref(),
            Some("/catalog/demo-product")
        );
    }

    #[test]
    fn render_generated_record_uses_overrides_when_present() {
        let generated = render_generated_record(
            &state(),
            &SeoTemplateRuleSet {
                title: Some("{{title}}".to_string()),
                ..SeoTemplateRuleSet::default()
            },
            Some(&SeoTemplateRuleSet {
                title: Some("Override {{title}}".to_string()),
                ..SeoTemplateRuleSet::default()
            }),
        );

        assert_eq!(generated.title.as_deref(), Some("Override Demo product"));
    }
}
