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
        MediaAssetSummary, MediaItem, MediaTranslationItem, UploadInput, UpsertTranslationInput,
        DEFAULT_MAX_SIZE,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct MediaStorageCleanupReport {
    pub inspected: u64,
    pub deleted_records: u64,
    pub kept_records: u64,
    pub retry_later: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaUsageSnapshot {
    pub tenant_id: Uuid,
    pub file_count: i64,
    pub total_bytes: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VerifiedMediaType {
    mime_type: &'static str,
    extension: &'static str,
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

pub async fn load_media_usage_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<MediaUsageSnapshot> {
    let file_count = MediaEntity::find()
        .filter(MediaCol::TenantId.eq(tenant_id))
        .count(db)
        .await? as i64;

    let total_bytes = MediaEntity::find()
        .filter(MediaCol::TenantId.eq(tenant_id))
        .select_only()
        .column_as(sea_orm::sea_query::Expr::col(MediaCol::Size).sum(), "total")
        .into_tuple::<Option<i64>>()
        .one(db)
        .await?
        .flatten()
        .unwrap_or(0);

    Ok(MediaUsageSnapshot {
        tenant_id,
        file_count,
        total_bytes,
    })
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

fn validate_upload_policy(input: &UploadInput) -> Result<(u64, VerifiedMediaType)> {
    let size = input.data.len() as u64;
    if size > DEFAULT_MAX_SIZE {
        return Err(MediaError::FileTooLarge {
            size,
            max: DEFAULT_MAX_SIZE,
        });
    }
    if input.data.is_empty() {
        return Err(MediaError::InvalidMediaContent {
            declared: input.content_type.clone(),
            reason: "empty files are not allowed".to_string(),
        });
    }

    let media_type = verify_media_type(&input.content_type, input.data.as_ref())?;
    Ok((size, media_type))
}

fn verify_media_type(declared: &str, data: &[u8]) -> Result<VerifiedMediaType> {
    let normalized = declared
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    let candidate = match normalized.as_str() {
        "image/jpeg" if starts_with(data, &[0xff, 0xd8, 0xff]) => Some(VerifiedMediaType {
            mime_type: "image/jpeg",
            extension: "jpg",
        }),
        "image/png" if starts_with(data, b"\x89PNG\r\n\x1a\n") => Some(VerifiedMediaType {
            mime_type: "image/png",
            extension: "png",
        }),
        "image/gif" if starts_with(data, b"GIF87a") || starts_with(data, b"GIF89a") => {
            Some(VerifiedMediaType {
                mime_type: "image/gif",
                extension: "gif",
            })
        }
        "image/webp" if is_riff_type(data, b"WEBP") => Some(VerifiedMediaType {
            mime_type: "image/webp",
            extension: "webp",
        }),
        "image/bmp" if starts_with(data, b"BM") => Some(VerifiedMediaType {
            mime_type: "image/bmp",
            extension: "bmp",
        }),
        "image/tiff" if starts_with(data, b"II*\0") || starts_with(data, b"MM\0*") => {
            Some(VerifiedMediaType {
                mime_type: "image/tiff",
                extension: "tiff",
            })
        }
        "image/avif" if is_iso_bmff_brand(data, &[b"avif", b"avis"]) => Some(VerifiedMediaType {
            mime_type: "image/avif",
            extension: "avif",
        }),
        "video/mp4" if is_iso_bmff(data) => Some(VerifiedMediaType {
            mime_type: "video/mp4",
            extension: "mp4",
        }),
        "video/quicktime" if is_iso_bmff_brand(data, &[b"qt  "]) => Some(VerifiedMediaType {
            mime_type: "video/quicktime",
            extension: "mov",
        }),
        "video/webm" if starts_with(data, &[0x1a, 0x45, 0xdf, 0xa3]) => Some(VerifiedMediaType {
            mime_type: "video/webm",
            extension: "webm",
        }),
        "video/x-msvideo" if is_riff_type(data, b"AVI ") => Some(VerifiedMediaType {
            mime_type: "video/x-msvideo",
            extension: "avi",
        }),
        "audio/mpeg" if is_mpeg_audio(data) => Some(VerifiedMediaType {
            mime_type: "audio/mpeg",
            extension: "mp3",
        }),
        "audio/wav" | "audio/x-wav" if is_riff_type(data, b"WAVE") => Some(VerifiedMediaType {
            mime_type: "audio/wav",
            extension: "wav",
        }),
        "audio/ogg" if starts_with(data, b"OggS") => Some(VerifiedMediaType {
            mime_type: "audio/ogg",
            extension: "ogg",
        }),
        "audio/flac" if starts_with(data, b"fLaC") => Some(VerifiedMediaType {
            mime_type: "audio/flac",
            extension: "flac",
        }),
        "audio/aac" if is_aac_adts(data) => Some(VerifiedMediaType {
            mime_type: "audio/aac",
            extension: "aac",
        }),
        "audio/mp4" if is_iso_bmff(data) => Some(VerifiedMediaType {
            mime_type: "audio/mp4",
            extension: "m4a",
        }),
        "application/pdf" if starts_with(data, b"%PDF-") => Some(VerifiedMediaType {
            mime_type: "application/pdf",
            extension: "pdf",
        }),
        _ => None,
    };

    candidate.ok_or_else(|| {
        if normalized == "image/svg+xml" || normalized.ends_with("+xml") {
            MediaError::UnsupportedMimeType(normalized)
        } else if is_supported_declared_type(&normalized) {
            MediaError::InvalidMediaContent {
                declared: normalized,
                reason: "file signature does not match the declared media type".to_string(),
            }
        } else {
            MediaError::UnsupportedMimeType(normalized)
        }
    })
}

fn is_supported_declared_type(value: &str) -> bool {
    matches!(
        value,
        "image/jpeg"
            | "image/png"
            | "image/gif"
            | "image/webp"
            | "image/bmp"
            | "image/tiff"
            | "image/avif"
            | "video/mp4"
            | "video/quicktime"
            | "video/webm"
            | "video/x-msvideo"
            | "audio/mpeg"
            | "audio/wav"
            | "audio/x-wav"
            | "audio/ogg"
            | "audio/flac"
            | "audio/aac"
            | "audio/mp4"
            | "application/pdf"
    )
}

fn starts_with(data: &[u8], signature: &[u8]) -> bool {
    data.get(..signature.len()) == Some(signature)
}

fn is_riff_type(data: &[u8], kind: &[u8; 4]) -> bool {
    starts_with(data, b"RIFF") && data.get(8..12) == Some(kind.as_slice())
}

fn is_iso_bmff(data: &[u8]) -> bool {
    data.get(4..8) == Some(b"ftyp".as_slice())
}

fn is_iso_bmff_brand(data: &[u8], brands: &[&[u8; 4]]) -> bool {
    if !is_iso_bmff(data) {
        return false;
    }
    data.get(8..12)
        .is_some_and(|brand| brands.iter().any(|candidate| brand == candidate.as_slice()))
        || data.get(16..).is_some_and(|rest| {
            rest.chunks_exact(4)
                .any(|brand| brands.iter().any(|candidate| brand == candidate.as_slice()))
        })
}

fn is_mpeg_audio(data: &[u8]) -> bool {
    starts_with(data, b"ID3")
        || data
            .get(..2)
            .is_some_and(|prefix| prefix[0] == 0xff && prefix[1] & 0xe0 == 0xe0)
}

fn is_aac_adts(data: &[u8]) -> bool {
    data.get(..2)
        .is_some_and(|prefix| prefix[0] == 0xff && prefix[1] & 0xf6 == 0xf0)
}

fn normalize_original_name(value: &str, extension: &str) -> String {
    let normalized_path = value.replace('\\', "/");
    let basename = normalized_path.rsplit('/').next().unwrap_or_default();
    let cleaned = basename
        .chars()
        .filter(|character| !character.is_control() && *character != '\0')
        .take(255)
        .collect::<String>();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        format!("upload.{extension}")
    } else {
        cleaned
    }
}

impl MediaService {
    pub fn new(db: DatabaseConnection, storage: StorageService) -> Self {
        Self { db, storage }
    }

    /// Validate, store, and record a new media upload.
    pub async fn upload(&self, input: UploadInput) -> Result<MediaItem> {
        let (_, verified) = validate_upload_policy(&input)?;
        let original_name = normalize_original_name(&input.original_name, verified.extension);
        let canonical_name = format!("upload.{}", verified.extension);
        let path = StorageService::generate_path(input.tenant_id, &canonical_name);
        let uploaded = self
            .storage
            .store(&path, input.data, verified.mime_type)
            .await?;

        let filename = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&path)
            .to_string();
        let id = generate_id();
        let now = Utc::now().fixed_offset();
        let active = MediaActiveModel {
            id: Set(id),
            tenant_id: Set(input.tenant_id),
            uploaded_by: Set(input.uploaded_by),
            filename: Set(filename),
            original_name: Set(original_name),
            mime_type: Set(verified.mime_type.to_string()),
            size: Set(uploaded.size as i64),
            storage_path: Set(path.clone()),
            storage_driver: Set(self.storage.backend_name().to_string()),
            width: Set(None),
            height: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(now),
        };

        let model = match active.insert(&self.db).await {
            Ok(model) => model,
            Err(error) => {
                if let Err(cleanup_error) = self.storage.delete(&path).await {
                    tracing::error!(
                        path = %path,
                        error = %cleanup_error,
                        "Failed to compensate media storage after database insert failure"
                    );
                }
                return Err(error.into());
            }
        };
        Ok(self.to_item(model))
    }

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
        Ok((
            items.into_iter().map(|model| self.to_item(model)).collect(),
            total,
        ))
    }

    pub async fn get_asset_summary(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        alt: Option<String>,
    ) -> Result<MediaAssetSummary> {
        let item = self.get(tenant_id, id).await?;
        Ok(MediaAssetSummary::from_media_item(&item, alt))
    }

    pub async fn list_asset_summaries(
        &self,
        tenant_id: Uuid,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<MediaAssetSummary>, u64)> {
        let (items, total) = self.list(tenant_id, limit, offset).await?;
        Ok((
            items
                .iter()
                .map(|item| MediaAssetSummary::from_media_item(item, None))
                .collect(),
            total,
        ))
    }

    pub async fn delete(&self, tenant_id: Uuid, id: Uuid) -> Result<()> {
        let model = MediaEntity::find_by_id(id)
            .filter(MediaCol::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MediaError::NotFound(id))?;

        if let Err(error) = self.storage.delete(&model.storage_path).await {
            tracing::warn!(
                media_id = %id,
                path = %model.storage_path,
                error = %error,
                "Failed to delete media object from storage; DB record will still be removed"
            );
        }

        MediaEntity::delete_by_id(id).exec(&self.db).await?;
        Ok(())
    }

    pub async fn upsert_translation(
        &self,
        tenant_id: Uuid,
        media_id: Uuid,
        input: UpsertTranslationInput,
    ) -> Result<MediaTranslationItem> {
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
            .map(|model| MediaTranslationItem {
                id: model.id,
                media_id: model.media_id,
                locale: model.locale,
                title: model.title,
                alt_text: model.alt_text,
                caption: model.caption,
            })
            .collect())
    }

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

        self.cleanup_storage_rows(rows).await
    }

    pub async fn cleanup_storage_orphans_all_tenants(
        &self,
        limit: u64,
    ) -> Result<MediaStorageCleanupReport> {
        let rows = MediaEntity::find()
            .order_by_asc(MediaCol::CreatedAt)
            .limit(limit)
            .all(&self.db)
            .await?;

        self.cleanup_storage_rows(rows).await
    }

    async fn cleanup_storage_rows(
        &self,
        rows: Vec<media::Model>,
    ) -> Result<MediaStorageCleanupReport> {
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

    fn to_item(&self, model: media::Model) -> MediaItem {
        let public_url = self.storage.public_url(&model.storage_path);
        MediaItem {
            id: model.id,
            tenant_id: model.tenant_id,
            uploaded_by: model.uploaded_by,
            filename: model.filename,
            original_name: model.original_name,
            mime_type: model.mime_type,
            size: model.size,
            storage_path: model.storage_path,
            storage_driver: model.storage_driver,
            public_url,
            width: model.width,
            height: model.height,
            metadata: model.metadata,
            created_at: model.created_at.with_timezone(&Utc),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use bytes::Bytes;
    use uuid::Uuid;

    use super::{
        classify_cleanup_probe, normalize_original_name, validate_upload_policy, verify_media_type,
        MediaStorageCleanupDecision, MediaStorageCleanupReport,
    };
    use crate::{
        dto::{UploadInput, DEFAULT_MAX_SIZE},
        error::MediaError,
    };
    use rustok_storage::StorageError;

    fn upload_input(content_type: &str, data: Vec<u8>) -> UploadInput {
        UploadInput {
            tenant_id: Uuid::new_v4(),
            uploaded_by: None,
            original_name: "asset.bin".to_string(),
            content_type: content_type.to_string(),
            data: Bytes::from(data),
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
    fn validate_upload_policy_accepts_supported_signature() {
        let input = upload_input("image/png", b"\x89PNG\r\n\x1a\nrest".to_vec());
        let (size, media_type) = validate_upload_policy(&input).expect("valid PNG");
        assert_eq!(size, input.data.len() as u64);
        assert_eq!(media_type.mime_type, "image/png");
        assert_eq!(media_type.extension, "png");
    }

    #[test]
    fn validate_upload_policy_rejects_unsupported_or_spoofed_content() {
        let html = upload_input("image/png", b"<html><script>alert(1)</script>".to_vec());
        assert!(matches!(
            validate_upload_policy(&html),
            Err(MediaError::InvalidMediaContent { .. })
        ));

        let svg = upload_input("image/svg+xml", b"<svg onload='alert(1)'>".to_vec());
        assert!(matches!(
            validate_upload_policy(&svg),
            Err(MediaError::UnsupportedMimeType(_))
        ));
    }

    #[test]
    fn validate_upload_policy_rejects_payloads_over_limit() {
        let input = upload_input("application/pdf", vec![0_u8; DEFAULT_MAX_SIZE as usize + 1]);
        let error = validate_upload_policy(&input).expect_err("oversized upload must be rejected");
        assert!(matches!(
            error,
            MediaError::FileTooLarge { size, max }
                if size == DEFAULT_MAX_SIZE + 1 && max == DEFAULT_MAX_SIZE
        ));
    }

    #[test]
    fn content_type_parameters_are_normalized() {
        let media_type = verify_media_type("application/pdf; charset=binary", b"%PDF-1.7\n")
            .expect("PDF signature");
        assert_eq!(media_type.mime_type, "application/pdf");
    }

    #[test]
    fn uploaded_display_name_cannot_preserve_path_components() {
        assert_eq!(normalize_original_name("../../evil.png", "png"), "evil.png");
        assert_eq!(
            normalize_original_name("..\\..\\evil.png", "png"),
            "evil.png"
        );
        assert_eq!(normalize_original_name("...", "jpg"), "upload.jpg");
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
