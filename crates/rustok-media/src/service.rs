use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

use rustok_core::generate_id;
use rustok_storage::{StorageError, StorageService};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaStorageCleanupDecision {
    KeepRecord,
    DeleteRecord,
    RetryLater,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MediaStorageCleanupReport {
    pub inspected: u64,
    pub deleted_records: u64,
    pub kept_records: u64,
    pub retry_later: u64,
}

impl MediaStorageCleanupReport {
    pub fn is_empty(&self) -> bool {
        self.inspected == 0
    }

    pub fn changed_records(&self) -> u64 {
        self.deleted_records
    }

    pub fn completed_without_retry(&self) -> bool {
        self.retry_later == 0
    }

    pub fn should_retry(&self) -> bool {
        self.retry_later > 0
    }
}

fn classify_cleanup_probe(
    result: &std::result::Result<bytes::Bytes, StorageError>,
) -> MediaStorageCleanupDecision {
    match result {
        Ok(_) => MediaStorageCleanupDecision::KeepRecord,
        Err(StorageError::NotFound(_)) | Err(StorageError::InvalidPath(_)) => {
            MediaStorageCleanupDecision::DeleteRecord
        }
        Err(StorageError::Io(_)) | Err(StorageError::Backend(_)) => {
            MediaStorageCleanupDecision::RetryLater
        }
    }
}

fn validate_upload_policy(input: &UploadInput) -> Result<u64> {
    if !ALLOWED_MIME_PREFIXES
        .iter()
        .any(|prefix| input.content_type.starts_with(prefix))
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

    Ok(size)
}

#[cfg(test)]
mod tests {
    use std::io;

    use bytes::Bytes;
    use uuid::Uuid;

    use super::{
        classify_cleanup_probe, validate_upload_policy, MediaStorageCleanupDecision,
        MediaStorageCleanupReport,
    };
    use crate::{
        dto::{UploadInput, DEFAULT_MAX_SIZE},
        error::MediaError,
    };
    use rustok_storage::StorageError;

    fn upload_input(content_type: &str, data_len: usize) -> UploadInput {
        UploadInput {
            tenant_id: Uuid::new_v4(),
            uploaded_by: None,
            original_name: "asset.bin".to_string(),
            content_type: content_type.to_string(),
            data: Bytes::from(vec![0_u8; data_len]),
        }
    }

    #[test]
    fn cleanup_report_helpers_expose_operability_state() {
        let empty = MediaStorageCleanupReport::default();
        assert!(empty.is_empty());
        assert!(empty.completed_without_retry());
        assert!(!empty.should_retry());
        assert_eq!(empty.changed_records(), 0);

        let report = MediaStorageCleanupReport {
            inspected: 3,
            deleted_records: 1,
            kept_records: 1,
            retry_later: 1,
        };

        assert!(!report.is_empty());
        assert_eq!(report.changed_records(), 1);
        assert!(!report.completed_without_retry());
        assert!(report.should_retry());
    }

    #[test]
    fn validate_upload_policy_accepts_supported_mime_at_size_limit() {
        let input = upload_input("image/webp", DEFAULT_MAX_SIZE as usize);

        assert_eq!(
            validate_upload_policy(&input).expect("supported image should pass"),
            DEFAULT_MAX_SIZE
        );
    }

    #[test]
    fn validate_upload_policy_rejects_unsupported_mime_before_storage() {
        let input = upload_input("text/html", 16);

        let error = validate_upload_policy(&input).expect_err("html upload must be rejected");
        assert!(matches!(
            error,
            MediaError::UnsupportedMimeType(content_type) if content_type == "text/html"
        ));
    }

    #[test]
    fn validate_upload_policy_rejects_payloads_over_limit() {
        let input = upload_input("application/pdf", DEFAULT_MAX_SIZE as usize + 1);

        let error = validate_upload_policy(&input).expect_err("oversized upload must be rejected");
        assert!(matches!(
            error,
            MediaError::FileTooLarge { size, max }
                if size == DEFAULT_MAX_SIZE + 1 && max == DEFAULT_MAX_SIZE
        ));
    }

    #[test]
    fn classify_cleanup_probe_deletes_only_missing_or_invalid_paths() {
        assert_eq!(
            classify_cleanup_probe(&Err(StorageError::NotFound("missing".to_string()))),
            MediaStorageCleanupDecision::DeleteRecord
        );
        assert_eq!(
            classify_cleanup_probe(&Err(StorageError::InvalidPath("../bad".to_string()))),
            MediaStorageCleanupDecision::DeleteRecord
        );
    }

    #[test]
    fn classify_cleanup_probe_keeps_readable_objects_and_retries_transient_errors() {
        assert_eq!(
            classify_cleanup_probe(&Ok(Bytes::from_static(b"object"))),
            MediaStorageCleanupDecision::KeepRecord
        );
        assert_eq!(
            classify_cleanup_probe(&Err(StorageError::Backend("timeout".to_string()))),
            MediaStorageCleanupDecision::RetryLater
        );
        assert_eq!(
            classify_cleanup_probe(&Err(StorageError::Io(io::Error::new(
                io::ErrorKind::TimedOut,
                "timeout",
            )))),
            MediaStorageCleanupDecision::RetryLater
        );
    }
}

impl MediaService {
    pub fn new(db: DatabaseConnection, storage: StorageService) -> Self {
        Self { db, storage }
    }

    // ── Upload ────────────────────────────────────────────────────────────────

    /// Validate, store, and record a new media upload.
    pub async fn upload(&self, input: UploadInput) -> Result<MediaItem> {
        validate_upload_policy(&input)?;

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

    // ── Storage cleanup ───────────────────────────────────────────────────────

    /// Probe persisted media records and remove DB rows whose storage objects are
    /// definitively absent or invalid. Readable objects are never deleted, while
    /// transient storage failures are counted for retry instead of changing DB state.
    pub async fn cleanup_storage_orphans(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> Result<MediaStorageCleanupReport> {
        let rows = MediaEntity::find()
            .filter(MediaCol::TenantId.eq(tenant_id))
            .order_by_asc(MediaCol::CreatedAt)
            .limit(limit)
            .all(&self.db)
            .await?;

        let mut report = MediaStorageCleanupReport::default();

        for row in rows {
            report.inspected += 1;
            let probe = self.storage.read(&row.storage_path).await;
            match classify_cleanup_probe(&probe) {
                MediaStorageCleanupDecision::KeepRecord => {
                    report.kept_records += 1;
                }
                MediaStorageCleanupDecision::DeleteRecord => {
                    MediaEntity::delete_by_id(row.id).exec(&self.db).await?;
                    report.deleted_records += 1;
                }
                MediaStorageCleanupDecision::RetryLater => {
                    report.retry_later += 1;
                    if let Err(error) = probe {
                        tracing::warn!(
                            media_id = %row.id,
                            path = %row.storage_path,
                            error = %error,
                            "Storage cleanup probe failed transiently; media record kept for retry"
                        );
                    }
                }
            }
        }

        Ok(report)
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
