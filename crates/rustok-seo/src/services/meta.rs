use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};
use uuid::Uuid;

use rustok_api::TenantContext;
use rustok_content::{normalize_locale_code, resolve_by_locale_with_fallback};
use rustok_core::normalize_locale_tag;
use rustok_seo_targets::SeoTargetSlug;

use crate::dto::{
    SeoDocumentEffectiveState, SeoFieldSource, SeoFieldState, SeoMetaInput, SeoMetaRecord,
    SeoMetaTranslationRecord, SeoRevisionRecord,
};
use crate::entities as seo_meta;
use crate::entities::{meta_translation, seo_revision};
use crate::{SeoError, SeoResult};

use super::redirects::validate_target_url;
use super::robots::first_open_graph_image_url;
use super::templates::{generated_translation, render_generated_record, source_label};
use super::{trimmed_option, LoadedMeta, SeoService, TargetState};

impl SeoService {
    pub async fn seo_meta(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        locale: Option<&str>,
    ) -> SeoResult<Option<SeoMetaRecord>> {
        if !self.is_enabled(tenant.id).await? {
            return Ok(None);
        }

        let requested_locale =
            normalize_requested_meta_locale(locale, tenant.default_locale.as_str())?;
        let explicit = self
            .load_explicit_meta(tenant.id, target_kind.clone(), target_id)
            .await?;
        let state = self
            .load_target_state(
                tenant,
                target_kind.clone(),
                target_id,
                requested_locale
                    .as_deref()
                    .unwrap_or(tenant.default_locale.as_str()),
            )
            .await?;
        let settings = self.load_settings(tenant.id).await?;

        match (explicit, state) {
            (Some(explicit), Some(state)) => Ok(Some(self.meta_record_from_explicit(
                tenant,
                state,
                explicit,
                requested_locale,
            ))),
            (Some(explicit), None) => Ok(Some(self.meta_record_from_explicit_only(
                tenant.default_locale.as_str(),
                target_kind,
                target_id,
                explicit,
                requested_locale,
            ))),
            (None, Some(state)) => Ok(Some(self.meta_record_from_generated_or_fallback(
                tenant,
                state,
                requested_locale,
                &settings,
            ))),
            (None, None) => Ok(None),
        }
    }

    pub async fn upsert_meta(
        &self,
        tenant: &TenantContext,
        input: SeoMetaInput,
    ) -> SeoResult<SeoMetaRecord> {
        let response_locale = upsert_response_locale(&input, tenant.default_locale.as_str())?;

        if self
            .load_target_state(
                tenant,
                input.target_kind.clone(),
                input.target_id,
                tenant.default_locale.as_str(),
            )
            .await?
            .is_none()
        {
            return Err(SeoError::NotFound);
        }

        let settings = self.load_settings(tenant.id).await?;
        if let Some(canonical_url) = input.canonical_url.as_deref() {
            validate_target_url(
                canonical_url,
                settings.allowed_canonical_hosts.as_slice(),
                "canonical_url",
            )?;
        }

        let existing = seo_meta::Entity::find()
            .filter(seo_meta::Column::TenantId.eq(tenant.id))
            .filter(seo_meta::Column::TargetType.eq(input.target_kind.as_str()))
            .filter(seo_meta::Column::TargetId.eq(input.target_id))
            .one(&self.db)
            .await?;

        let meta = if let Some(existing) = existing {
            let mut active: seo_meta::ActiveModel = existing.into();
            active.no_index = Set(input.noindex);
            active.no_follow = Set(input.nofollow);
            active.canonical_url = Set(input.canonical_url.clone());
            active.structured_data = Set(input.structured_data.clone().map(|value| value.0));
            active.update(&self.db).await?
        } else {
            seo_meta::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant.id),
                target_type: Set(input.target_kind.as_str().to_string()),
                target_id: Set(input.target_id),
                no_index: Set(input.noindex),
                no_follow: Set(input.nofollow),
                canonical_url: Set(input.canonical_url.clone()),
                structured_data: Set(input.structured_data.clone().map(|value| value.0)),
            }
            .insert(&self.db)
            .await?
        };

        for translation in input.translations {
            let locale = super::normalize_effective_locale(
                translation.locale.as_str(),
                tenant.default_locale.as_str(),
            )?;
            let existing_translation = meta_translation::Entity::find()
                .filter(meta_translation::Column::MetaId.eq(meta.id))
                .filter(meta_translation::Column::Locale.eq(locale.clone()))
                .one(&self.db)
                .await?;

            if let Some(existing_translation) = existing_translation {
                let mut active: meta_translation::ActiveModel = existing_translation.into();
                active.title = Set(trimmed_option(translation.title));
                active.description = Set(trimmed_option(translation.description));
                active.keywords = Set(trimmed_option(translation.keywords));
                active.og_title = Set(trimmed_option(translation.og_title));
                active.og_description = Set(trimmed_option(translation.og_description));
                active.og_image = Set(trimmed_option(translation.og_image));
                active.update(&self.db).await?;
            } else {
                meta_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    meta_id: Set(meta.id),
                    locale: Set(locale),
                    title: Set(trimmed_option(translation.title)),
                    description: Set(trimmed_option(translation.description)),
                    keywords: Set(trimmed_option(translation.keywords)),
                    og_title: Set(trimmed_option(translation.og_title)),
                    og_description: Set(trimmed_option(translation.og_description)),
                    og_image: Set(trimmed_option(translation.og_image)),
                }
                .insert(&self.db)
                .await?;
            }
        }

        self.seo_meta(
            tenant,
            input.target_kind,
            input.target_id,
            Some(response_locale.as_str()),
        )
        .await?
        .ok_or(SeoError::NotFound)
    }

    pub async fn publish_revision(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        note: Option<String>,
    ) -> SeoResult<SeoRevisionRecord> {
        let Some(explicit) = self
            .load_explicit_meta(tenant.id, target_kind.clone(), target_id)
            .await?
        else {
            return Err(SeoError::NotFound);
        };
        let latest_revision = seo_revision::Entity::find()
            .filter(seo_revision::Column::TenantId.eq(tenant.id))
            .filter(seo_revision::Column::TargetKind.eq(target_kind.as_str()))
            .filter(seo_revision::Column::TargetId.eq(target_id))
            .order_by_desc(seo_revision::Column::Revision)
            .one(&self.db)
            .await?;
        let next_revision = latest_revision.map(|item| item.revision + 1).unwrap_or(1);
        let now = chrono::Utc::now().fixed_offset();

        let revision = seo_revision::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            target_kind: Set(target_kind.as_str().to_string()),
            target_id: Set(target_id),
            revision: Set(next_revision),
            note: Set(trimmed_option(note)),
            payload: Set(snapshot_payload(explicit)),
            created_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        Ok(SeoRevisionRecord {
            id: revision.id,
            target_kind,
            target_id,
            revision: revision.revision,
            note: revision.note,
            created_at: revision.created_at.into(),
        })
    }

    pub async fn rollback_revision(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        revision: i32,
    ) -> SeoResult<SeoMetaRecord> {
        let Some(snapshot) = seo_revision::Entity::find()
            .filter(seo_revision::Column::TenantId.eq(tenant.id))
            .filter(seo_revision::Column::TargetKind.eq(target_kind.as_str()))
            .filter(seo_revision::Column::TargetId.eq(target_id))
            .filter(seo_revision::Column::Revision.eq(revision))
            .one(&self.db)
            .await?
        else {
            return Err(SeoError::NotFound);
        };

        let input = snapshot_to_input(snapshot.payload, target_kind, target_id);
        self.upsert_meta(tenant, input).await
    }

    pub(super) async fn load_explicit_meta(
        &self,
        tenant_id: Uuid,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
    ) -> SeoResult<Option<LoadedMeta>> {
        let Some(meta) = seo_meta::Entity::find()
            .filter(seo_meta::Column::TenantId.eq(tenant_id))
            .filter(seo_meta::Column::TargetType.eq(target_kind.as_str()))
            .filter(seo_meta::Column::TargetId.eq(target_id))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let translations = meta_translation::Entity::find()
            .filter(meta_translation::Column::MetaId.eq(meta.id))
            .order_by_asc(meta_translation::Column::Locale)
            .all(&self.db)
            .await?;
        Ok(Some(LoadedMeta { meta, translations }))
    }

    fn meta_record_from_explicit(
        &self,
        tenant: &TenantContext,
        state: TargetState,
        explicit: LoadedMeta,
        requested_locale: Option<String>,
    ) -> SeoMetaRecord {
        let resolved = resolve_by_locale_with_fallback(
            explicit.translations.as_slice(),
            state.effective_locale.as_str(),
            Some(tenant.default_locale.as_str()),
            |item| item.locale.as_str(),
        );
        let translation = resolved.item.cloned();
        let title_present = translation
            .as_ref()
            .and_then(|item| trimmed_option(item.title.clone()))
            .is_some();
        let description_present = translation
            .as_ref()
            .and_then(|item| trimmed_option(item.description.clone()))
            .is_some();
        let keywords_present = translation
            .as_ref()
            .and_then(|item| trimmed_option(item.keywords.clone()))
            .is_some();
        let canonical_present = explicit
            .meta
            .canonical_url
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        let structured_data_present = explicit.meta.structured_data.is_some();
        SeoMetaRecord {
            target_kind: state.target_kind,
            target_id: state.target_id,
            requested_locale,
            effective_locale: resolved.effective_locale.clone(),
            available_locales: explicit
                .translations
                .iter()
                .map(|item| item.locale.clone())
                .collect(),
            noindex: explicit.meta.no_index,
            nofollow: explicit.meta.no_follow,
            canonical_url: explicit.meta.canonical_url.clone(),
            translation: crate::dto::SeoMetaTranslationRecord {
                locale: translation
                    .as_ref()
                    .map(|item| item.locale.clone())
                    .unwrap_or(resolved.effective_locale),
                title: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.title.clone()))
                    .or(Some(state.title)),
                description: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.description.clone()))
                    .or(state.description),
                keywords: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.keywords.clone())),
                og_title: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.og_title.clone())),
                og_description: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.og_description.clone())),
                og_image: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.og_image.clone())),
            },
            source: "explicit".to_string(),
            open_graph: Some(state.open_graph),
            structured_data: explicit
                .meta
                .structured_data
                .clone()
                .map(async_graphql::Json),
            effective_state: SeoDocumentEffectiveState {
                title: field_state(SeoFieldSource::Explicit, title_present),
                description: field_state(SeoFieldSource::Explicit, description_present),
                canonical_url: field_state(SeoFieldSource::Explicit, canonical_present),
                keywords: field_state(SeoFieldSource::Explicit, keywords_present),
                robots: field_state(SeoFieldSource::Explicit, true),
                open_graph: field_state(SeoFieldSource::Explicit, true),
                twitter: field_state(SeoFieldSource::Explicit, true),
                structured_data: field_state(SeoFieldSource::Explicit, structured_data_present),
            },
        }
    }

    fn meta_record_from_explicit_only(
        &self,
        default_locale: &str,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        explicit: LoadedMeta,
        requested_locale: Option<String>,
    ) -> SeoMetaRecord {
        let resolved = resolve_by_locale_with_fallback(
            explicit.translations.as_slice(),
            requested_locale.as_deref().unwrap_or(default_locale),
            Some(default_locale),
            |item| item.locale.as_str(),
        );
        let translation = resolved.item.cloned();
        let title_present = translation
            .as_ref()
            .and_then(|item| trimmed_option(item.title.clone()))
            .is_some();
        let description_present = translation
            .as_ref()
            .and_then(|item| trimmed_option(item.description.clone()))
            .is_some();
        let keywords_present = translation
            .as_ref()
            .and_then(|item| trimmed_option(item.keywords.clone()))
            .is_some();
        let canonical_present = explicit
            .meta
            .canonical_url
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        let structured_data_present = explicit.meta.structured_data.is_some();
        SeoMetaRecord {
            target_kind,
            target_id,
            requested_locale,
            effective_locale: resolved.effective_locale.clone(),
            available_locales: explicit
                .translations
                .iter()
                .map(|item| item.locale.clone())
                .collect(),
            noindex: explicit.meta.no_index,
            nofollow: explicit.meta.no_follow,
            canonical_url: explicit.meta.canonical_url.clone(),
            translation: crate::dto::SeoMetaTranslationRecord {
                locale: translation
                    .as_ref()
                    .map(|item| item.locale.clone())
                    .unwrap_or(resolved.effective_locale),
                title: translation.as_ref().and_then(|item| item.title.clone()),
                description: translation
                    .as_ref()
                    .and_then(|item| item.description.clone()),
                keywords: translation.as_ref().and_then(|item| item.keywords.clone()),
                og_title: translation.as_ref().and_then(|item| item.og_title.clone()),
                og_description: translation
                    .as_ref()
                    .and_then(|item| item.og_description.clone()),
                og_image: translation.as_ref().and_then(|item| item.og_image.clone()),
            },
            source: "explicit".to_string(),
            open_graph: None,
            structured_data: explicit
                .meta
                .structured_data
                .clone()
                .map(async_graphql::Json),
            effective_state: SeoDocumentEffectiveState {
                title: field_state(SeoFieldSource::Explicit, title_present),
                description: field_state(SeoFieldSource::Explicit, description_present),
                canonical_url: field_state(SeoFieldSource::Explicit, canonical_present),
                keywords: field_state(SeoFieldSource::Explicit, keywords_present),
                robots: field_state(SeoFieldSource::Explicit, true),
                open_graph: field_state(
                    SeoFieldSource::Explicit,
                    translation
                        .as_ref()
                        .and_then(|item| trimmed_option(item.og_title.clone()))
                        .is_some()
                        || translation
                            .as_ref()
                            .and_then(|item| trimmed_option(item.og_description.clone()))
                            .is_some()
                        || translation
                            .as_ref()
                            .and_then(|item| trimmed_option(item.og_image.clone()))
                            .is_some(),
                ),
                twitter: field_state(SeoFieldSource::Explicit, false),
                structured_data: field_state(SeoFieldSource::Explicit, structured_data_present),
            },
        }
    }

    fn meta_record_from_generated_or_fallback(
        &self,
        _tenant: &TenantContext,
        state: TargetState,
        requested_locale: Option<String>,
        settings: &crate::dto::SeoModuleSettings,
    ) -> SeoMetaRecord {
        let generated = render_generated_record(
            &state,
            &settings.template_defaults,
            settings.template_overrides.get(state.target_kind.as_str()),
        );
        let generated_source = generated.title.is_some()
            || generated.description.is_some()
            || generated.canonical_url.is_some()
            || generated.keywords.is_some()
            || generated.og_title.is_some()
            || generated.og_description.is_some()
            || generated.robots.is_some()
            || generated.twitter_title.is_some()
            || generated.twitter_description.is_some();
        let source = if generated_source {
            SeoFieldSource::Generated
        } else {
            SeoFieldSource::Fallback
        };
        let title = generated
            .title
            .clone()
            .unwrap_or_else(|| state.title.clone());
        let description = generated
            .description
            .clone()
            .or_else(|| state.description.clone());
        let og_title = generated
            .og_title
            .clone()
            .or_else(|| state.open_graph.title.clone());
        let og_description = generated
            .og_description
            .clone()
            .or_else(|| state.open_graph.description.clone());
        let canonical_url = generated.canonical_url.clone();
        let mut translation = if source == SeoFieldSource::Generated {
            generated_translation(&generated, state.effective_locale.clone())
        } else {
            SeoMetaTranslationRecord::default()
        };
        translation.locale = state.effective_locale.clone();
        translation.title = Some(title.clone());
        translation.description = description.clone();
        translation.og_title = og_title.clone();
        translation.og_description = og_description.clone();
        translation.og_image = first_open_graph_image_url(&state.open_graph);

        SeoMetaRecord {
            target_kind: state.target_kind,
            target_id: state.target_id,
            requested_locale: requested_locale.or(state.requested_locale),
            effective_locale: state.effective_locale.clone(),
            available_locales: state
                .alternates
                .iter()
                .map(|item| item.locale.clone())
                .collect(),
            noindex: false,
            nofollow: false,
            canonical_url: canonical_url.clone(),
            translation,
            source: source_label(source, state.fallback_source.as_str()),
            open_graph: Some(state.open_graph),
            structured_data: Some(async_graphql::Json(state.structured_data)),
            effective_state: SeoDocumentEffectiveState {
                title: field_state(source, true),
                description: field_state(source, description.is_some()),
                canonical_url: field_state(source, canonical_url.is_some()),
                keywords: field_state(source, generated.keywords.is_some()),
                robots: field_state(source, generated.robots.is_some()),
                open_graph: field_state(source, og_title.is_some() || og_description.is_some()),
                twitter: field_state(
                    source,
                    generated.twitter_title.is_some() || generated.twitter_description.is_some(),
                ),
                structured_data: field_state(SeoFieldSource::Fallback, true),
            },
        }
    }
}

fn field_state(source: SeoFieldSource, present: bool) -> SeoFieldState {
    SeoFieldState { source, present }
}

fn normalize_requested_meta_locale(
    locale: Option<&str>,
    fallback_locale: &str,
) -> SeoResult<Option<String>> {
    match locale.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => normalize_locale_tag(value)
            .or_else(|| normalize_locale_code(value))
            .map(Some)
            .ok_or_else(|| SeoError::validation("invalid locale")),
        None => Ok(Some(super::normalize_effective_locale(
            fallback_locale,
            fallback_locale,
        )?)),
    }
}

fn upsert_response_locale(input: &SeoMetaInput, fallback_locale: &str) -> SeoResult<String> {
    input
        .translations
        .first()
        .map(|translation| {
            super::normalize_effective_locale(translation.locale.as_str(), fallback_locale)
        })
        .transpose()?
        .or_else(|| Some(fallback_locale.to_string()))
        .ok_or_else(|| SeoError::validation("invalid locale"))
}

fn snapshot_payload(explicit: LoadedMeta) -> Value {
    json!({
        "noindex": explicit.meta.no_index,
        "nofollow": explicit.meta.no_follow,
        "canonical_url": explicit.meta.canonical_url,
        "structured_data": explicit.meta.structured_data,
        "translations": explicit.translations.iter().map(|translation| {
            json!({
                "locale": translation.locale,
                "title": translation.title,
                "description": translation.description,
                "keywords": translation.keywords,
                "og_title": translation.og_title,
                "og_description": translation.og_description,
                "og_image": translation.og_image,
            })
        }).collect::<Vec<_>>(),
    })
}

fn snapshot_to_input(payload: Value, target_kind: SeoTargetSlug, target_id: Uuid) -> SeoMetaInput {
    SeoMetaInput {
        target_kind,
        target_id,
        noindex: payload
            .get("noindex")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        nofollow: payload
            .get("nofollow")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        canonical_url: payload
            .get("canonical_url")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        structured_data: payload
            .get("structured_data")
            .cloned()
            .filter(|value| !value.is_null())
            .map(async_graphql::Json),
        translations: payload
            .get("translations")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| serde_json::from_value(value).ok())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_requested_meta_locale, upsert_response_locale};
    use crate::{seo_builtin_slug, SeoMetaInput, SeoMetaTranslationInput, SeoTargetSlug};
    use uuid::Uuid;

    #[test]
    fn normalize_requested_meta_locale_canonicalizes_equivalent_tags() {
        let locale = normalize_requested_meta_locale(Some(" pt_br "), "en")
            .expect("locale normalization should succeed");

        assert_eq!(locale.as_deref(), Some("pt-BR"));
    }

    #[test]
    fn normalize_requested_meta_locale_rejects_invalid_values() {
        let error = normalize_requested_meta_locale(Some("**"), "en")
            .expect_err("invalid locale should fail");

        assert!(error.to_string().contains("invalid locale"));
    }

    #[test]
    fn upsert_response_locale_prefers_canonical_translation_locale() {
        let input = SeoMetaInput {
            target_kind: SeoTargetSlug::new(seo_builtin_slug::PAGE)
                .expect("builtin SEO target slug must stay valid"),
            target_id: Uuid::new_v4(),
            noindex: false,
            nofollow: false,
            canonical_url: None,
            structured_data: None,
            translations: vec![SeoMetaTranslationInput {
                locale: "pt_br".to_string(),
                title: None,
                description: None,
                keywords: None,
                og_title: None,
                og_description: None,
                og_image: None,
            }],
        };

        let locale = upsert_response_locale(&input, "en").expect("response locale should resolve");

        assert_eq!(locale, "pt-BR");
    }
}
