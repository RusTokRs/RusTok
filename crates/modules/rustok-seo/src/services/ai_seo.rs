use uuid::Uuid;

use rustok_api::TenantContext;
use rustok_seo_targets::{
    GeneratedSeoMetadata, SeoTargetLoadScope, SeoTargetSlug, validate_seo_metadata_payload,
};

use crate::{SeoError, SeoResult};

use super::{SeoService, normalize_effective_locale};

impl SeoService {
    /// Generates intelligent SEO metadata (Title, Description, Keywords, OG, Robots)
    /// for any registered target entity (Product, Category, Blog, Page, etc.).
    pub async fn generate_seo_metadata(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> SeoResult<GeneratedSeoMetadata> {
        let locale = normalize_effective_locale(
            locale.unwrap_or(tenant.default_locale.as_str()),
            tenant.default_locale.as_str(),
        )?;

        let target_state = self
            .load_target_state_with_scope(
                tenant,
                target_kind,
                target_id,
                locale.as_str(),
                SeoTargetLoadScope::Authoring,
                channel_slug,
            )
            .await?
            .ok_or(SeoError::NotFound)?;

        let name = target_state.title.trim();
        let brand = target_state
            .template_fields
            .get("brand")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let category = target_state
            .template_fields
            .get("category")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        // 1. Smart Title generation (SERP length optimized under 60 chars)
        let meta_title = match (brand, category) {
            (Some(b), Some(c)) => {
                let candidate = format!("{name} — {b} | {c}");
                if candidate.chars().count() <= 60 {
                    candidate
                } else {
                    format!("{name} — {b}")
                }
            }
            (Some(b), None) => format!("{name} — {b}"),
            (None, Some(c)) => format!("{name} | {c}"),
            (None, None) => name.to_string(),
        };

        let is_cyrillic = locale.starts_with("ru") || locale.starts_with("uk") || locale.starts_with("be");
        let meta_description = if let Some(desc) = target_state
            .description
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            let trimmed = desc.trim();
            if trimmed.chars().count() > 155 {
                let truncated: String = trimmed.chars().take(152).collect();
                format!("{truncated}...")
            } else {
                trimmed.to_string()
            }
        } else if is_cyrillic {
            match category {
                Some(c) => format!("{name} в категории {c}. Купить с гарантией и быстрой доставкой."),
                None => format!("{name}. Официальный каталог, актуальные цены и быстрая доставка."),
            }
        } else {
            match category {
                Some(c) => format!("{name} in {c}. Buy online with warranty and fast delivery."),
                None => format!("{name}. Official catalog, best prices, and fast delivery."),
            }
        };

        // 3. Smart Keywords extraction
        let mut keywords = Vec::new();
        keywords.push(name.to_lowercase());
        if let Some(b) = brand {
            keywords.push(b.to_lowercase());
        }
        if let Some(c) = category {
            keywords.push(c.to_lowercase());
        }
        let meta_keywords = keywords.join(", ");

        // 4. Open Graph & Social Cards
        let og_title = meta_title.clone();
        let og_description = meta_description.clone();

        // 5. Canonical & Robots
        let canonical_url = target_state.canonical_path;
        let robots = "index, follow".to_string();

        let generated = GeneratedSeoMetadata {
            meta_title: Some(meta_title),
            meta_description: Some(meta_description),
            meta_keywords: Some(meta_keywords),
            og_title: Some(og_title),
            og_description: Some(og_description),
            canonical_url: Some(canonical_url),
            robots: Some(robots),
        };

        validate_seo_metadata_payload(&generated)
            .map_err(|err| SeoError::validation(err))?;

        Ok(generated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_seo_targets::GeneratedSeoMetadata;

    #[test]
    fn validates_generated_seo_metadata_invariants() {
        let meta = GeneratedSeoMetadata {
            meta_title: Some("Wireless Noise-Canceling Headphones".to_string()),
            meta_description: Some("Experience crystal clear audio with advanced active noise cancellation.".to_string()),
            meta_keywords: Some("headphones, wireless, audio".to_string()),
            og_title: Some("Wireless Noise-Canceling Headphones".to_string()),
            og_description: Some("Experience crystal clear audio with advanced active noise cancellation.".to_string()),
            canonical_url: Some("/catalog/audio/headphones".to_string()),
            robots: Some("index, follow".to_string()),
        };

        assert!(validate_seo_metadata_payload(&meta).is_ok());
    }
}
