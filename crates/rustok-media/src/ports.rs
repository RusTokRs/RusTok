use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use uuid::Uuid;

use crate::{MediaError, MediaImageDescriptor, MediaItem, MediaService, MediaTranslationItem};

/// Transport-neutral read boundary for media asset metadata and SEO image descriptors.
#[async_trait]
pub trait MediaAssetReadPort: Send + Sync {
    async fn get_asset(&self, context: PortContext, media_id: Uuid)
        -> Result<MediaItem, PortError>;

    async fn list_assets(
        &self,
        context: PortContext,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<MediaItem>, u64), PortError>;

    async fn get_image_descriptor(
        &self,
        context: PortContext,
        media_id: Uuid,
        alt: Option<String>,
    ) -> Result<Option<MediaImageDescriptor>, PortError>;

    async fn get_translations(
        &self,
        context: PortContext,
        media_id: Uuid,
    ) -> Result<Vec<MediaTranslationItem>, PortError>;
}

#[async_trait]
impl MediaAssetReadPort for MediaService {
    async fn get_asset(
        &self,
        context: PortContext,
        media_id: Uuid,
    ) -> Result<MediaItem, PortError> {
        context.require_deadline_semantics()?;
        let tenant_id = parse_tenant_id(&context)?;
        self.get(tenant_id, media_id)
            .await
            .map_err(media_error_to_port_error)
    }

    async fn list_assets(
        &self,
        context: PortContext,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<MediaItem>, u64), PortError> {
        context.require_deadline_semantics()?;
        let tenant_id = parse_tenant_id(&context)?;
        self.list(tenant_id, limit, offset)
            .await
            .map_err(media_error_to_port_error)
    }

    async fn get_image_descriptor(
        &self,
        context: PortContext,
        media_id: Uuid,
        alt: Option<String>,
    ) -> Result<Option<MediaImageDescriptor>, PortError> {
        context.require_deadline_semantics()?;
        let tenant_id = parse_tenant_id(&context)?;
        let item = self
            .get(tenant_id, media_id)
            .await
            .map_err(media_error_to_port_error)?;
        Ok(MediaImageDescriptor::from_media_item(&item, alt))
    }

    async fn get_translations(
        &self,
        context: PortContext,
        media_id: Uuid,
    ) -> Result<Vec<MediaTranslationItem>, PortError> {
        context.require_deadline_semantics()?;
        let tenant_id = parse_tenant_id(&context)?;
        self.get_translations(tenant_id, media_id)
            .await
            .map_err(media_error_to_port_error)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "media.invalid_tenant_id",
            "media port context must carry a UUID tenant_id",
        )
    })
}

fn media_error_to_port_error(error: MediaError) -> PortError {
    match error {
        MediaError::NotFound(id) => PortError::new(
            PortErrorKind::NotFound,
            "media.not_found",
            format!("media asset not found: {id}"),
            false,
        ),
        MediaError::Forbidden => PortError::new(
            PortErrorKind::Forbidden,
            "media.forbidden",
            "media access denied",
            false,
        ),
        MediaError::UnsupportedMimeType(content_type) => PortError::validation(
            "media.unsupported_mime_type",
            format!("unsupported media type: {content_type}"),
        ),
        MediaError::FileTooLarge { size, max } => PortError::validation(
            "media.file_too_large",
            format!("file too large: {size} bytes; max {max} bytes"),
        ),
        MediaError::InvalidLocale(locale) => {
            PortError::validation("media.invalid_locale", format!("invalid locale: {locale}"))
        }
        MediaError::Storage(source) => PortError::unavailable("media.storage", source.to_string()),
        MediaError::Db(source) => PortError::unavailable("media.database", source.to_string()),
    }
}
