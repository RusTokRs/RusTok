//! Pricing-owned public presentation. Error payloads and rich metadata are never public copy.
use rustok_api::{PortError, PortErrorKind};
use rustok_commerce_foundation::CommerceError;
use uuid::Uuid;

/// Safe public fields, with a fresh reference unrelated to resource IDs or rich trace IDs.
#[derive(Debug)]
pub struct PricingPublicError {
    pub message: &'static str,
    pub code: &'static str,
    pub retryable: bool,
    pub correlation_id: Uuid,
}

fn public_error(kind: &PortErrorKind, owner_retryable: bool) -> PricingPublicError {
    let (code, message) = match kind {
        PortErrorKind::Validation => ("PRICING_REQUEST_INVALID", "Pricing request is invalid"),
        PortErrorKind::NotFound => (
            "PRICING_RESOURCE_NOT_FOUND",
            "Pricing resource was not found",
        ),
        PortErrorKind::Conflict => (
            "PRICING_STATE_CONFLICT",
            "Pricing operation conflicts with the current state",
        ),
        PortErrorKind::Forbidden => (
            "PRICING_ACCESS_DENIED",
            "Pricing operation is not permitted",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            "PRICING_TEMPORARILY_UNAVAILABLE",
            "Pricing is temporarily unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            "PRICING_OPERATION_FAILED",
            "Pricing operation could not be completed safely",
        ),
    };
    PricingPublicError {
        message,
        code,
        // Never turn an owner-declared retryable failure into a definitive rejection.
        retryable: owner_retryable
            || matches!(kind, PortErrorKind::Unavailable | PortErrorKind::Timeout),
        correlation_id: Uuid::new_v4(),
    }
}

/// Pure port presentation; the boundary logs only bounded facts using this reference.
/// Arbitrary port codes/messages are not trusted as public content.
pub fn map_pricing_port_error(error: &PortError) -> PricingPublicError {
    public_error(&error.kind, error.retryable)
}

/// Classify through the canonical Pricing owner policy and record bounded service facts.
pub fn map_pricing_service_error(
    error: &CommerceError,
    operation: &'static str,
    boundary: &'static str,
) -> PricingPublicError {
    let policy = crate::ports::pricing_error_policy(error);
    let public = public_error(&policy.kind, policy.retryable);
    crate::ports::log_pricing_service_failure(error, operation, boundary, &public);
    public
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_core::error::{Error as CoreError, ErrorKind, RichError};

    const SECRET: &str = "private-sql-sku-token@example.invalid";

    fn assert_policy(error: CommerceError, kind: PortErrorKind, code: &str, retryable: bool) {
        let policy = crate::ports::pricing_error_policy(&error);
        assert_eq!(policy.kind, kind);
        assert_eq!(policy.code, code);
        assert_eq!(policy.retryable, retryable);
        let public = map_pricing_service_error(&error, "scenario", "pricing_scenario");
        assert_eq!(public.retryable, retryable);
        assert!(!format!("{public:?}").contains(SECRET));
        assert!(!public.correlation_id.is_nil());
    }

    #[test]
    fn legacy_domain_outcomes_keep_owner_classification_and_codes() {
        let cases = [
            (
                CommerceError::Database(sea_orm::DbErr::Custom(SECRET.into())),
                PortErrorKind::Unavailable,
                "pricing.database_unavailable",
                true,
            ),
            (
                CommerceError::ProductNotFound(Uuid::nil()),
                PortErrorKind::NotFound,
                "pricing.product_not_found",
                false,
            ),
            (
                CommerceError::VariantNotFound(Uuid::nil()),
                PortErrorKind::NotFound,
                "pricing.variant_not_found",
                false,
            ),
            (
                CommerceError::DuplicateHandle {
                    handle: SECRET.into(),
                    locale: SECRET.into(),
                },
                PortErrorKind::Conflict,
                "pricing.duplicate_handle",
                false,
            ),
            (
                CommerceError::DuplicateSku(SECRET.into()),
                PortErrorKind::Conflict,
                "pricing.duplicate_sku",
                false,
            ),
            (
                CommerceError::InvalidPrice(SECRET.into()),
                PortErrorKind::Validation,
                "pricing.validation",
                false,
            ),
            (
                CommerceError::Validation(SECRET.into()),
                PortErrorKind::Validation,
                "pricing.validation",
                false,
            ),
            (
                CommerceError::InsufficientInventory {
                    requested: 999,
                    available: -1,
                },
                PortErrorKind::Conflict,
                "pricing.insufficient_inventory",
                false,
            ),
            (
                CommerceError::InvalidOptionCombination,
                PortErrorKind::Validation,
                "pricing.invalid_option_combination",
                false,
            ),
            (
                CommerceError::ShippingProfileNotFound(Uuid::nil()),
                PortErrorKind::NotFound,
                "pricing.shipping_profile_not_found",
                false,
            ),
            (
                CommerceError::DuplicateShippingProfileSlug(SECRET.into()),
                PortErrorKind::Conflict,
                "pricing.duplicate_shipping_profile_slug",
                false,
            ),
            (
                CommerceError::NoVariants,
                PortErrorKind::Validation,
                "pricing.no_variants",
                false,
            ),
            (
                CommerceError::CannotDeletePublished,
                PortErrorKind::Conflict,
                "pricing.cannot_delete_published",
                false,
            ),
        ];
        for (error, kind, code, retryable) in cases {
            assert_policy(error, kind, code, retryable);
        }
    }

    #[test]
    fn nested_core_errors_preserve_all_eleven_classifications() {
        let cases = [
            (
                CoreError::Database(sea_orm::DbErr::Custom(SECRET.into())),
                PortErrorKind::Unavailable,
                "pricing.database_unavailable",
                true,
            ),
            (
                CoreError::Cache(SECRET.into()),
                PortErrorKind::Unavailable,
                "pricing.dependency_unavailable",
                true,
            ),
            (
                CoreError::External(SECRET.into()),
                PortErrorKind::Unavailable,
                "pricing.dependency_unavailable",
                true,
            ),
            (
                CoreError::Auth(SECRET.into()),
                PortErrorKind::Forbidden,
                "pricing.access_denied",
                false,
            ),
            (
                CoreError::Forbidden(SECRET.into()),
                PortErrorKind::Forbidden,
                "pricing.access_denied",
                false,
            ),
            (
                CoreError::InvalidIdFormat(SECRET.into()),
                PortErrorKind::Validation,
                "pricing.validation",
                false,
            ),
            (
                CoreError::Validation(SECRET.into()),
                PortErrorKind::Validation,
                "pricing.validation",
                false,
            ),
            (
                CoreError::NotFound(SECRET.into()),
                PortErrorKind::NotFound,
                "pricing.resource_not_found",
                false,
            ),
            (
                CoreError::Serialization(
                    serde_json::from_str::<serde_json::Value>("{").unwrap_err(),
                ),
                PortErrorKind::InvariantViolation,
                "pricing.core_error",
                false,
            ),
            (
                CoreError::Scripting(SECRET.into()),
                PortErrorKind::InvariantViolation,
                "pricing.core_error",
                false,
            ),
            (
                CoreError::Internal(SECRET.into()),
                PortErrorKind::InvariantViolation,
                "pricing.core_error",
                false,
            ),
        ];
        for (error, kind, code, retryable) in cases {
            assert_policy(CommerceError::Core(error), kind, code, retryable);
        }
    }

    #[test]
    fn rich_kind_is_authority_not_status_or_untrusted_metadata() {
        let cases = [
            (
                ErrorKind::Validation,
                PortErrorKind::Validation,
                "pricing.validation",
                false,
            ),
            (
                ErrorKind::BusinessLogic,
                PortErrorKind::Validation,
                "pricing.validation",
                false,
            ),
            (
                ErrorKind::Unauthenticated,
                PortErrorKind::Forbidden,
                "pricing.access_denied",
                false,
            ),
            (
                ErrorKind::Forbidden,
                PortErrorKind::Forbidden,
                "pricing.access_denied",
                false,
            ),
            (
                ErrorKind::NotFound,
                PortErrorKind::NotFound,
                "pricing.resource_not_found",
                false,
            ),
            (
                ErrorKind::Conflict,
                PortErrorKind::Conflict,
                "pricing.state_conflict",
                false,
            ),
            (
                ErrorKind::RateLimited,
                PortErrorKind::Unavailable,
                "pricing.rate_limited",
                true,
            ),
            (
                ErrorKind::Database,
                PortErrorKind::Unavailable,
                "pricing.database_unavailable",
                true,
            ),
            (
                ErrorKind::ExternalService,
                PortErrorKind::Unavailable,
                "pricing.dependency_unavailable",
                true,
            ),
            (
                ErrorKind::Timeout,
                PortErrorKind::Timeout,
                "pricing.timeout",
                true,
            ),
            (
                ErrorKind::Internal,
                PortErrorKind::InvariantViolation,
                "pricing.rich_error",
                false,
            ),
        ];
        for (kind, port_kind, code, retryable) in cases {
            let mut rich = RichError::new(kind, SECRET)
                .with_user_message(SECRET)
                .with_error_code(SECRET)
                .with_field("secret", SECRET)
                .with_source(std::io::Error::other(SECRET));
            rich.status_code = 200;
            rich.trace_id = Some(Uuid::nil());
            assert_policy(CommerceError::from(rich), port_kind, code, retryable);
        }
    }

    #[test]
    fn arbitrary_port_payload_is_private_and_owner_retryability_is_not_lost() {
        let cases = [
            (
                PortErrorKind::Validation,
                "PRICING_REQUEST_INVALID",
                "Pricing request is invalid",
                false,
            ),
            (
                PortErrorKind::NotFound,
                "PRICING_RESOURCE_NOT_FOUND",
                "Pricing resource was not found",
                false,
            ),
            (
                PortErrorKind::Conflict,
                "PRICING_STATE_CONFLICT",
                "Pricing operation conflicts with the current state",
                false,
            ),
            (
                PortErrorKind::Forbidden,
                "PRICING_ACCESS_DENIED",
                "Pricing operation is not permitted",
                false,
            ),
            (
                PortErrorKind::Unavailable,
                "PRICING_TEMPORARILY_UNAVAILABLE",
                "Pricing is temporarily unavailable",
                true,
            ),
            (
                PortErrorKind::Timeout,
                "PRICING_TEMPORARILY_UNAVAILABLE",
                "Pricing is temporarily unavailable",
                true,
            ),
            (
                PortErrorKind::InvariantViolation,
                "PRICING_OPERATION_FAILED",
                "Pricing operation could not be completed safely",
                false,
            ),
        ];
        for (kind, code, message, infrastructure) in cases {
            for owner_retryable in [false, true] {
                let error = PortError {
                    kind: kind.clone(),
                    code: SECRET.into(),
                    message: SECRET.into(),
                    retryable: owner_retryable,
                };
                let public = map_pricing_port_error(&error);
                assert_eq!(public.code, code);
                assert_eq!(public.message, message);
                assert_eq!(public.retryable, owner_retryable || infrastructure);
                assert!(!format!("{public:?}").contains(SECRET));
                assert!(!public.correlation_id.is_nil());
            }
        }
    }
}
