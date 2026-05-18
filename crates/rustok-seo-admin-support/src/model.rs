use rustok_seo_targets::SeoTargetSlug;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::locale::normalize_locale_tag;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SeoMetaTranslationView {
    pub locale: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    #[serde(rename = "ogTitle")]
    pub og_title: Option<String>,
    #[serde(rename = "ogDescription")]
    pub og_description: Option<String>,
    #[serde(rename = "ogImage")]
    pub og_image: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SeoMetaView {
    #[serde(rename = "targetKind")]
    pub target_kind: Option<SeoTargetSlug>,
    #[serde(rename = "targetId")]
    pub target_id: Option<String>,
    #[serde(rename = "requestedLocale")]
    pub requested_locale: Option<String>,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    #[serde(rename = "availableLocales")]
    pub available_locales: Vec<String>,
    pub noindex: bool,
    pub nofollow: bool,
    #[serde(rename = "canonicalUrl")]
    pub canonical_url: Option<String>,
    pub translation: SeoMetaTranslationView,
    pub source: String,
    #[serde(rename = "structuredData")]
    pub structured_data: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SeoRevisionView {
    pub revision: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeoMetaMutationInput {
    #[serde(rename = "targetKind")]
    pub target_kind: SeoTargetSlug,
    #[serde(rename = "targetId")]
    pub target_id: String,
    pub noindex: bool,
    pub nofollow: bool,
    #[serde(rename = "canonicalUrl", skip_serializing_if = "Option::is_none")]
    pub canonical_url: Option<String>,
    #[serde(rename = "structuredData", skip_serializing_if = "Option::is_none")]
    pub structured_data: Option<Value>,
    pub translations: Vec<SeoMetaTranslationMutationInput>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeoMetaTranslationMutationInput {
    pub locale: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<String>,
    #[serde(rename = "ogTitle", skip_serializing_if = "Option::is_none")]
    pub og_title: Option<String>,
    #[serde(rename = "ogDescription", skip_serializing_if = "Option::is_none")]
    pub og_description: Option<String>,
    #[serde(rename = "ogImage", skip_serializing_if = "Option::is_none")]
    pub og_image: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SeoEntityForm {
    pub locale: String,
    pub title: String,
    pub description: String,
    pub keywords: String,
    pub canonical_url: String,
    pub og_title: String,
    pub og_description: String,
    pub og_image: String,
    pub structured_data_type: String,
    pub structured_data_payload: String,
    pub noindex: bool,
    pub nofollow: bool,
}

impl SeoEntityForm {
    pub fn new(default_locale: String) -> Self {
        Self {
            locale: default_locale,
            title: String::new(),
            description: String::new(),
            keywords: String::new(),
            canonical_url: String::new(),
            og_title: String::new(),
            og_description: String::new(),
            og_image: String::new(),
            structured_data_type: String::new(),
            structured_data_payload: String::new(),
            noindex: false,
            nofollow: false,
        }
    }

    pub fn apply_locale(&mut self, locale: String) {
        self.locale = normalize_locale_tag(locale.as_str()).unwrap_or_default();
    }

    pub fn apply_record(&mut self, meta: &SeoMetaView) {
        self.locale = meta.translation.locale.clone();
        self.title = meta.translation.title.clone().unwrap_or_default();
        self.description = meta.translation.description.clone().unwrap_or_default();
        self.keywords = meta.translation.keywords.clone().unwrap_or_default();
        self.canonical_url = meta.canonical_url.clone().unwrap_or_default();
        self.og_title = meta.translation.og_title.clone().unwrap_or_default();
        self.og_description = meta.translation.og_description.clone().unwrap_or_default();
        self.og_image = meta.translation.og_image.clone().unwrap_or_default();
        self.structured_data_type.clear();
        self.structured_data_payload.clear();
        if let Some(structured_data) = meta.structured_data.as_ref() {
            match structured_data {
                Value::Object(object) => {
                    if let Some(schema_type) = extract_primary_type_value(object.get("@type")) {
                        self.structured_data_type =
                            normalize_schema_type_input(schema_type.as_str()).unwrap_or(schema_type);
                    }
                    let mut payload = object.clone();
                    payload.remove("@type");
                    if !payload.is_empty() {
                        self.structured_data_payload = serde_json::to_string_pretty(&Value::Object(payload))
                            .unwrap_or_else(|_| "{}".to_string());
                    }
                }
                other => {
                    self.structured_data_payload = serde_json::to_string_pretty(other)
                        .unwrap_or_else(|_| "{}".to_string());
                }
            }
        }
        self.noindex = meta.noindex;
        self.nofollow = meta.nofollow;
    }

    pub fn clear_content(&mut self) {
        self.title.clear();
        self.description.clear();
        self.keywords.clear();
        self.canonical_url.clear();
        self.og_title.clear();
        self.og_description.clear();
        self.og_image.clear();
        self.structured_data_type.clear();
        self.structured_data_payload.clear();
        self.noindex = false;
        self.nofollow = false;
    }

    pub fn build_input(
        &self,
        target_kind: SeoTargetSlug,
        target_id: &str,
    ) -> Result<SeoMetaMutationInput, String> {
        let target_id = validate_target_id(target_id)?.to_string();
        let locale = if self.locale.trim().is_empty() {
            return Err("Host locale is required.".to_string());
        } else {
            normalize_locale_tag(self.locale.as_str())
                .ok_or_else(|| "Invalid host locale.".to_string())?
        };

        Ok(SeoMetaMutationInput {
            target_kind,
            target_id,
            noindex: self.noindex,
            nofollow: self.nofollow,
            canonical_url: non_empty_option(&self.canonical_url),
            structured_data: self.parse_structured_data()?,
            translations: vec![SeoMetaTranslationMutationInput {
                locale,
                title: non_empty_option(&self.title),
                description: non_empty_option(&self.description),
                keywords: non_empty_option(&self.keywords),
                og_title: non_empty_option(&self.og_title),
                og_description: non_empty_option(&self.og_description),
                og_image: non_empty_option(&self.og_image),
            }],
        })
    }

    pub fn completeness_report(&self) -> SeoCompletenessReport {
        let mut score = 0_u8;
        let mut recommendations = Vec::new();

        let title_len = self.title.trim().chars().count();
        if (10..=60).contains(&title_len) {
            score += 25;
        } else if title_len > 0 {
            score += 15;
            recommendations.push(SeoRecommendation::AdjustTitleLength);
        } else {
            recommendations.push(SeoRecommendation::AddSeoTitle);
        }

        let description_len = self.description.trim().chars().count();
        if (50..=160).contains(&description_len) {
            score += 25;
        } else if description_len > 0 {
            score += 15;
            recommendations.push(SeoRecommendation::AdjustDescriptionLength);
        } else {
            recommendations.push(SeoRecommendation::AddMetaDescription);
        }

        if !self.canonical_url.trim().is_empty() {
            score += 15;
        } else {
            recommendations.push(SeoRecommendation::SetCanonicalUrl);
        }

        if !self.og_title.trim().is_empty() {
            score += 10;
        } else {
            recommendations.push(SeoRecommendation::AddOpenGraphTitle);
        }

        if !self.og_description.trim().is_empty() {
            score += 10;
        } else {
            recommendations.push(SeoRecommendation::AddOpenGraphDescription);
        }

        if !self.og_image.trim().is_empty() {
            score += 10;
        } else {
            recommendations.push(SeoRecommendation::AddOpenGraphImage);
        }

        if !self.structured_data_type.trim().is_empty() || !self.structured_data_payload.trim().is_empty() {
            score += 5;
        }

        SeoCompletenessReport {
            score,
            recommendations,
        }
    }

    fn parse_structured_data(&self) -> Result<Option<Value>, String> {
        let normalized_schema_type = normalize_schema_type_input(self.structured_data_type.as_str());
        let schema_type = normalized_schema_type.as_deref().unwrap_or("");
        let payload = self.structured_data_payload.trim();
        if schema_type.is_empty() && payload.is_empty() {
            return Ok(None);
        }

        let payload_value = if payload.is_empty() {
            None
        } else {
            Some(
                serde_json::from_str::<Value>(payload)
                    .map_err(|err| format!("Invalid structured data payload JSON: {err}"))?,
            )
        };

        if schema_type.is_empty() {
            if let Some(value) = payload_value.as_ref() {
                if !has_non_empty_json_ld_type(value) {
                    return Err(
                        "Structured data payload must contain at least one non-empty @type when schema type is empty."
                            .to_string(),
                    );
                }
            }
            return Ok(payload_value);
        }

        let mut object = match payload_value {
            Some(Value::Object(mut object)) => {
                if let Some(existing_type) =
                    extract_primary_type_value(object.get("@type")).as_deref()
                {
                    let normalized_existing_type =
                        normalize_schema_type_input(existing_type).unwrap_or_else(|| existing_type.to_string());
                    if normalized_existing_type != schema_type {
                        return Err(
                            "Structured data payload @type must match schema type field."
                                .to_string(),
                        );
                    }
                }
                object.remove("@type");
                object
            }
            Some(_) => {
                return Err(
                    "Structured data payload must be a JSON object when schema type is set."
                        .to_string(),
                )
            }
            None => Map::new(),
        };
        object.insert("@type".to_string(), Value::String(schema_type.to_string()));
        Ok(Some(Value::Object(object)))
    }
}

fn has_non_empty_json_ld_type(value: &Value) -> bool {
    match value {
        Value::Object(object) => {
            let has_direct_type = object
                .get("@type")
                .map(has_non_empty_type_value)
                .unwrap_or(false);
            if has_direct_type {
                return true;
            }

            object
                .get("@graph")
                .map(has_non_empty_json_ld_type)
                .unwrap_or(false)
        }
        Value::Array(values) => values.iter().any(has_non_empty_json_ld_type),
        _ => false,
    }
}

fn has_non_empty_type_value(value: &Value) -> bool {
    match value {
        Value::String(raw) => !raw.trim().is_empty(),
        Value::Array(values) => values
            .iter()
            .filter_map(Value::as_str)
            .any(|raw| !raw.trim().is_empty()),
        _ => false,
    }
}

fn extract_primary_type_value(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(raw)) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Some(Value::Array(values)) => values.iter().find_map(|value| match value {
            Value::String(raw) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            _ => None,
        }),
        _ => None,
    }
}

fn normalize_schema_type_input(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_leading_at = trimmed.trim_start_matches('@').trim();
    if without_leading_at.is_empty() {
        None
    } else {
        Some(without_leading_at.to_string())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeoCompletenessReport {
    pub score: u8,
    pub recommendations: Vec<SeoRecommendation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeoRecommendation {
    AdjustTitleLength,
    AddSeoTitle,
    AdjustDescriptionLength,
    AddMetaDescription,
    SetCanonicalUrl,
    AddOpenGraphTitle,
    AddOpenGraphDescription,
    AddOpenGraphImage,
}

pub fn validate_target_id(value: &str) -> Result<Uuid, String> {
    if value.trim().is_empty() {
        return Err("Target id is required.".to_string());
    }

    Uuid::parse_str(value.trim()).map_err(|_| "Invalid target id.".to_string())
}

fn non_empty_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{SeoEntityForm, SeoMetaTranslationView, SeoMetaView, SeoRecommendation};
    use serde_json::json;
    use rustok_seo_targets::{builtin_slug as seo_builtin_slug, SeoTargetSlug};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn build_input_canonicalizes_locale_before_write() {
        let mut form = SeoEntityForm::new("pt_br".to_string());
        form.title = "Titulo".to_string();

        let input = form
            .build_input(
                SeoTargetSlug::new(seo_builtin_slug::PAGE).expect("builtin SEO target slug"),
                Uuid::new_v4().to_string().as_str(),
            )
            .expect("input should build");

        assert_eq!(input.translations[0].locale, "pt-BR");
    }

    #[test]
    fn build_input_rejects_missing_host_locale() {
        let form = SeoEntityForm::new(String::new());
        let error = form
            .build_input(
                SeoTargetSlug::new(seo_builtin_slug::PAGE).expect("builtin SEO target slug"),
                Uuid::new_v4().to_string().as_str(),
            )
            .expect_err("missing locale should fail");

        assert_eq!(error, "Host locale is required.");
    }

    #[test]
    fn completeness_report_uses_typed_recommendations() {
        let form = SeoEntityForm::new("ru".to_string());
        let report = form.completeness_report();

        assert!(report
            .recommendations
            .contains(&SeoRecommendation::AddSeoTitle));
        assert!(report
            .recommendations
            .contains(&SeoRecommendation::AddMetaDescription));
    }

    #[test]
    fn build_input_serializes_typed_structured_data_fields() {
        let mut form = SeoEntityForm::new("en-US".to_string());
        form.structured_data_type = "Product".to_string();
        form.structured_data_payload = r#"{"name":"Demo"}"#.to_string();

        let input = form
            .build_input(
                SeoTargetSlug::new(seo_builtin_slug::PRODUCT).expect("builtin SEO target slug"),
                Uuid::new_v4().to_string().as_str(),
            )
            .expect("input should build");

        assert_eq!(input.structured_data, Some(json!({"@type":"Product","name":"Demo"})));
    }

    #[test]
    fn build_input_rejects_non_object_payload_when_schema_type_present() {
        let mut form = SeoEntityForm::new("en-US".to_string());
        form.structured_data_type = "Product".to_string();
        form.structured_data_payload = r#"["not-object"]"#.to_string();

        let error = form
            .build_input(
                SeoTargetSlug::new(seo_builtin_slug::PRODUCT).expect("builtin SEO target slug"),
                Uuid::new_v4().to_string().as_str(),
            )
            .expect_err("non-object payload should fail");

        assert_eq!(
            error,
            "Structured data payload must be a JSON object when schema type is set."
        );
    }

    #[test]
    fn build_input_rejects_payload_without_type_when_schema_type_empty() {
        let mut form = SeoEntityForm::new("en-US".to_string());
        form.structured_data_payload = r#"{"name":"No type"}"#.to_string();

        let error = form
            .build_input(
                SeoTargetSlug::new(seo_builtin_slug::PRODUCT).expect("builtin SEO target slug"),
                Uuid::new_v4().to_string().as_str(),
            )
            .expect_err("payload without @type should fail");

        assert_eq!(
            error,
            "Structured data payload must contain at least one non-empty @type when schema type is empty."
        );
    }

    #[test]
    fn build_input_rejects_payload_type_mismatch() {
        let mut form = SeoEntityForm::new("en-US".to_string());
        form.structured_data_type = "Product".to_string();
        form.structured_data_payload = r#"{"@type":"Article","name":"Demo"}"#.to_string();

        let error = form
            .build_input(
                SeoTargetSlug::new(seo_builtin_slug::PRODUCT).expect("builtin SEO target slug"),
                Uuid::new_v4().to_string().as_str(),
            )
            .expect_err("mismatched @type should fail");

        assert_eq!(
            error,
            "Structured data payload @type must match schema type field."
        );
    }

    #[test]
    fn apply_record_extracts_type_from_type_array() {
        let mut form = SeoEntityForm::new("en-US".to_string());
        let record = SeoMetaView {
            target_kind: None,
            target_id: None,
            requested_locale: None,
            effective_locale: "en-US".to_string(),
            available_locales: vec!["en-US".to_string()],
            noindex: false,
            nofollow: false,
            canonical_url: None,
            translation: SeoMetaTranslationView {
                locale: "en-US".to_string(),
                ..SeoMetaTranslationView::default()
            },
            source: "explicit".to_string(),
            structured_data: Some(json!({"@type":["Article","Thing"],"name":"Demo"})),
        };

        form.apply_record(&record);

        assert_eq!(form.structured_data_type, "Article");
    }

    #[test]
    fn apply_record_extracts_first_non_empty_type_from_type_array() {
        let mut form = SeoEntityForm::new("en-US".to_string());
        let record = SeoMetaView {
            target_kind: None,
            target_id: None,
            requested_locale: None,
            effective_locale: "en-US".to_string(),
            available_locales: vec!["en-US".to_string()],
            noindex: false,
            nofollow: false,
            canonical_url: None,
            translation: SeoMetaTranslationView {
                locale: "en-US".to_string(),
                ..SeoMetaTranslationView::default()
            },
            source: "explicit".to_string(),
            structured_data: Some(json!({"@type":["", " ", "FAQPage"],"name":"Demo"})),
        };

        form.apply_record(&record);

        assert_eq!(form.structured_data_type, "FAQPage");
    }

    #[test]
    fn build_input_accepts_graph_payload_with_nested_typed_nodes() {
        let mut form = SeoEntityForm::new("en-US".to_string());
        form.structured_data_payload =
            r#"{"@graph":[{"@type":"BreadcrumbList","itemListElement":[]}]}"#.to_string();

        let input = form
            .build_input(
                SeoTargetSlug::new(seo_builtin_slug::PAGE).expect("builtin SEO target slug"),
                Uuid::new_v4().to_string().as_str(),
            )
            .expect("graph payload with nested type should be valid");

        assert_eq!(
            input.structured_data,
            Some(json!({"@graph":[{"@type":"BreadcrumbList","itemListElement":[]}]}))
        );
    }

    #[test]
    fn build_input_normalizes_schema_type_without_leading_at() {
        let mut form = SeoEntityForm::new("en-US".to_string());
        form.structured_data_type = "  @Product  ".to_string();
        form.structured_data_payload = r#"{"name":"Demo"}"#.to_string();

        let input = form
            .build_input(
                SeoTargetSlug::new(seo_builtin_slug::PRODUCT).expect("builtin SEO target slug"),
                Uuid::new_v4().to_string().as_str(),
            )
            .expect("input should build");

        assert_eq!(
            input.structured_data,
            Some(json!({"@type":"Product","name":"Demo"}))
        );
    }

    #[test]
    fn build_input_accepts_payload_type_with_leading_at_when_matching() {
        let mut form = SeoEntityForm::new("en-US".to_string());
        form.structured_data_type = "Product".to_string();
        form.structured_data_payload = r#"{"@type":"@Product","name":"Demo"}"#.to_string();

        let input = form
            .build_input(
                SeoTargetSlug::new(seo_builtin_slug::PRODUCT).expect("builtin SEO target slug"),
                Uuid::new_v4().to_string().as_str(),
            )
            .expect("input should build");

        assert_eq!(
            input.structured_data,
            Some(json!({"@type":"Product","name":"Demo"}))
        );
    }

    #[test]
    fn apply_record_normalizes_schema_type_with_leading_at() {
        let mut form = SeoEntityForm::new("en-US".to_string());
        let record = SeoMetaView {
            target_kind: None,
            target_id: None,
            requested_locale: None,
            effective_locale: "en-US".to_string(),
            available_locales: vec!["en-US".to_string()],
            noindex: false,
            nofollow: false,
            canonical_url: None,
            translation: SeoMetaTranslationView {
                locale: "en-US".to_string(),
                ..SeoMetaTranslationView::default()
            },
            source: "explicit".to_string(),
            structured_data: Some(json!({"@type":"@Product","name":"Demo"})),
        };

        form.apply_record(&record);

        assert_eq!(form.structured_data_type, "Product");
    }
}
