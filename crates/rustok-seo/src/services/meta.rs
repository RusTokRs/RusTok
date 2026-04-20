use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};
use uuid::Uuid;

use rustok_api::TenantContext;
use rustok_content::{normalize_locale_code, resolve_by_locale_with_fallback};
use rustok_core::normalize_locale_tag;

use crate::dto::{SeoMetaInput, SeoMetaRecord, SeoRevisionRecord, SeoTargetKind};
use crate::entities as seo_meta;
use crate::entities::{meta_translation, seo_revision};
use crate::{SeoError, SeoResult};

use super::redirects::validate_target_url;
use super::robots::first_open_graph_image_url;
use super::{trimmed_option, LoadedMeta, SeoService, TargetState};

impl SeoService {
    pub async fn seo_meta(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetKind,
        target_id: Uuid,
        locale: Option<&str>,
    ) -> SeoResult<Option<SeoMetaRecord>> {
        if !self.is_enabled(tenant.id).await? {
            return Ok(None);
        }

        let requested_locale =
            normalize_requested_meta_locale(locale, tenant.default_locale.as_str())?;
        let explicit = self
            .load_explicit_meta(tenant.id, target_kind, target_id)
            .await?;
        let state = self
            .load_target_state(
                tenant,
                target_kind,
                target_id,
                requested_locale
                    .as_deref()
                    .unwrap_or(tenant.default_locale.as_str()),
            )
            .await?;

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
            (None, Some(state)) => Ok(Some(self.meta_record_from_fallback(state))),
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
                input.target_kind,
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
        target_kind: SeoTargetKind,
        target_id: Uuid,
        note: Option<String>,
    ) -> SeoResult<SeoRevisionRecord> {
        let Some(explicit) = self
            .load_explicit_meta(tenant.id, target_kind, target_id)
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
        target_kind: SeoTargetKind,
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
        target_kind: SeoTargetKind,
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
            canonical_url: explicit.meta.canonical_url,
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
            structured_data: explicit.meta.structured_data.map(async_graphql::Json),
        }
    }

    fn meta_record_from_explicit_only(
        &self,
        default_locale: &str,
        target_kind: SeoTargetKind,
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
            canonical_url: explicit.meta.canonical_url,
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
            structured_data: explicit.meta.structured_data.map(async_graphql::Json),
        }
    }

    fn meta_record_from_fallback(&self, state: TargetState) -> SeoMetaRecord {
        SeoMetaRecord {
            target_kind: state.target_kind,
            target_id: state.target_id,
            requested_locale: state.requested_locale,
            effective_locale: state.effective_locale.clone(),
            available_locales: state
                .alternates
                .iter()
                .map(|item| item.locale.clone())
                .collect(),
            noindex: false,
            nofollow: false,
            canonical_url: None,
            translation: crate::dto::SeoMetaTranslationRecord {
                locale: state.effective_locale,
                title: Some(state.title),
                description: state.description,
                keywords: None,
                og_title: state.open_graph.title.clone(),
                og_description: state.open_graph.description.clone(),
                og_image: first_open_graph_image_url(&state.open_graph),
            },
            source: format!("{}_fallback", state.fallback_source),
            open_graph: Some(state.open_graph),
            structured_data: Some(async_graphql::Json(state.structured_data)),
        }
    }
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

fn snapshot_to_input(payload: Value, target_kind: SeoTargetKind, target_id: Uuid) -> SeoMetaInput {
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
    use crate::{SeoMetaInput, SeoMetaTranslationInput, SeoTargetKind};
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
            target_kind: SeoTargetKind::Page,
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
