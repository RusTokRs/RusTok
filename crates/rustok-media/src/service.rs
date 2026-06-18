use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

use rustok_core::generate_id;
use rustok_storage::StorageService;

use crate::{
    dto::{
        MediaItem, MediaTranslationItem, UploadInput, UpsertTranslationInput,
        ALLOWED_MIME_PREFIXES, DEFAULT_MAX_SIZE,
    },
    entities::{
        media::{self, ActiveModel as MediaActiveModel, Column as MediaCol, Entity as MediaEntity},
        media_translation::{
            ActiveModel as TranslationActiveModel, Column as TransCol, Entity as TransEntity,
        },
    },
    error::{MediaError, Result},
};

pub struct MediaService {
    db: DatabaseConnection,
    storage: StorageService,
}

fn normalize_translation_input(input: UpsertTranslationInput) -> UpsertTranslationInput {
    UpsertTranslationInput {
        locale: normalize_locale(input.locale),
        title: normalize_optional_text(input.title),
        alt_text: normalize_optional_text(input.alt_text),
        caption: normalize_optional_text(input.caption),
    }
}

fn normalize_locale(locale: String) -> String {
    locale.trim().to_ascii_lowercase()
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{normalize_locale, normalize_optional_text, normalize_translation_input};
    use crate::dto::UpsertTranslationInput;

    #[test]
    fn normalize_translation_input_trims_locale_and_optional_text() {
        let normalized = normalize_translation_input(UpsertTranslationInput {
            locale: " RU ".to_string(),
            title: Some("  Hero  ".to_string()),
            alt_text: Some("   ".to_string()),
            caption: Some(" Caption text ".to_string()),
        });

        assert_eq!(normalized.locale, "ru");
        assert_eq!(normalized.title.as_deref(), Some("Hero"));
        assert_eq!(normalized.alt_text, None);
        assert_eq!(normalized.caption.as_deref(), Some("Caption text"));
    }

    #[test]
    fn normalize_locale_keeps_region_shape_but_canonicalizes_case() {
        assert_eq!(normalize_locale(" En-US ".to_string()), "en-us");
    }

    #[test]
    fn normalize_optional_text_converts_blank_values_to_none() {
        assert_eq!(normalize_optional_text(Some("\t\n ".to_string())), None);
        assert_eq!(normalize_optional_text(None), None);
    }
}

impl MediaService {
    pub fn new(db: DatabaseConnection, storage: StorageService) -> Self {
        Self { db, storage }
    }

    // ── Upload ────────────────────────────────────────────────────────────────

    /// Validate, store, and record a new media upload.
    pub async fn upload(&self, input: UploadInput) -> Result<MediaItem> {
        // Validation
        if !ALLOWED_MIME_PREFIXES
            .iter()
            .any(|p| input.content_type.starts_with(p))
        {
            return Err(MediaError::UnsupportedMimeType(input.content_type.clone()));
        }
        let size = input.data.len() as u64;
        if size > DEFAULT_MAX_SIZE {
            return Err(MediaError::FileTooLarge {
                size,
                max: DEFAULT_MAX_SIZE,
            });
        }

        // Generate storage path and persist to backend
        let path = StorageService::generate_path(input.tenant_id, &input.original_name);
        let uploaded = self
            .storage
            .store(&path, input.data, &input.content_type)
            .await?;

        // Sanitise filename (keep extension + uuid)
        let filename = std::path::Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path)
            .to_string();

        let id = generate_id();
        let now = Utc::now().fixed_offset();

        let active = MediaActiveModel {
            id: Set(id),
            tenant_id: Set(input.tenant_id),
            uploaded_by: Set(input.uploaded_by),
            filename: Set(filename),
            original_name: Set(input.original_name.clone()),
            mime_type: Set(input.content_type.clone()),
            size: Set(uploaded.size as i64),
            storage_path: Set(path.clone()),
            storage_driver: Set(self.storage.backend_name().to_string()),
            width: Set(None),
            height: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(now),
        };

        let model = active.insert(&self.db).await?;
        Ok(self.to_item(model))
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    pub async fn get(&self, tenant_id: Uuid, id: Uuid) -> Result<MediaItem> {
        let model = MediaEntity::find_by_id(id)
            .filter(MediaCol::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MediaError::NotFound(id))?;
        Ok(self.to_item(model))
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<MediaItem>, u64)> {
        let query = MediaEntity::find()
            .filter(MediaCol::TenantId.eq(tenant_id))
            .order_by_desc(MediaCol::CreatedAt);

        let total = query.clone().count(&self.db).await?;
        let items: Vec<crate::entities::media::Model> =
            query.limit(limit).offset(offset).all(&self.db).await?;
        Ok((items.into_iter().map(|m| self.to_item(m)).collect(), total))
    }

    // ── Delete ────────────────────────────────────────────────────────────────

    pub async fn delete(&self, tenant_id: Uuid, id: Uuid) -> Result<()> {
        let model = MediaEntity::find_by_id(id)
            .filter(MediaCol::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MediaError::NotFound(id))?;

        // Best-effort storage cleanup — log but don't fail on storage errors
        if let Err(e) = self.storage.delete(&model.storage_path).await {
            tracing::warn!(
                media_id = %id,
                path = %model.storage_path,
                error = %e,
                "Failed to delete media object from storage; DB record will still be removed"
            );
        }

        MediaEntity::delete_by_id(id).exec(&self.db).await?;
        Ok(())
    }

    // ── Translations ──────────────────────────────────────────────────────────

    pub async fn upsert_translation(
        &self,
        tenant_id: Uuid,
        media_id: Uuid,
        input: UpsertTranslationInput,
    ) -> Result<MediaTranslationItem> {
        // Ensure media belongs to tenant
        let _ = self.get(tenant_id, media_id).await?;
        let input = input.normalize().map_err(MediaError::InvalidLocale)?;

        let existing = TransEntity::find()
            .filter(TransCol::MediaId.eq(media_id))
            .filter(TransCol::Locale.eq(&input.locale))
            .one(&self.db)
            .await?;

        let model = if let Some(existing) = existing {
            let mut active: TranslationActiveModel = existing.into();
            active.title = Set(input.title);
            active.alt_text = Set(input.alt_text);
            active.caption = Set(input.caption);
            active.update(&self.db).await?
        } else {
            TranslationActiveModel {
                id: Set(generate_id()),
                media_id: Set(media_id),
                locale: Set(input.locale),
                title: Set(input.title),
                alt_text: Set(input.alt_text),
                caption: Set(input.caption),
            }
            .insert(&self.db)
            .await?
        };

        Ok(MediaTranslationItem {
            id: model.id,
            media_id: model.media_id,
            locale: model.locale,
            title: model.title,
            alt_text: model.alt_text,
            caption: model.caption,
        })
    }

    pub async fn get_translations(
        &self,
        tenant_id: Uuid,
        media_id: Uuid,
    ) -> Result<Vec<MediaTranslationItem>> {
        let _ = self.get(tenant_id, media_id).await?;
        let rows = TransEntity::find()
            .filter(TransCol::MediaId.eq(media_id))
            .order_by_asc(TransCol::Locale)
            .all(&self.db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|m| MediaTranslationItem {
                id: m.id,
                media_id: m.media_id,
                locale: m.locale,
                title: m.title,
                alt_text: m.alt_text,
                caption: m.caption,
            })
            .collect())
    }

    // ── Private ───────────────────────────────────────────────────────────────

    fn to_item(&self, m: media::Model) -> MediaItem {
        let public_url = self.storage.public_url(&m.storage_path);
        MediaItem {
            id: m.id,
            tenant_id: m.tenant_id,
            uploaded_by: m.uploaded_by,
            filename: m.filename,
            original_name: m.original_name,
            mime_type: m.mime_type,
            size: m.size,
            storage_path: m.storage_path,
            storage_driver: m.storage_driver,
            public_url,
            width: m.width,
            height: m.height,
            metadata: m.metadata,
            created_at: m.created_at.with_timezone(&Utc),
        }
    }
}
