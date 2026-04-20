use rustok_core::normalize_locale_tag;
use rustok_seo::SeoTargetKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

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
    pub target_kind: Option<SeoTargetKind>,
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
    pub target_kind: SeoTargetKind,
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
    pub structured_data: String,
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
            structured_data: String::new(),
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
        self.structured_data = meta
            .structured_data
            .as_ref()
            .map(|value| serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string()))
            .unwrap_or_default();
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
        self.structured_data.clear();
        self.noindex = false;
        self.nofollow = false;
    }

    pub fn build_input(
        &self,
        target_kind: SeoTargetKind,
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

        if !self.structured_data.trim().is_empty() {
            score += 5;
        }

        SeoCompletenessReport {
            score,
            recommendations,
        }
    }

    fn parse_structured_data(&self) -> Result<Option<Value>, String> {
        if self.structured_data.trim().is_empty() {
            return Ok(None);
        }

        serde_json::from_str::<Value>(self.structured_data.as_str())
            .map(Some)
            .map_err(|err| format!("Invalid structured data JSON: {err}"))
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
    use super::{SeoEntityForm, SeoRecommendation};
    use rustok_seo::SeoTargetKind;
    use uuid::Uuid;

    #[test]
    fn build_input_canonicalizes_locale_before_write() {
        let mut form = SeoEntityForm::new("pt_br".to_string());
        form.title = "Titulo".to_string();

        let input = form
            .build_input(SeoTargetKind::Page, Uuid::new_v4().to_string().as_str())
            .expect("input should build");

        assert_eq!(input.translations[0].locale, "pt-BR");
    }

    #[test]
    fn build_input_rejects_missing_host_locale() {
        let form = SeoEntityForm::new(String::new());
        let error = form
            .build_input(SeoTargetKind::Page, Uuid::new_v4().to_string().as_str())
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
}
