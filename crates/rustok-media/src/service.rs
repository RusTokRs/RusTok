use chrono::Utc;
use object_store::{ObjectStoreExt, path::Path};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use rustok_core::generate_id;
use rustok_storage::{ObjectKey, ObjectScope, ObjectZone, StorageRuntime};

use crate::{
    dto::{
        CreateRenditionInput, DEFAULT_MAX_SIZE, MediaAssetSummary, MediaItem, MediaRenditionItem,
        MediaTranslationItem, PrepareUploadSessionInput, PreparedUploadSession, UploadInput,
        UpsertTranslationInput,
    },
    entities::{
        asset::{ActiveModel as AssetActiveModel, Column as AssetCol, Entity as AssetEntity},
        blob::{self, ActiveModel as BlobActiveModel, Column as BlobCol, Entity as BlobEntity},
        media_translation::{
            ActiveModel as TranslationActiveModel, Column as TransCol, Entity as TransEntity,
        },
        rendition::{
            ActiveModel as RenditionActiveModel, Column as RenditionCol, Entity as RenditionEntity,
        },
        upload_session::{
            ActiveModel as UploadSessionActiveModel, Column as UploadSessionCol,
            Entity as UploadSessionEntity,
        },
    },
    error::{MediaError, Result},
    image::{ImageProcessingLimits, ImageWorker, inspect_image},
    lifecycle::{AssetState, BlobState, RenditionState, UploadState},
};

pub struct MediaService {
    db: DatabaseConnection,
    storage: StorageRuntime,
    image_worker: ImageWorker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaReconciliationDecision {
    Healthy,
    MarkMissing,
    RetryLater,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct MediaReconciliationReport {
    pub inspected: u64,
    pub healthy: u64,
    pub missing_marked: u64,
    pub deletions_completed: u64,
    pub upload_sessions_inspected: u64,
    pub upload_sessions_expired: u64,
    pub staging_objects_deleted: u64,
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

impl MediaReconciliationReport {
    pub fn is_empty(&self) -> bool {
        self.inspected == 0 && self.upload_sessions_inspected == 0
    }

    pub fn changed_records(&self) -> u64 {
        self.missing_marked
            + self.deletions_completed
            + self.upload_sessions_expired
            + self.staging_objects_deleted
    }

    pub fn completed_without_retry(&self) -> bool {
        self.retry_later == 0
    }

    pub fn should_retry(&self) -> bool {
        self.retry_later > 0
    }

    fn merge(&mut self, other: Self) {
        self.inspected += other.inspected;
        self.healthy += other.healthy;
        self.missing_marked += other.missing_marked;
        self.deletions_completed += other.deletions_completed;
        self.upload_sessions_inspected += other.upload_sessions_inspected;
        self.upload_sessions_expired += other.upload_sessions_expired;
        self.staging_objects_deleted += other.staging_objects_deleted;
        self.retry_later += other.retry_later;
    }
}

pub async fn load_media_usage_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<MediaUsageSnapshot> {
    let file_count = AssetEntity::find()
        .filter(AssetCol::TenantId.eq(tenant_id))
        .filter(AssetCol::LifecycleState.eq(AssetState::Active.as_str()))
        .count(db)
        .await? as i64;

    let total_bytes = BlobEntity::find()
        .filter(BlobCol::TenantId.eq(tenant_id))
        .filter(BlobCol::State.eq(BlobState::Ready.as_str()))
        .select_only()
        .column_as(sea_orm::sea_query::Expr::col(BlobCol::Size).sum(), "total")
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

fn classify_reconciliation_probe(
    result: &std::result::Result<object_store::ObjectMeta, object_store::Error>,
) -> MediaReconciliationDecision {
    match result {
        Ok(_) => MediaReconciliationDecision::Healthy,
        Err(object_store::Error::NotFound { .. }) => MediaReconciliationDecision::MarkMissing,
        Err(_) => MediaReconciliationDecision::RetryLater,
    }
}

fn record_reconciliation_metrics(report: &MediaReconciliationReport) {
    for (outcome, count) in [
        ("healthy", report.healthy),
        ("missing_marked", report.missing_marked),
        ("deletions_completed", report.deletions_completed),
        ("upload_sessions_expired", report.upload_sessions_expired),
        ("staging_objects_deleted", report.staging_objects_deleted),
        ("retry_later", report.retry_later),
    ] {
        if count > 0 {
            rustok_telemetry::metrics::record_media_reconciliation(outcome, count);
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

fn normalize_rendition_purpose(value: &str) -> Result<String> {
    let purpose = value.trim().to_ascii_lowercase().replace('_', "-");
    let valid = !purpose.is_empty()
        && purpose.len() <= 64
        && !purpose.starts_with('-')
        && !purpose.ends_with('-')
        && purpose
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    valid
        .then_some(purpose)
        .ok_or_else(|| MediaError::InvalidRenditionPurpose(value.to_string()))
}

impl MediaService {
    pub fn new(db: DatabaseConnection, storage: StorageRuntime) -> Self {
        Self {
            db,
            storage,
            image_worker: ImageWorker::production(),
        }
    }

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn with_image_worker(mut self, image_worker: ImageWorker) -> Self {
        self.image_worker = image_worker;
        self
    }

    pub async fn usage_snapshot(&self, tenant_id: Uuid) -> Result<MediaUsageSnapshot> {
        load_media_usage_snapshot(&self.db, tenant_id).await
    }

    /// Validate, store, and record a new media upload.
    pub async fn upload(&self, input: UploadInput) -> Result<MediaItem> {
        self.persist_upload(input, None).await
    }

    async fn persist_upload(
        &self,
        input: UploadInput,
        upload_session_id: Option<Uuid>,
    ) -> Result<MediaItem> {
        if let Some(upload_session_id) = upload_session_id
            && let Some(existing) = AssetEntity::find()
                .filter(AssetCol::TenantId.eq(input.tenant_id))
                .filter(AssetCol::UploadSessionId.eq(upload_session_id))
                .one(&self.db)
                .await?
        {
            return self.get(input.tenant_id, existing.id).await;
        }
        let (size, verified) = validate_upload_policy(&input)?;
        let original_name = normalize_original_name(&input.original_name, verified.extension);
        let asset_id = generate_id();
        let blob_id = generate_id();
        let now = Utc::now();
        let key = ObjectKey::chronological(
            "media",
            ObjectZone::Objects,
            ObjectScope::Tenant(input.tenant_id),
            now,
            blob_id,
            verified.extension,
        )?;
        let checksum_sha256 = hex::encode(Sha256::digest(input.data.as_ref()));
        let dimensions = if verified.mime_type.starts_with("image/") {
            let (width, height) = inspect_image(&input.data, ImageProcessingLimits::default())?;
            Some((width as i32, height as i32))
        } else {
            None
        };
        self.storage
            .objects
            .put_opts(
                key.as_path(),
                input.data.clone().into(),
                self.storage.put_options(verified.mime_type),
            )
            .await?;
        let path = key.to_string();

        let timestamp = now.fixed_offset();
        let persistence = async {
            let transaction = self.db.begin().await?;
            AssetActiveModel {
                id: Set(asset_id),
                tenant_id: Set(input.tenant_id),
                uploaded_by: Set(input.uploaded_by),
                upload_session_id: Set(upload_session_id),
                active_blob_id: Set(None),
                original_name: Set(original_name),
                lifecycle_state: Set(AssetState::Active.as_str().to_string()),
                metadata: Set(serde_json::json!({})),
                created_at: Set(timestamp),
                updated_at: Set(timestamp),
                delete_requested_at: Set(None),
                deleted_at: Set(None),
            }
            .insert(&transaction)
            .await?;
            BlobActiveModel {
                id: Set(blob_id),
                tenant_id: Set(input.tenant_id),
                asset_id: Set(asset_id),
                object_key: Set(path.clone()),
                mime_type: Set(verified.mime_type.to_string()),
                size: Set(size as i64),
                checksum_sha256: Set(checksum_sha256),
                width: Set(dimensions.map(|(width, _)| width)),
                height: Set(dimensions.map(|(_, height)| height)),
                state: Set(BlobState::Ready.as_str().to_string()),
                created_at: Set(timestamp),
                ready_at: Set(Some(timestamp)),
                delete_requested_at: Set(None),
                deleted_at: Set(None),
                reconcile_attempts: Set(0),
                last_reconciled_at: Set(timestamp),
                last_error: Set(None),
            }
            .insert(&transaction)
            .await?;
            AssetEntity::update_many()
                .col_expr(
                    AssetCol::ActiveBlobId,
                    sea_orm::sea_query::Expr::value(blob_id),
                )
                .filter(AssetCol::Id.eq(asset_id))
                .exec(&transaction)
                .await?;
            transaction.commit().await
        }
        .await;

        match persistence {
            Ok(()) => {
                let item = self.get(input.tenant_id, asset_id).await?;
                rustok_telemetry::metrics::record_media_upload(
                    &input.tenant_id.to_string(),
                    &item.mime_type,
                    item.size as u64,
                );
                Ok(item)
            }
            Err(error) => {
                match BlobEntity::find_by_id(blob_id)
                    .filter(BlobCol::TenantId.eq(input.tenant_id))
                    .filter(BlobCol::ObjectKey.eq(&path))
                    .one(&self.db)
                    .await
                {
                    Ok(Some(_)) => return self.get(input.tenant_id, asset_id).await,
                    Ok(None) => {
                        if let Err(cleanup_error) = self.storage.objects.delete(key.as_path()).await
                        {
                            tracing::error!(
                                path = %path,
                                error = %cleanup_error,
                                "Failed to compensate uncommitted media object"
                            );
                        }
                    }
                    Err(verification_error) => {
                        tracing::error!(
                            path = %path,
                            error = %verification_error,
                            "Media commit outcome is unknown; preserving object for reconciliation"
                        );
                    }
                }
                Err(error.into())
            }
        }
    }

    pub fn supports_presigned_upload(&self) -> bool {
        self.storage.signer.is_some()
    }

    pub async fn prepare_upload_session(
        &self,
        input: PrepareUploadSessionInput,
    ) -> Result<PreparedUploadSession> {
        self.prepare_upload_session_with_id(generate_id(), input)
            .await
    }

    pub(crate) async fn prepare_upload_session_with_id(
        &self,
        id: Uuid,
        input: PrepareUploadSessionInput,
    ) -> Result<PreparedUploadSession> {
        if !self.supports_presigned_upload() {
            return Err(MediaError::PresignedUploadUnavailable);
        }
        if input.expires_in.is_zero() || input.expires_in > std::time::Duration::from_secs(900) {
            return Err(MediaError::InvalidMediaContent {
                declared: input.content_type,
                reason: "upload session expiry must be between 1 second and 15 minutes".to_string(),
            });
        }
        if input.content_length == Some(0)
            || input
                .content_length
                .is_some_and(|size| size > DEFAULT_MAX_SIZE)
        {
            return Err(MediaError::FileTooLarge {
                size: input.content_length.unwrap_or_default(),
                max: DEFAULT_MAX_SIZE,
            });
        }
        let content_type = input
            .content_type
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if !is_supported_declared_type(&content_type) {
            return Err(MediaError::UnsupportedMimeType(content_type));
        }

        if let Some(existing) = UploadSessionEntity::find_by_id(id)
            .filter(UploadSessionCol::TenantId.eq(input.tenant_id))
            .one(&self.db)
            .await?
        {
            let now = Utc::now().fixed_offset();
            if existing.expires_at <= now {
                return Err(MediaError::UploadSessionExpired(id));
            }
            let expires_in = (existing.expires_at - now).to_std().map_err(|error| {
                MediaError::InvalidMediaContent {
                    declared: existing.expected_mime_type.clone(),
                    reason: error.to_string(),
                }
            })?;
            return self
                .sign_upload_session(id, &existing.staging_key, existing.expires_at, expires_in)
                .await;
        }

        let created_at = Utc::now();
        let expires_at = created_at
            + chrono::Duration::from_std(input.expires_in).map_err(|error| {
                MediaError::InvalidMediaContent {
                    declared: content_type.clone(),
                    reason: error.to_string(),
                }
            })?;
        let key = ObjectKey::chronological(
            "media",
            ObjectZone::Staging,
            ObjectScope::Tenant(input.tenant_id),
            created_at,
            id,
            "upload",
        )?;
        let timestamp = created_at.fixed_offset();
        UploadSessionActiveModel {
            id: Set(id),
            tenant_id: Set(input.tenant_id),
            actor_id: Set(input.actor_id),
            staging_key: Set(key.to_string()),
            original_name: Set(normalize_original_name(&input.original_name, "bin")),
            expected_mime_type: Set(content_type),
            expected_size: Set(input.content_length.map(|size| size as i64)),
            state: Set(UploadState::Pending.as_str().to_string()),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
            expires_at: Set(expires_at.fixed_offset()),
            completed_at: Set(None),
            staging_deleted_at: Set(None),
            last_error: Set(None),
        }
        .insert(&self.db)
        .await?;

        self.sign_upload_session(
            id,
            &key.to_string(),
            expires_at.fixed_offset(),
            input.expires_in,
        )
        .await
    }

    async fn sign_upload_session(
        &self,
        id: Uuid,
        staging_key: &str,
        expires_at: chrono::DateTime<chrono::FixedOffset>,
        expires_in: std::time::Duration,
    ) -> Result<PreparedUploadSession> {
        match self
            .storage
            .signed_upload_url(&Path::from(staging_key), expires_in)
            .await
        {
            Ok(Some(endpoint)) => {
                rustok_telemetry::metrics::record_media_upload_session("prepared");
                Ok(PreparedUploadSession {
                    id,
                    endpoint,
                    expires_at: expires_at.with_timezone(&Utc),
                })
            }
            Ok(None) => {
                rustok_telemetry::metrics::record_media_upload_session("prepare_failed");
                self.mark_upload_session_failed(id, "storage signer is unavailable")
                    .await;
                Err(MediaError::PresignedUploadUnavailable)
            }
            Err(error) => {
                rustok_telemetry::metrics::record_media_upload_session("prepare_failed");
                self.mark_upload_session_failed(id, &error.to_string())
                    .await;
                Err(error.into())
            }
        }
    }

    pub async fn complete_upload_session(
        &self,
        tenant_id: Uuid,
        session_id: Uuid,
    ) -> Result<MediaItem> {
        let session = UploadSessionEntity::find_by_id(session_id)
            .filter(UploadSessionCol::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MediaError::NotFound(session_id))?;
        if let Some(asset) = AssetEntity::find()
            .filter(AssetCol::TenantId.eq(tenant_id))
            .filter(AssetCol::UploadSessionId.eq(session_id))
            .one(&self.db)
            .await?
        {
            self.complete_upload_session_row(&session).await;
            rustok_telemetry::metrics::record_media_upload_session("reused");
            return self.get(tenant_id, asset.id).await;
        }
        if session.expires_at <= Utc::now().fixed_offset() {
            UploadSessionEntity::update_many()
                .col_expr(
                    UploadSessionCol::State,
                    sea_orm::sea_query::Expr::value(UploadState::Expired.as_str()),
                )
                .col_expr(
                    UploadSessionCol::UpdatedAt,
                    sea_orm::sea_query::Expr::value(Utc::now().fixed_offset()),
                )
                .filter(UploadSessionCol::Id.eq(session_id))
                .exec(&self.db)
                .await?;
            return Err(MediaError::UploadSessionExpired(session_id));
        }
        UploadSessionEntity::update_many()
            .col_expr(
                UploadSessionCol::State,
                sea_orm::sea_query::Expr::value(UploadState::Finalizing.as_str()),
            )
            .col_expr(
                UploadSessionCol::UpdatedAt,
                sea_orm::sea_query::Expr::value(Utc::now().fixed_offset()),
            )
            .filter(UploadSessionCol::Id.eq(session_id))
            .exec(&self.db)
            .await?;

        let data = match self
            .storage
            .objects
            .get(&Path::from(session.staging_key.as_str()))
            .await
        {
            Ok(result) => {
                let staged_size = result.meta.size;
                if staged_size > DEFAULT_MAX_SIZE {
                    let error = MediaError::FileTooLarge {
                        size: staged_size,
                        max: DEFAULT_MAX_SIZE,
                    };
                    self.mark_upload_session_failed(session_id, &error.to_string())
                        .await;
                    return Err(error);
                }
                if session
                    .expected_size
                    .is_some_and(|expected| expected != staged_size as i64)
                {
                    let reason = format!(
                        "staged object has {staged_size} bytes; expected {}",
                        session.expected_size.unwrap_or_default()
                    );
                    self.mark_upload_session_failed(session_id, &reason).await;
                    return Err(MediaError::InvalidMediaContent {
                        declared: session.expected_mime_type,
                        reason,
                    });
                }
                match result.bytes().await {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        self.mark_upload_session_failed(session_id, &error.to_string())
                            .await;
                        return Err(error.into());
                    }
                }
            }
            Err(error) => {
                self.mark_upload_session_failed(session_id, &error.to_string())
                    .await;
                return Err(error.into());
            }
        };
        if session
            .expected_size
            .is_some_and(|expected| expected != data.len() as i64)
        {
            let reason = format!(
                "staged object has {} bytes; expected {}",
                data.len(),
                session.expected_size.unwrap_or_default()
            );
            self.mark_upload_session_failed(session_id, &reason).await;
            return Err(MediaError::InvalidMediaContent {
                declared: session.expected_mime_type,
                reason,
            });
        }

        let item = match self
            .persist_upload(
                UploadInput {
                    tenant_id,
                    uploaded_by: session.actor_id,
                    original_name: session.original_name.clone(),
                    content_type: session.expected_mime_type.clone(),
                    data,
                },
                Some(session_id),
            )
            .await
        {
            Ok(item) => item,
            Err(error) => {
                self.mark_upload_session_failed(session_id, &error.to_string())
                    .await;
                return Err(error);
            }
        };
        self.complete_upload_session_row(&session).await;
        rustok_telemetry::metrics::record_media_upload_session("completed");
        Ok(item)
    }

    async fn complete_upload_session_row(&self, session: &crate::entities::upload_session::Model) {
        let now = Utc::now().fixed_offset();
        let path = Path::from(session.staging_key.as_str());
        let deletion = self.storage.objects.delete(&path).await;
        let (staging_deleted_at, last_error) = match deletion {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => (Some(now), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let result = UploadSessionEntity::update_many()
            .col_expr(
                UploadSessionCol::State,
                sea_orm::sea_query::Expr::value(UploadState::Completed.as_str()),
            )
            .col_expr(
                UploadSessionCol::UpdatedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .col_expr(
                UploadSessionCol::CompletedAt,
                sea_orm::sea_query::Expr::value(Some(now)),
            )
            .col_expr(
                UploadSessionCol::StagingDeletedAt,
                sea_orm::sea_query::Expr::value(staging_deleted_at),
            )
            .col_expr(
                UploadSessionCol::LastError,
                sea_orm::sea_query::Expr::value(last_error),
            )
            .filter(UploadSessionCol::Id.eq(session.id))
            .exec(&self.db)
            .await;
        if let Err(error) = result {
            tracing::error!(
                upload_session_id = %session.id,
                error = %error,
                "Failed to persist completed upload session state"
            );
        }
    }

    async fn mark_upload_session_failed(&self, session_id: Uuid, error: &str) {
        let result = UploadSessionEntity::update_many()
            .col_expr(
                UploadSessionCol::State,
                sea_orm::sea_query::Expr::value(UploadState::Failed.as_str()),
            )
            .col_expr(
                UploadSessionCol::UpdatedAt,
                sea_orm::sea_query::Expr::value(Utc::now().fixed_offset()),
            )
            .col_expr(
                UploadSessionCol::LastError,
                sea_orm::sea_query::Expr::value(error.chars().take(4_096).collect::<String>()),
            )
            .filter(UploadSessionCol::Id.eq(session_id))
            .exec(&self.db)
            .await;
        if let Err(database_error) = result {
            tracing::error!(
                upload_session_id = %session_id,
                error = %database_error,
                "Failed to persist upload session failure state"
            );
        }
    }

    /// Build or return the immutable rendition identified by source blob and recipe digest.
    pub async fn create_rendition(
        &self,
        input: CreateRenditionInput,
    ) -> Result<MediaRenditionItem> {
        let purpose = normalize_rendition_purpose(&input.purpose)?;
        input
            .recipe
            .validate(crate::image::ImageProcessingLimits::default())?;
        let recipe_hash = input.recipe.digest()?;
        let (asset, source_blob) = AssetEntity::find_by_id(input.asset_id)
            .filter(AssetCol::TenantId.eq(input.tenant_id))
            .filter(AssetCol::LifecycleState.eq(AssetState::Active.as_str()))
            .find_also_related(BlobEntity)
            .one(&self.db)
            .await?
            .ok_or(MediaError::NotFound(input.asset_id))?;
        let source_blob = source_blob.ok_or(MediaError::NotFound(input.asset_id))?;
        if !source_blob.mime_type.starts_with("image/") {
            return Err(MediaError::UnsupportedMimeType(source_blob.mime_type));
        }

        let now = Utc::now().fixed_offset();
        let existing = RenditionEntity::find()
            .filter(RenditionCol::SourceBlobId.eq(source_blob.id))
            .filter(RenditionCol::RecipeHash.eq(&recipe_hash))
            .one(&self.db)
            .await?;
        let (rendition_id, expected_state, expected_updated_at) = if let Some(existing) = existing {
            if existing.state == RenditionState::Ready.as_str()
                && let Some(item) = self.ready_rendition_item(&existing).await?
            {
                return Ok(item);
            }
            let stale_before = now - chrono::Duration::minutes(5);
            if matches!(
                existing.state.as_str(),
                state if state == RenditionState::Pending.as_str()
                    || state == RenditionState::Processing.as_str()
            ) && existing.updated_at > stale_before
            {
                return Err(MediaError::RenditionInProgress(existing.id));
            }
            (existing.id, existing.state, existing.updated_at)
        } else {
            let rendition_id = generate_id();
            let insert = RenditionActiveModel {
                id: Set(rendition_id),
                tenant_id: Set(input.tenant_id),
                asset_id: Set(asset.id),
                source_blob_id: Set(source_blob.id),
                result_blob_id: Set(None),
                recipe_hash: Set(recipe_hash.clone()),
                recipe: Set(serde_json::to_value(&input.recipe)?),
                purpose: Set(purpose.clone()),
                state: Set(RenditionState::Pending.as_str().to_string()),
                created_at: Set(now),
                updated_at: Set(now),
                last_error: Set(None),
            }
            .insert(&self.db)
            .await;
            if let Err(error) = insert {
                if let Some(concurrent) = RenditionEntity::find()
                    .filter(RenditionCol::SourceBlobId.eq(source_blob.id))
                    .filter(RenditionCol::RecipeHash.eq(&recipe_hash))
                    .one(&self.db)
                    .await?
                {
                    return Err(MediaError::RenditionInProgress(concurrent.id));
                }
                return Err(error.into());
            }
            (
                rendition_id,
                RenditionState::Pending.as_str().to_string(),
                now,
            )
        };

        let claim = RenditionEntity::update_many()
            .col_expr(
                RenditionCol::State,
                sea_orm::sea_query::Expr::value(RenditionState::Processing.as_str()),
            )
            .col_expr(
                RenditionCol::UpdatedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .col_expr(
                RenditionCol::LastError,
                sea_orm::sea_query::Expr::value(Option::<String>::None),
            )
            .filter(RenditionCol::Id.eq(rendition_id))
            .filter(RenditionCol::State.eq(expected_state))
            .filter(RenditionCol::UpdatedAt.eq(expected_updated_at))
            .exec(&self.db)
            .await?;
        if claim.rows_affected != 1 {
            return Err(MediaError::RenditionInProgress(rendition_id));
        }

        let source = match self
            .storage
            .objects
            .get(&Path::from(source_blob.object_key.as_str()))
            .await
        {
            Ok(result) => match result.bytes().await {
                Ok(bytes) => bytes,
                Err(error) => {
                    self.mark_rendition_failed(rendition_id, &error.to_string())
                        .await;
                    return Err(error.into());
                }
            },
            Err(error) => {
                self.mark_rendition_failed(rendition_id, &error.to_string())
                    .await;
                return Err(error.into());
            }
        };
        let output_format = input.recipe.output.extension();
        let processing_started = std::time::Instant::now();
        let output = match self
            .image_worker
            .process(source.to_vec(), input.recipe)
            .await
        {
            Ok(output) => output,
            Err(error) => {
                rustok_telemetry::metrics::record_media_rendition(
                    output_format,
                    "failed",
                    processing_started.elapsed().as_secs_f64(),
                );
                self.mark_rendition_failed(rendition_id, &error.to_string())
                    .await;
                return Err(error.into());
            }
        };

        let result_blob_id = generate_id();
        let created_at = Utc::now();
        let key = ObjectKey::chronological(
            "media",
            ObjectZone::Objects,
            ObjectScope::Tenant(input.tenant_id),
            created_at,
            result_blob_id,
            output.extension,
        )?;
        let object_key = key.to_string();
        let size = output.bytes.len() as i64;
        let checksum_sha256 = hex::encode(Sha256::digest(&output.bytes));
        if let Err(error) = self
            .storage
            .objects
            .put_opts(
                key.as_path(),
                output.bytes.into(),
                self.storage.put_options(output.mime_type),
            )
            .await
        {
            rustok_telemetry::metrics::record_media_rendition(
                output_format,
                "failed",
                processing_started.elapsed().as_secs_f64(),
            );
            self.mark_rendition_failed(rendition_id, &error.to_string())
                .await;
            return Err(error.into());
        }

        let timestamp = created_at.fixed_offset();
        let persistence = async {
            let transaction = self.db.begin().await?;
            BlobActiveModel {
                id: Set(result_blob_id),
                tenant_id: Set(input.tenant_id),
                asset_id: Set(asset.id),
                object_key: Set(object_key.clone()),
                mime_type: Set(output.mime_type.to_string()),
                size: Set(size),
                checksum_sha256: Set(checksum_sha256),
                width: Set(Some(output.width as i32)),
                height: Set(Some(output.height as i32)),
                state: Set(BlobState::Ready.as_str().to_string()),
                created_at: Set(timestamp),
                ready_at: Set(Some(timestamp)),
                delete_requested_at: Set(None),
                deleted_at: Set(None),
                reconcile_attempts: Set(0),
                last_reconciled_at: Set(timestamp),
                last_error: Set(None),
            }
            .insert(&transaction)
            .await?;
            RenditionEntity::update_many()
                .col_expr(
                    RenditionCol::ResultBlobId,
                    sea_orm::sea_query::Expr::value(result_blob_id),
                )
                .col_expr(
                    RenditionCol::State,
                    sea_orm::sea_query::Expr::value(RenditionState::Ready.as_str()),
                )
                .col_expr(
                    RenditionCol::UpdatedAt,
                    sea_orm::sea_query::Expr::value(timestamp),
                )
                .filter(RenditionCol::Id.eq(rendition_id))
                .exec(&transaction)
                .await?;
            transaction.commit().await
        }
        .await;

        if let Err(error) = persistence {
            match RenditionEntity::find_by_id(rendition_id)
                .one(&self.db)
                .await
            {
                Ok(Some(rendition)) => match self.ready_rendition_item(&rendition).await {
                    Ok(Some(item)) => {
                        rustok_telemetry::metrics::record_media_rendition(
                            output_format,
                            "ready",
                            processing_started.elapsed().as_secs_f64(),
                        );
                        return Ok(item);
                    }
                    Ok(None) => {}
                    Err(verification_error) => {
                        tracing::error!(
                            path = %object_key,
                            error = %verification_error,
                            "Rendition commit outcome is unknown; preserving object"
                        );
                        return Err(error.into());
                    }
                },
                Ok(None) => {}
                Err(verification_error) => {
                    tracing::error!(
                        path = %object_key,
                        error = %verification_error,
                        "Rendition commit outcome is unknown; preserving object"
                    );
                    return Err(error.into());
                }
            }
            if let Err(cleanup_error) = self.storage.objects.delete(key.as_path()).await {
                tracing::error!(
                    path = %object_key,
                    error = %cleanup_error,
                    "Failed to compensate uncommitted rendition object"
                );
            }
            self.mark_rendition_failed(rendition_id, &error.to_string())
                .await;
            rustok_telemetry::metrics::record_media_rendition(
                output_format,
                "failed",
                processing_started.elapsed().as_secs_f64(),
            );
            return Err(error.into());
        }

        rustok_telemetry::metrics::record_media_rendition(
            output_format,
            "ready",
            processing_started.elapsed().as_secs_f64(),
        );

        Ok(MediaRenditionItem {
            id: rendition_id,
            asset_id: asset.id,
            source_blob_id: source_blob.id,
            result_blob_id,
            purpose,
            recipe_hash,
            mime_type: output.mime_type.to_string(),
            size,
            width: output.width as i32,
            height: output.height as i32,
            public_url: self
                .storage
                .public_url(key.as_path())
                .unwrap_or_else(|| object_key.clone()),
            storage_path: object_key,
        })
    }

    async fn ready_rendition_item(
        &self,
        rendition: &crate::entities::rendition::Model,
    ) -> Result<Option<MediaRenditionItem>> {
        let Some(result_blob_id) = rendition.result_blob_id else {
            return Ok(None);
        };
        let Some(blob) = BlobEntity::find_by_id(result_blob_id)
            .filter(BlobCol::State.eq(BlobState::Ready.as_str()))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let path = Path::from(blob.object_key.as_str());
        Ok(Some(MediaRenditionItem {
            id: rendition.id,
            asset_id: rendition.asset_id,
            source_blob_id: rendition.source_blob_id,
            result_blob_id,
            purpose: rendition.purpose.clone(),
            recipe_hash: rendition.recipe_hash.clone(),
            mime_type: blob.mime_type,
            size: blob.size,
            width: blob.width.unwrap_or_default(),
            height: blob.height.unwrap_or_default(),
            public_url: self
                .storage
                .public_url(&path)
                .unwrap_or_else(|| blob.object_key.clone()),
            storage_path: blob.object_key,
        }))
    }

    async fn mark_rendition_failed(&self, rendition_id: Uuid, error: &str) {
        let result = RenditionEntity::update_many()
            .col_expr(
                RenditionCol::State,
                sea_orm::sea_query::Expr::value(RenditionState::Failed.as_str()),
            )
            .col_expr(
                RenditionCol::UpdatedAt,
                sea_orm::sea_query::Expr::value(Utc::now().fixed_offset()),
            )
            .col_expr(
                RenditionCol::LastError,
                sea_orm::sea_query::Expr::value(error.chars().take(4_096).collect::<String>()),
            )
            .filter(RenditionCol::Id.eq(rendition_id))
            .exec(&self.db)
            .await;
        if let Err(database_error) = result {
            tracing::error!(
                rendition_id = %rendition_id,
                error = %database_error,
                "Failed to persist rendition failure state"
            );
        }
    }

    pub async fn get(&self, tenant_id: Uuid, id: Uuid) -> Result<MediaItem> {
        let (asset, blob) = AssetEntity::find_by_id(id)
            .filter(AssetCol::TenantId.eq(tenant_id))
            .filter(AssetCol::LifecycleState.eq(AssetState::Active.as_str()))
            .find_also_related(BlobEntity)
            .one(&self.db)
            .await?
            .ok_or(MediaError::NotFound(id))?;
        let blob = blob.ok_or(MediaError::NotFound(id))?;
        Ok(self.to_item(asset, blob))
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<MediaItem>, u64)> {
        let query = AssetEntity::find()
            .filter(AssetCol::TenantId.eq(tenant_id))
            .filter(AssetCol::LifecycleState.eq(AssetState::Active.as_str()))
            .order_by_desc(AssetCol::CreatedAt);

        let total = query.clone().count(&self.db).await?;
        let rows = query
            .find_also_related(BlobEntity)
            .limit(limit)
            .offset(offset)
            .all(&self.db)
            .await?;
        Ok((
            rows.into_iter()
                .filter_map(|(asset, blob)| blob.map(|blob| self.to_item(asset, blob)))
                .collect(),
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
        let asset = AssetEntity::find_by_id(id)
            .filter(AssetCol::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MediaError::NotFound(id))?;
        if asset.lifecycle_state == AssetState::Deleted.as_str() {
            return Ok(());
        }
        if asset.lifecycle_state == AssetState::DeletePending.as_str() {
            self.reconcile_asset_deletion(tenant_id, id).await?;
            return Ok(());
        }
        if asset.lifecycle_state != AssetState::Active.as_str()
            && asset.lifecycle_state != AssetState::Failed.as_str()
        {
            return Err(MediaError::NotFound(id));
        }
        let now = Utc::now().fixed_offset();
        let transaction = self.db.begin().await?;
        let mut active: AssetActiveModel = asset.into();
        active.lifecycle_state = Set(AssetState::DeletePending.as_str().to_string());
        active.active_blob_id = Set(None);
        active.updated_at = Set(now);
        active.delete_requested_at = Set(Some(now));
        active.update(&transaction).await?;
        BlobEntity::update_many()
            .col_expr(
                BlobCol::State,
                sea_orm::sea_query::Expr::value(BlobState::DeletePending.as_str()),
            )
            .col_expr(
                BlobCol::DeleteRequestedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(BlobCol::AssetId.eq(id))
            .filter(BlobCol::State.ne(BlobState::Deleted.as_str()))
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        rustok_telemetry::metrics::record_media_delete(&tenant_id.to_string());
        self.reconcile_asset_deletion(tenant_id, id).await?;
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
            .filter(TransCol::TenantId.eq(tenant_id))
            .filter(TransCol::AssetId.eq(media_id))
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
                tenant_id: Set(tenant_id),
                asset_id: Set(media_id),
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
            media_id: model.asset_id,
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
            .filter(TransCol::TenantId.eq(tenant_id))
            .filter(TransCol::AssetId.eq(media_id))
            .order_by_asc(TransCol::Locale)
            .all(&self.db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|model| MediaTranslationItem {
                id: model.id,
                media_id: model.asset_id,
                locale: model.locale,
                title: model.title,
                alt_text: model.alt_text,
                caption: model.caption,
            })
            .collect())
    }

    pub async fn reconcile_storage(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> Result<MediaReconciliationReport> {
        let rows = self.reconciliation_rows(Some(tenant_id), limit).await?;

        let mut report = self.reconcile_storage_rows(rows).await?;
        self.finalize_delete_pending_assets(Some(tenant_id), limit)
            .await?;
        report.merge(
            self.reconcile_upload_sessions(Some(tenant_id), limit)
                .await?,
        );
        record_reconciliation_metrics(&report);
        Ok(report)
    }

    pub async fn reconcile_storage_all_tenants(
        &self,
        limit: u64,
    ) -> Result<MediaReconciliationReport> {
        let rows = self.reconciliation_rows(None, limit).await?;

        let mut report = self.reconcile_storage_rows(rows).await?;
        self.finalize_delete_pending_assets(None, limit).await?;
        report.merge(self.reconcile_upload_sessions(None, limit).await?);
        record_reconciliation_metrics(&report);
        Ok(report)
    }

    async fn reconcile_asset_deletion(&self, tenant_id: Uuid, asset_id: Uuid) -> Result<()> {
        let rows = BlobEntity::find()
            .filter(BlobCol::TenantId.eq(tenant_id))
            .filter(BlobCol::AssetId.eq(asset_id))
            .filter(BlobCol::State.eq(BlobState::DeletePending.as_str()))
            .all(&self.db)
            .await?;
        self.reconcile_storage_rows(rows).await?;
        self.finalize_asset_deletion(tenant_id, asset_id).await?;
        Ok(())
    }

    async fn reconciliation_rows(
        &self,
        tenant_id: Option<Uuid>,
        limit: u64,
    ) -> Result<Vec<blob::Model>> {
        let mut pending = BlobEntity::find()
            .filter(BlobCol::State.eq(BlobState::DeletePending.as_str()))
            .order_by_asc(BlobCol::DeleteRequestedAt)
            .limit(limit);
        if let Some(tenant_id) = tenant_id {
            pending = pending.filter(BlobCol::TenantId.eq(tenant_id));
        }
        let mut rows = pending.all(&self.db).await?;
        let remaining = limit.saturating_sub(rows.len() as u64);
        if remaining == 0 {
            return Ok(rows);
        }
        let mut ready = BlobEntity::find()
            .filter(BlobCol::State.eq(BlobState::Ready.as_str()))
            .order_by_asc(BlobCol::LastReconciledAt)
            .order_by_asc(BlobCol::CreatedAt)
            .limit(remaining);
        if let Some(tenant_id) = tenant_id {
            ready = ready.filter(BlobCol::TenantId.eq(tenant_id));
        }
        rows.extend(ready.all(&self.db).await?);
        Ok(rows)
    }

    async fn reconcile_storage_rows(
        &self,
        rows: Vec<blob::Model>,
    ) -> Result<MediaReconciliationReport> {
        let mut report = MediaReconciliationReport::default();
        for row in rows {
            report.inspected += 1;
            let path = Path::from(row.object_key.as_str());
            if row.state == BlobState::DeletePending.as_str() {
                match self.storage.objects.delete(&path).await {
                    Ok(()) | Err(object_store::Error::NotFound { .. }) => {
                        let mut active: BlobActiveModel = row.into();
                        let now = Utc::now().fixed_offset();
                        active.state = Set(BlobState::Deleted.as_str().to_string());
                        active.deleted_at = Set(Some(now));
                        active.last_reconciled_at = Set(now);
                        active.last_error = Set(None);
                        active.update(&self.db).await?;
                        report.deletions_completed += 1;
                    }
                    Err(error) => {
                        let object_key = row.object_key.clone();
                        let mut active: BlobActiveModel = row.into();
                        active.reconcile_attempts = Set(active
                            .reconcile_attempts
                            .as_ref()
                            .to_owned()
                            .saturating_add(1));
                        active.last_reconciled_at = Set(Utc::now().fixed_offset());
                        active.last_error = Set(Some(error.to_string()));
                        active.update(&self.db).await?;
                        report.retry_later += 1;
                        tracing::warn!(
                            path = %object_key,
                            error = %error,
                            "Media object deletion remains pending"
                        );
                    }
                }
                continue;
            }

            let probe = self.storage.objects.head(&path).await;
            match classify_reconciliation_probe(&probe) {
                MediaReconciliationDecision::Healthy => {
                    let mut active: BlobActiveModel = row.into();
                    active.last_reconciled_at = Set(Utc::now().fixed_offset());
                    active.last_error = Set(None);
                    active.update(&self.db).await?;
                    report.healthy += 1;
                }
                MediaReconciliationDecision::MarkMissing => {
                    let asset_id = row.asset_id;
                    let blob_id = row.id;
                    let tenant_id = row.tenant_id;
                    let mut active: BlobActiveModel = row.into();
                    active.state = Set(BlobState::Failed.as_str().to_string());
                    active.reconcile_attempts = Set(active
                        .reconcile_attempts
                        .as_ref()
                        .to_owned()
                        .saturating_add(1));
                    active.last_error = Set(Some("object missing from storage".to_string()));
                    active.last_reconciled_at = Set(Utc::now().fixed_offset());
                    active.update(&self.db).await?;
                    let asset = AssetEntity::find_by_id(asset_id)
                        .filter(AssetCol::TenantId.eq(tenant_id))
                        .one(&self.db)
                        .await?;
                    if asset.as_ref().and_then(|asset| asset.active_blob_id) == Some(blob_id) {
                        AssetEntity::update_many()
                            .col_expr(
                                AssetCol::LifecycleState,
                                sea_orm::sea_query::Expr::value(AssetState::Failed.as_str()),
                            )
                            .col_expr(
                                AssetCol::UpdatedAt,
                                sea_orm::sea_query::Expr::value(Utc::now().fixed_offset()),
                            )
                            .filter(AssetCol::Id.eq(asset_id))
                            .filter(AssetCol::TenantId.eq(tenant_id))
                            .filter(AssetCol::LifecycleState.eq(AssetState::Active.as_str()))
                            .exec(&self.db)
                            .await?;
                    } else {
                        RenditionEntity::update_many()
                            .col_expr(
                                RenditionCol::State,
                                sea_orm::sea_query::Expr::value(RenditionState::Failed.as_str()),
                            )
                            .col_expr(
                                RenditionCol::ResultBlobId,
                                sea_orm::sea_query::Expr::value(Option::<Uuid>::None),
                            )
                            .col_expr(
                                RenditionCol::LastError,
                                sea_orm::sea_query::Expr::value(
                                    "rendition object missing from storage",
                                ),
                            )
                            .col_expr(
                                RenditionCol::UpdatedAt,
                                sea_orm::sea_query::Expr::value(Utc::now().fixed_offset()),
                            )
                            .filter(RenditionCol::TenantId.eq(tenant_id))
                            .filter(RenditionCol::ResultBlobId.eq(blob_id))
                            .exec(&self.db)
                            .await?;
                    }
                    report.missing_marked += 1;
                }
                MediaReconciliationDecision::RetryLater => {
                    report.retry_later += 1;
                    if let Err(error) = probe {
                        let object_key = row.object_key.clone();
                        let mut active: BlobActiveModel = row.into();
                        active.reconcile_attempts = Set(active
                            .reconcile_attempts
                            .as_ref()
                            .to_owned()
                            .saturating_add(1));
                        active.last_error = Set(Some(error.to_string()));
                        active.last_reconciled_at = Set(Utc::now().fixed_offset());
                        active.update(&self.db).await?;
                        tracing::warn!(
                            path = %object_key,
                            error = %error,
                            "Media storage reconciliation will retry"
                        );
                    }
                }
            }
        }

        Ok(report)
    }

    async fn finalize_delete_pending_assets(
        &self,
        tenant_id: Option<Uuid>,
        limit: u64,
    ) -> Result<()> {
        let mut query = AssetEntity::find()
            .filter(AssetCol::LifecycleState.eq(AssetState::DeletePending.as_str()))
            .order_by_asc(AssetCol::DeleteRequestedAt)
            .limit(limit);
        if let Some(tenant_id) = tenant_id {
            query = query.filter(AssetCol::TenantId.eq(tenant_id));
        }
        for asset in query.all(&self.db).await? {
            self.finalize_asset_deletion(asset.tenant_id, asset.id)
                .await?;
        }
        Ok(())
    }

    async fn finalize_asset_deletion(&self, tenant_id: Uuid, asset_id: Uuid) -> Result<()> {
        let remaining = BlobEntity::find()
            .filter(BlobCol::TenantId.eq(tenant_id))
            .filter(BlobCol::AssetId.eq(asset_id))
            .filter(BlobCol::State.ne(BlobState::Deleted.as_str()))
            .count(&self.db)
            .await?;
        if remaining == 0 {
            let now = Utc::now().fixed_offset();
            AssetEntity::update_many()
                .col_expr(
                    AssetCol::LifecycleState,
                    sea_orm::sea_query::Expr::value(AssetState::Deleted.as_str()),
                )
                .col_expr(AssetCol::DeletedAt, sea_orm::sea_query::Expr::value(now))
                .col_expr(AssetCol::UpdatedAt, sea_orm::sea_query::Expr::value(now))
                .filter(AssetCol::TenantId.eq(tenant_id))
                .filter(AssetCol::Id.eq(asset_id))
                .filter(AssetCol::LifecycleState.eq(AssetState::DeletePending.as_str()))
                .exec(&self.db)
                .await?;
        }
        Ok(())
    }

    async fn reconcile_upload_sessions(
        &self,
        tenant_id: Option<Uuid>,
        limit: u64,
    ) -> Result<MediaReconciliationReport> {
        let now = Utc::now().fixed_offset();
        let mut query = UploadSessionEntity::find()
            .filter(UploadSessionCol::StagingDeletedAt.is_null())
            .filter(
                Condition::any()
                    .add(UploadSessionCol::ExpiresAt.lte(now))
                    .add(UploadSessionCol::State.is_in([
                        UploadState::Completed.as_str(),
                        UploadState::Failed.as_str(),
                        UploadState::Expired.as_str(),
                    ])),
            )
            .order_by_asc(UploadSessionCol::ExpiresAt)
            .limit(limit);
        if let Some(tenant_id) = tenant_id {
            query = query.filter(UploadSessionCol::TenantId.eq(tenant_id));
        }
        let sessions = query.all(&self.db).await?;
        let mut report = MediaReconciliationReport::default();

        for session in sessions {
            report.upload_sessions_inspected += 1;
            let completed_asset = AssetEntity::find()
                .filter(AssetCol::TenantId.eq(session.tenant_id))
                .filter(AssetCol::UploadSessionId.eq(session.id))
                .one(&self.db)
                .await?;
            let completed =
                completed_asset.is_some() || session.state == UploadState::Completed.as_str();
            match self
                .storage
                .objects
                .delete(&Path::from(session.staging_key.as_str()))
                .await
            {
                Ok(()) | Err(object_store::Error::NotFound { .. }) => {
                    let state = if completed {
                        UploadState::Completed
                    } else {
                        UploadState::Expired
                    };
                    UploadSessionEntity::update_many()
                        .col_expr(
                            UploadSessionCol::State,
                            sea_orm::sea_query::Expr::value(state.as_str()),
                        )
                        .col_expr(
                            UploadSessionCol::UpdatedAt,
                            sea_orm::sea_query::Expr::value(now),
                        )
                        .col_expr(
                            UploadSessionCol::CompletedAt,
                            sea_orm::sea_query::Expr::value(
                                completed.then_some(session.completed_at.unwrap_or(now)),
                            ),
                        )
                        .col_expr(
                            UploadSessionCol::StagingDeletedAt,
                            sea_orm::sea_query::Expr::value(Some(now)),
                        )
                        .col_expr(
                            UploadSessionCol::LastError,
                            sea_orm::sea_query::Expr::value(Option::<String>::None),
                        )
                        .filter(UploadSessionCol::Id.eq(session.id))
                        .exec(&self.db)
                        .await?;
                    report.staging_objects_deleted += 1;
                    if !completed {
                        report.upload_sessions_expired += 1;
                    }
                }
                Err(error) => {
                    UploadSessionEntity::update_many()
                        .col_expr(
                            UploadSessionCol::UpdatedAt,
                            sea_orm::sea_query::Expr::value(now),
                        )
                        .col_expr(
                            UploadSessionCol::LastError,
                            sea_orm::sea_query::Expr::value(error.to_string()),
                        )
                        .filter(UploadSessionCol::Id.eq(session.id))
                        .exec(&self.db)
                        .await?;
                    report.retry_later += 1;
                }
            }
        }

        Ok(report)
    }

    fn to_item(&self, asset: crate::entities::asset::Model, blob: blob::Model) -> MediaItem {
        let path = Path::from(blob.object_key.as_str());
        let public_url = self
            .storage
            .public_url(&path)
            .unwrap_or_else(|| blob.object_key.clone());
        let filename = std::path::Path::new(&blob.object_key)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&blob.object_key)
            .to_string();
        MediaItem {
            id: asset.id,
            tenant_id: asset.tenant_id,
            uploaded_by: asset.uploaded_by,
            filename,
            original_name: asset.original_name,
            mime_type: blob.mime_type,
            size: blob.size,
            storage_path: blob.object_key,
            storage_driver: self.storage.kind.as_str().to_string(),
            public_url,
            width: blob.width,
            height: blob.height,
            metadata: asset.metadata,
            created_at: asset.created_at.with_timezone(&Utc),
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use chrono::Utc;
    use uuid::Uuid;

    use super::{
        MediaReconciliationDecision, MediaReconciliationReport, classify_reconciliation_probe,
        normalize_original_name, validate_upload_policy, verify_media_type,
    };
    use crate::{
        dto::{DEFAULT_MAX_SIZE, UploadInput},
        error::MediaError,
    };

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
    fn reconciliation_report_helpers_expose_operability_state() {
        let empty = MediaReconciliationReport::default();
        assert!(empty.is_empty());
        assert!(empty.completed_without_retry());
        assert!(!empty.should_retry());
        assert_eq!(empty.changed_records(), 0);

        let report = MediaReconciliationReport {
            inspected: 3,
            healthy: 1,
            missing_marked: 1,
            deletions_completed: 0,
            retry_later: 1,
            ..MediaReconciliationReport::default()
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
    fn reconciliation_marks_missing_objects_without_deleting_evidence() {
        assert_eq!(
            classify_reconciliation_probe(&Err(object_store::Error::NotFound {
                path: "missing".to_string(),
                source: Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "missing",)),
            })),
            MediaReconciliationDecision::MarkMissing
        );
    }

    #[test]
    fn reconciliation_keeps_readable_objects_and_retries_transient_errors() {
        assert_eq!(
            classify_reconciliation_probe(&Ok(object_store::ObjectMeta {
                location: object_store::path::Path::from("media/object"),
                last_modified: Utc::now(),
                size: 6,
                e_tag: None,
                version: None,
            })),
            MediaReconciliationDecision::Healthy
        );
        assert_eq!(
            classify_reconciliation_probe(&Err(object_store::Error::Generic {
                store: "test",
                source: Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout",)),
            })),
            MediaReconciliationDecision::RetryLater
        );
    }
}
