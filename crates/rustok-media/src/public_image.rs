use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use object_store::{ObjectStoreExt, path::Path};
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_storage::StorageRuntime;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    dto::{MediaImageDescriptor, MediaImagePublicUrlPolicy, MediaItem},
    entities::{asset, blob},
    lifecycle::{AssetState, BlobState},
};

pub const MEDIA_PUBLIC_IMAGE_PATH_PREFIX: &str = "/api/media/public/images";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPublicImageAsset {
    pub asset: MediaItem,
    pub descriptor: Option<MediaImageDescriptor>,
}

#[derive(Debug, Clone)]
pub struct MediaPublicImageBody {
    pub bytes: Bytes,
    pub mime_type: String,
    pub checksum_sha256: String,
}

impl MediaPublicImageBody {
    pub fn etag(&self) -> String {
        format!("\"sha256-{}\"", self.checksum_sha256)
    }
}

#[async_trait]
pub trait MediaPublicImageReadPort: Send + Sync {
    async fn get_public_image_asset(
        &self,
        context: PortContext,
        media_id: Uuid,
        alt: Option<String>,
    ) -> Result<MediaPublicImageAsset, PortError>;
}

#[derive(Clone)]
pub struct MediaPublicImageService {
    db: DatabaseConnection,
    storage: StorageRuntime,
}

impl MediaPublicImageService {
    pub fn new(db: DatabaseConnection, storage: StorageRuntime) -> Self {
        Self { db, storage }
    }

    pub async fn read_public_image(
        &self,
        tenant_id: Uuid,
        media_id: Uuid,
        checksum_sha256: &str,
    ) -> Result<MediaPublicImageBody, PortError> {
        let checksum_sha256 = normalize_checksum(checksum_sha256)
            .ok_or_else(public_image_not_found)?;
        let (_, blob) = self.active_asset_blob(tenant_id, media_id).await?;
        if !blob.mime_type.starts_with("image/")
            || !blob.checksum_sha256.eq_ignore_ascii_case(&checksum_sha256)
        {
            return Err(public_image_not_found());
        }

        let result = self
            .storage
            .objects
            .get(&Path::from(blob.object_key.as_str()))
            .await
            .map_err(map_storage_error)?;
        if i64::try_from(result.meta.size).ok() != Some(blob.size) {
            tracing::error!(
                tenant_id = %tenant_id,
                media_id = %media_id,
                expected_size = blob.size,
                actual_size = result.meta.size,
                "Media public image object size does not match owner metadata"
            );
            return Err(PortError::invariant_violation(
                "media.public_image_size_mismatch",
                "media public image requires review",
            ));
        }
        let bytes = result.bytes().await.map_err(map_storage_error)?;
        if i64::try_from(bytes.len()).ok() != Some(blob.size) {
            tracing::error!(
                tenant_id = %tenant_id,
                media_id = %media_id,
                expected_size = blob.size,
                actual_size = bytes.len(),
                "Media public image body size does not match owner metadata"
            );
            return Err(PortError::invariant_violation(
                "media.public_image_body_size_mismatch",
                "media public image requires review",
            ));
        }

        Ok(MediaPublicImageBody {
            bytes,
            mime_type: blob.mime_type,
            checksum_sha256,
        })
    }

    async fn active_asset_blob(
        &self,
        tenant_id: Uuid,
        media_id: Uuid,
    ) -> Result<(asset::Model, blob::Model), PortError> {
        let (asset, active_blob) = asset::Entity::find_by_id(media_id)
            .filter(asset::Column::TenantId.eq(tenant_id))
            .filter(asset::Column::LifecycleState.eq(AssetState::Active.as_str()))
            .find_also_related(blob::Entity)
            .one(&self.db)
            .await
            .map_err(map_database_error)?
            .ok_or_else(public_image_not_found)?;
        let active_blob = active_blob.ok_or_else(public_image_not_found)?;
        if active_blob.state != BlobState::Ready.as_str() {
            return Err(public_image_not_found());
        }
        Ok((asset, active_blob))
    }

    fn media_item(&self, asset: asset::Model, blob: blob::Model) -> MediaItem {
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

#[async_trait]
impl MediaPublicImageReadPort for MediaPublicImageService {
    async fn get_public_image_asset(
        &self,
        context: PortContext,
        media_id: Uuid,
        alt: Option<String>,
    ) -> Result<MediaPublicImageAsset, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
            PortError::validation(
                "media.tenant_id_invalid",
                "media ports require a valid tenant identifier",
            )
        })?;
        let (asset, blob) = self.active_asset_blob(tenant_id, media_id).await?;
        let checksum_sha256 = blob.checksum_sha256.clone();
        let asset = self.media_item(asset, blob);
        let descriptor = MediaImageDescriptor::from_media_item(&asset, alt).and_then(|descriptor| {
            match descriptor.public_url_policy() {
                MediaImagePublicUrlPolicy::DirectPublic => Some(descriptor),
                MediaImagePublicUrlPolicy::ProxyRequired => MediaImageDescriptor::from_parts(
                    public_image_path(media_id, &checksum_sha256),
                    descriptor.alt,
                    descriptor.width,
                    descriptor.height,
                    descriptor.mime_type,
                ),
                MediaImagePublicUrlPolicy::NotAddressable => None,
            }
        });

        Ok(MediaPublicImageAsset { asset, descriptor })
    }
}

pub fn public_image_path(media_id: Uuid, checksum_sha256: &str) -> String {
    format!(
        "{MEDIA_PUBLIC_IMAGE_PATH_PREFIX}/{media_id}/{}",
        checksum_sha256.to_ascii_lowercase()
    )
}

fn normalize_checksum(value: &str) -> Option<String> {
    let value = value.trim();
    (value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_ascii_lowercase())
}

fn public_image_not_found() -> PortError {
    PortError::not_found(
        "media.public_image_not_found",
        "media public image was not found",
    )
}

fn map_database_error(error: sea_orm::DbErr) -> PortError {
    tracing::warn!(error = %error, "Media public image database read failed");
    PortError::new(
        PortErrorKind::Unavailable,
        "media.public_image_database_unavailable",
        "media public image is temporarily unavailable",
        true,
    )
}

fn map_storage_error(error: object_store::Error) -> PortError {
    if matches!(error, object_store::Error::NotFound { .. }) {
        return public_image_not_found();
    }
    tracing::warn!(error = %error, "Media public image storage read failed");
    PortError::new(
        PortErrorKind::Unavailable,
        "media.public_image_storage_unavailable",
        "media public image is temporarily unavailable",
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::{normalize_checksum, public_image_path};
    use uuid::Uuid;

    #[test]
    fn capability_path_uses_media_id_and_normalized_checksum() {
        let media_id = Uuid::nil();
        let checksum = "A".repeat(64);
        assert_eq!(
            public_image_path(media_id, &checksum),
            format!("/api/media/public/images/{media_id}/{}", "a".repeat(64))
        );
        assert_eq!(normalize_checksum(&checksum), Some("a".repeat(64)));
        assert_eq!(normalize_checksum("not-a-checksum"), None);
    }
}
