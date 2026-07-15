use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use uuid::Uuid;

use crate::{MediaError, MediaImageDescriptor, MediaItem, MediaService, MediaTranslationItem};

const MAX_MEDIA_LIST_LIMIT: u64 = 100;

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
        require_media_read_policy(&context)?;
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
        require_media_read_policy(&context)?;
        validate_media_list_limit(limit)?;
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
        require_media_read_policy(&context)?;
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
        require_media_read_policy(&context)?;
        let tenant_id = parse_tenant_id(&context)?;
        self.get_translations(tenant_id, media_id)
            .await
            .map_err(media_error_to_port_error)
    }
}

fn validate_media_list_limit(limit: u64) -> Result<(), PortError> {
    if !(1..=MAX_MEDIA_LIST_LIMIT).contains(&limit) {
        return Err(PortError::validation(
            "media.list_limit_invalid",
            format!("media list limit must be between 1 and {MAX_MEDIA_LIST_LIMIT}"),
        ));
    }
    Ok(())
}

fn require_media_read_policy(context: &PortContext) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())
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
        MediaError::InvalidMediaContent { declared, reason } => PortError::validation(
            "media.invalid_content",
            format!("media content does not match declared type {declared}: {reason}"),
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::{PortActor, PortContext, PortErrorKind};
    use uuid::Uuid;

    use super::{media_error_to_port_error, parse_tenant_id, require_media_read_policy};
    use crate::MediaError;

    fn context(tenant_id: impl Into<String>) -> PortContext {
        PortContext::new(
            tenant_id,
            PortActor::service("media-port-test"),
            "en",
            "corr-1",
        )
        .with_deadline(Duration::from_secs(1))
    }

    #[test]
    fn require_media_read_policy_requires_deadline_but_not_idempotency() {
        let without_deadline = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::service("media-port-test"),
            "en",
            "corr-1",
        );

        let error = require_media_read_policy(&without_deadline)
            .expect_err("read port calls must carry a deadline");
        assert_eq!(error.kind, PortErrorKind::Timeout);
        assert_eq!(error.code, "port.deadline_required");

        assert!(require_media_read_policy(&context(Uuid::new_v4().to_string())).is_ok());
    }

    #[test]
    fn parse_tenant_id_accepts_uuid_context_values() {
        let tenant_id = Uuid::new_v4();

        assert_eq!(
            parse_tenant_id(&context(tenant_id.to_string())).expect("tenant UUID should parse"),
            tenant_id
        );
    }

    #[test]
    fn parse_tenant_id_rejects_non_uuid_context_values_as_validation_errors() {
        let error = parse_tenant_id(&context("tenant-slug")).expect_err("tenant slug must fail");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "media.invalid_tenant_id");
        assert!(!error.retryable);
    }

    #[test]
    fn media_error_to_port_error_preserves_not_found_and_forbidden_semantics() {
        let id = Uuid::new_v4();
        let not_found = media_error_to_port_error(MediaError::NotFound(id));
        let forbidden = media_error_to_port_error(MediaError::Forbidden);

        assert_eq!(not_found.kind, PortErrorKind::NotFound);
        assert_eq!(not_found.code, "media.not_found");
        assert!(!not_found.retryable);
        assert!(not_found.message.contains(&id.to_string()));

        assert_eq!(forbidden.kind, PortErrorKind::Forbidden);
        assert_eq!(forbidden.code, "media.forbidden");
        assert!(!forbidden.retryable);
    }

    #[test]
    fn media_error_to_port_error_maps_policy_errors_to_non_retryable_validation() {
        let unsupported =
            media_error_to_port_error(MediaError::UnsupportedMimeType("text/html".to_string()));
        let oversized = media_error_to_port_error(MediaError::FileTooLarge { size: 12, max: 10 });
        let invalid_locale =
            media_error_to_port_error(MediaError::InvalidLocale("bad/locale".to_string()));

        for error in [unsupported, oversized, invalid_locale] {
            assert_eq!(error.kind, PortErrorKind::Validation);
            assert!(error.code.starts_with("media."));
            assert!(!error.retryable);
        }
    }

    #[test]
    fn media_error_to_port_error_marks_storage_and_database_failures_retryable() {
        let storage = media_error_to_port_error(MediaError::Storage(
            rustok_storage::StorageError::Backend("timeout".to_string()),
        ));
        let database = media_error_to_port_error(MediaError::Db(sea_orm::DbErr::Conn(
            sea_orm::RuntimeErr::Internal("database unavailable".to_string()),
        )));

        assert_eq!(storage.kind, PortErrorKind::Unavailable);
        assert_eq!(storage.code, "media.storage");
        assert!(storage.retryable);

        assert_eq!(database.kind, PortErrorKind::Unavailable);
        assert_eq!(database.code, "media.database");
        assert!(database.retryable);
    }
}
