use std::fmt::{Display, Formatter};

use rustok_api::RequestContext;
use uuid::Uuid;

const MAX_TRUSTED_STOREFRONT_CHANNEL_SLUG_LEN: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedStorefrontChannel {
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorefrontChannelAuthorityError {
    InvalidRequestedChannelId,
    RequestedChannelMismatch,
    RequestTenantMismatch,
    IncompleteTrustedChannelContext,
    InvalidTrustedChannelContext,
}

impl Display for StorefrontChannelAuthorityError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRequestedChannelId => "channel_id contains an invalid UUID",
            Self::RequestedChannelMismatch => {
                "channel_id does not match the trusted request channel"
            }
            Self::RequestTenantMismatch => {
                "trusted request channel tenant does not match the Search tenant"
            }
            Self::IncompleteTrustedChannelContext => {
                "trusted storefront channel context is incomplete"
            }
            Self::InvalidTrustedChannelContext => "trusted storefront channel context is invalid",
        })
    }
}

impl std::error::Error for StorefrontChannelAuthorityError {}

pub fn resolve_trusted_storefront_channel_input(
    request_context: &RequestContext,
    expected_tenant_id: Uuid,
    requested_channel_id: Option<&str>,
) -> Result<TrustedStorefrontChannel, StorefrontChannelAuthorityError> {
    let requested_channel_id = requested_channel_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            Uuid::parse_str(value)
                .map_err(|_| StorefrontChannelAuthorityError::InvalidRequestedChannelId)
        })
        .transpose()?;

    resolve_trusted_storefront_channel(request_context, expected_tenant_id, requested_channel_id)
}

pub fn resolve_trusted_storefront_channel(
    request_context: &RequestContext,
    expected_tenant_id: Uuid,
    requested_channel_id: Option<Uuid>,
) -> Result<TrustedStorefrontChannel, StorefrontChannelAuthorityError> {
    if request_context.tenant_id != expected_tenant_id {
        return Err(StorefrontChannelAuthorityError::RequestTenantMismatch);
    }

    let trusted = match (
        request_context.channel_id,
        request_context.channel_slug.as_deref(),
    ) {
        (None, None) => TrustedStorefrontChannel {
            channel_id: None,
            channel_slug: None,
        },
        (Some(channel_id), Some(channel_slug)) => {
            let channel_slug = channel_slug.trim();
            if channel_id.is_nil()
                || channel_slug.is_empty()
                || channel_slug.len() > MAX_TRUSTED_STOREFRONT_CHANNEL_SLUG_LEN
                || channel_slug.chars().any(char::is_control)
            {
                return Err(StorefrontChannelAuthorityError::InvalidTrustedChannelContext);
            }
            TrustedStorefrontChannel {
                channel_id: Some(channel_id),
                channel_slug: Some(channel_slug.to_string()),
            }
        }
        _ => {
            return Err(StorefrontChannelAuthorityError::IncompleteTrustedChannelContext);
        }
    };

    if requested_channel_id.is_some() && requested_channel_id != trusted.channel_id {
        return Err(StorefrontChannelAuthorityError::RequestedChannelMismatch);
    }

    Ok(trusted)
}

#[cfg(test)]
mod tests {
    use rustok_api::RequestContext;
    use uuid::Uuid;

    use super::{
        StorefrontChannelAuthorityError, resolve_trusted_storefront_channel,
        resolve_trusted_storefront_channel_input,
    };

    fn request_context(
        tenant_id: Uuid,
        channel_id: Option<Uuid>,
        channel_slug: Option<&str>,
    ) -> RequestContext {
        RequestContext {
            tenant_id,
            user_id: None,
            channel_id,
            channel_slug: channel_slug.map(ToOwned::to_owned),
            channel_resolution_source: None,
            locale: "en".to_string(),
        }
    }

    #[test]
    fn absent_channel_remains_unscoped() {
        let tenant_id = Uuid::new_v4();
        let resolved = resolve_trusted_storefront_channel(
            &request_context(tenant_id, None, None),
            tenant_id,
            None,
        )
        .expect("absent trusted channel should remain unscoped");

        assert_eq!(resolved.channel_id, None);
        assert_eq!(resolved.channel_slug, None);
    }

    #[test]
    fn matching_assertion_preserves_trusted_channel() {
        let tenant_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let resolved = resolve_trusted_storefront_channel(
            &request_context(tenant_id, Some(channel_id), Some("web")),
            tenant_id,
            Some(channel_id),
        )
        .expect("matching caller assertion should be accepted");

        assert_eq!(resolved.channel_id, Some(channel_id));
        assert_eq!(resolved.channel_slug.as_deref(), Some("web"));
    }

    #[test]
    fn mismatched_assertion_cannot_select_another_channel() {
        let tenant_id = Uuid::new_v4();
        let error = resolve_trusted_storefront_channel(
            &request_context(tenant_id, Some(Uuid::new_v4()), Some("web")),
            tenant_id,
            Some(Uuid::new_v4()),
        )
        .expect_err("caller channel override must fail closed");

        assert_eq!(error, StorefrontChannelAuthorityError::RequestedChannelMismatch);
    }

    #[test]
    fn mismatched_tenant_fails_closed() {
        let error = resolve_trusted_storefront_channel(
            &request_context(Uuid::new_v4(), None, None),
            Uuid::new_v4(),
            None,
        )
        .expect_err("foreign request context must fail closed");

        assert_eq!(error, StorefrontChannelAuthorityError::RequestTenantMismatch);
    }

    #[test]
    fn incomplete_context_fails_closed() {
        let tenant_id = Uuid::new_v4();
        let error = resolve_trusted_storefront_channel(
            &request_context(tenant_id, Some(Uuid::new_v4()), None),
            tenant_id,
            None,
        )
        .expect_err("trusted channel id without slug must fail closed");

        assert_eq!(
            error,
            StorefrontChannelAuthorityError::IncompleteTrustedChannelContext
        );
    }

    #[test]
    fn malformed_caller_channel_id_is_rejected() {
        let tenant_id = Uuid::new_v4();
        let error = resolve_trusted_storefront_channel_input(
            &request_context(tenant_id, None, None),
            tenant_id,
            Some("not-a-uuid"),
        )
        .expect_err("invalid caller channel assertion must be rejected");

        assert_eq!(
            error,
            StorefrontChannelAuthorityError::InvalidRequestedChannelId
        );
    }
}
