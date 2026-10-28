use uuid::Uuid;

use rustok_api::TenantContext;
use rustok_seo_targets::{
    GeneratedSeoMetadata, SeoTargetLoadScope, SeoTargetSlug, validate_seo_metadata_payload,
};

use crate::dto::{SeoMetaInput, SeoMetaRecord, SeoMetaTranslationInput};
use crate::{SeoError, SeoResult};

use super::{LoadedMeta, SeoService, normalize_effective_locale, trimmed_option};

impl SeoService {
    /// Generates intelligent SEO metadata (Title, Description, Keywords, OG, Robots)
    /// for any registered target entity (Product, Category, Blog, Page, etc.).
    ///
    /// This is a DRAFT-only operation: it never writes. Every transport that
    /// persists generated output must go through [`SeoService::ai_smart_fill_meta`],
    /// which owns the explicit-state preservation rules in this repository.
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

        let is_cyrillic =
            locale.starts_with("ru") || locale.starts_with("uk") || locale.starts_with("be");
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
                Some(c) => {
                    format!("{name} в категории {c}. Купить с гарантией и быстрой доставкой.")
                }
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

        validate_seo_metadata_payload(&generated).map_err(SeoError::validation)?;

        Ok(generated)
    }

    /// Persists an AI smart-fill for one locale of a target.
    ///
    /// Composition lives in the generation repository so every transport
    /// (GraphQL, REST) applies the same explicit-state preservation rules.
    /// Generation refreshes textual fields only; operator-controlled state is
    /// never materialized from generation snapshots:
    ///
    /// * stored robots directives and an explicit canonical URL stay as they are;
    /// * an absent explicit canonical stays absent — the generated/frozen
    ///   canonical snapshot must not become an explicit override;
    /// * stored structured data stays as it is;
    /// * an explicit `og:image` for the exact locale is preserved, because the
    ///   generator has no OG image source.
    ///
    /// A target without explicit metadata starts from generated text with every
    /// explicit field empty, so the render pipeline keeps deriving canonical,
    /// schema, and robots from runtime state.
    pub async fn ai_smart_fill_meta(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> SeoResult<SeoMetaRecord> {
        let locale = normalize_effective_locale(
            locale.unwrap_or(tenant.default_locale.as_str()),
            tenant.default_locale.as_str(),
        )?;
        let generated = self
            .generate_seo_metadata(
                tenant,
                target_kind.clone(),
                target_id,
                Some(locale.as_str()),
                channel_slug,
            )
            .await?;
        let explicit = self
            .load_explicit_meta(tenant.id, target_kind.clone(), target_id)
            .await?;
        let input = build_smart_fill_input(
            target_kind,
            target_id,
            locale.as_str(),
            &generated,
            explicit.as_ref(),
        );
        self.upsert_meta(tenant, input).await
    }
}

/// Builds the persisted smart-fill input: generated text plus preserved
/// explicit state. Kept pure so the preservation contract is unit-testable.
fn build_smart_fill_input(
    target_kind: SeoTargetSlug,
    target_id: Uuid,
    locale: &str,
    generated: &GeneratedSeoMetadata,
    explicit: Option<&LoadedMeta>,
) -> SeoMetaInput {
    let (noindex, nofollow, canonical_url, structured_data, og_image) = match explicit {
        Some(explicit) => (
            explicit.meta.no_index,
            explicit.meta.no_follow,
            trimmed_option(explicit.meta.canonical_url.clone()),
            explicit
                .meta
                .structured_data
                .clone()
                .map(async_graphql::Json),
            explicit
                .translations
                .iter()
                .find(|translation| translation.locale == locale)
                .and_then(|translation| trimmed_option(translation.og_image.clone())),
        ),
        None => (false, false, None, None, None),
    };

    SeoMetaInput {
        target_kind,
        target_id,
        noindex,
        nofollow,
        canonical_url,
        structured_data,
        translations: vec![SeoMetaTranslationInput {
            locale: locale.to_owned(),
            title: trimmed_option(generated.meta_title.clone()),
            description: trimmed_option(generated.meta_description.clone()),
            keywords: trimmed_option(generated.meta_keywords.clone()),
            og_title: trimmed_option(generated.og_title.clone()),
            og_description: trimmed_option(generated.og_description.clone()),
            og_image,
        }],
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use rustok_seo_targets::GeneratedSeoMetadata;

    fn generated_snapshot() -> GeneratedSeoMetadata {
        GeneratedSeoMetadata {
            meta_title: Some("AI title".to_string()),
            meta_description: Some("AI description".to_string()),
            meta_keywords: Some("ai, keywords".to_string()),
            og_title: Some("AI og title".to_string()),
            og_description: Some("AI og description".to_string()),
            canonical_url: Some("/frozen/ai/canonical".to_string()),
            robots: Some("index, follow".to_string()),
        }
    }

    fn explicit_meta(og_image: Option<String>) -> LoadedMeta {
        LoadedMeta {
            meta: crate::entities::Model {
                id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                target_type: "product".to_string(),
                target_id: Uuid::new_v4(),
                no_index: true,
                no_follow: false,
                canonical_url: Some("/custom/canonical".to_string()),
                structured_data: Some(json!({"@type": "Article"})),
            },
            translations: vec![crate::entities::meta_translation::Model {
                id: Uuid::new_v4(),
                meta_id: Uuid::new_v4(),
                locale: "en".to_string(),
                title: Some("Operator title".to_string()),
                description: None,
                keywords: None,
                og_title: None,
                og_description: None,
                og_image,
            }],
        }
    }

    #[test]
    fn smart_fill_preserves_explicit_state_and_refreshes_text_only() {
        let explicit = explicit_meta(Some("/images/og.png".to_string()));
        let input = build_smart_fill_input(
            SeoTargetSlug::new("product").expect("valid target kind"),
            Uuid::new_v4(),
            "en",
            &generated_snapshot(),
            Some(&explicit),
        );

        // Operator-controlled state survives the smart fill unchanged.
        assert!(input.noindex, "robots directives must not be clobbered");
        assert!(!input.nofollow);
        assert_eq!(
            input.canonical_url.as_deref(),
            Some("/custom/canonical"),
            "explicit canonical must be preserved"
        );
        assert!(
            input.structured_data.is_some(),
            "stored structured data must be preserved"
        );

        // Textual fields are refreshed from generation; og:image stays operator-owned.
        let translation = &input.translations[0];
        assert_eq!(translation.title.as_deref(), Some("AI title"));
        assert_eq!(translation.og_image.as_deref(), Some("/images/og.png"));
    }

    #[test]
    fn smart_fill_never_materializes_generation_snapshots_without_explicit_state() {
        let input = build_smart_fill_input(
            SeoTargetSlug::new("product").expect("valid target kind"),
            Uuid::new_v4(),
            "en",
            &generated_snapshot(),
            None,
        );

        assert!(!input.noindex);
        assert!(!input.nofollow);
        assert_eq!(
            input.canonical_url, None,
            "the generated canonical snapshot must not become an explicit override"
        );
        assert_eq!(
            input.structured_data, None,
            "the generator has no structured data authority"
        );
        let translation = &input.translations[0];
        assert_eq!(translation.title.as_deref(), Some("AI title"));
        assert_eq!(translation.og_image, None);
    }

    #[test]
    fn smart_fill_skips_og_image_from_other_locale_rows() {
        let mut explicit = explicit_meta(Some("/images/og-en.png".to_string()));
        explicit.translations[0].locale = "de".to_string();

        let input = build_smart_fill_input(
            SeoTargetSlug::new("product").expect("valid target kind"),
            Uuid::new_v4(),
            "en",
            &generated_snapshot(),
            Some(&explicit),
        );

        assert_eq!(
            input.translations[0].og_image, None,
            "only the exact locale's explicit og:image may be preserved"
        );
    }

    #[test]
    fn validates_generated_seo_metadata_invariants() {
        let meta = GeneratedSeoMetadata {
            meta_title: Some("Wireless Noise-Canceling Headphones".to_string()),
            meta_description: Some(
                "Experience crystal clear audio with advanced active noise cancellation."
                    .to_string(),
            ),
            meta_keywords: Some("headphones, wireless, audio".to_string()),
            og_title: Some("Wireless Noise-Canceling Headphones".to_string()),
            og_description: Some(
                "Experience crystal clear audio with advanced active noise cancellation."
                    .to_string(),
            ),
            canonical_url: Some("/catalog/audio/headphones".to_string()),
            robots: Some("index, follow".to_string()),
        };

        assert!(validate_seo_metadata_payload(&meta).is_ok());
    }
}
