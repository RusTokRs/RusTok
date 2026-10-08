use async_graphql::{ErrorExtensions, Result};
use rustok_api::{PortContext, PortError, PortErrorKind};
use uuid::Uuid;

use crate::storefront_shipping::{
    StorefrontShippingSelectionValidationError, validate_storefront_shipping_option_selection,
};

const STOREFRONT_SHIPPING_OPTION_GRAPHQL_BOUNDARY: &str =
    "commerce_graphql_storefront_shipping_option";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShippingOptionFailureKind {
    MultipleDeliveryGroups,
    NoDeliveryGroups,
    OwnerValidation,
    OwnerNotFound,
    OwnerConflict,
    OwnerForbidden,
    StorageUnavailable,
    OwnerInvariant,
    CurrencyMismatch,
    ChannelUnavailable,
    ProfileIncompatible,
    Inactive,
}

#[derive(Debug)]
struct ShippingOptionFailure {
    kind: ShippingOptionFailureKind,
    source_owner: &'static str,
    source_operation: &'static str,
    internal_code: String,
    internal_kind: &'static str,
    internal_retryable: bool,
    shipping_option_id: Option<Uuid>,
    profile_slug_length: Option<usize>,
    option_currency_code_length: Option<usize>,
    owner_error: Option<PortError>,
    message: Option<String>,
}

impl ShippingOptionFailure {
    fn multiple_delivery_groups(shipping_option_id: Uuid) -> Self {
        Self {
            message: Some(
                "selectedShippingOptionId can only be used for carts with a single delivery group"
                    .to_string(),
            ),
            ..Self::local(
                ShippingOptionFailureKind::MultipleDeliveryGroups,
                "validate_single_delivery_group",
                "shipping_selection.multiple_delivery_groups",
                "validation",
                Some(shipping_option_id),
            )
        }
    }

    fn no_delivery_groups(shipping_option_id: Option<Uuid>) -> Self {
        Self {
            message: Some(
                "shipping selection is not valid for a cart without delivery groups".to_string(),
            ),
            ..Self::local(
                ShippingOptionFailureKind::NoDeliveryGroups,
                "validate_delivery_groups",
                "shipping_selection.no_delivery_groups",
                "validation",
                shipping_option_id,
            )
        }
    }

    fn owner(shipping_option_id: Uuid, error: PortError) -> Self {
        let (kind, internal_kind) = match &error.kind {
            PortErrorKind::Validation => (ShippingOptionFailureKind::OwnerValidation, "validation"),
            PortErrorKind::NotFound => (ShippingOptionFailureKind::OwnerNotFound, "not_found"),
            PortErrorKind::Conflict => (ShippingOptionFailureKind::OwnerConflict, "conflict"),
            PortErrorKind::Forbidden => (ShippingOptionFailureKind::OwnerForbidden, "forbidden"),
            PortErrorKind::Unavailable | PortErrorKind::Timeout => {
                (ShippingOptionFailureKind::StorageUnavailable, "unavailable")
            }
            PortErrorKind::InvariantViolation => {
                (ShippingOptionFailureKind::OwnerInvariant, "invariant")
            }
        };

        Self {
            kind,
            source_owner: "rustok_fulfillment",
            source_operation: "read_shipping_option_projection",
            internal_code: error.code.clone(),
            internal_kind,
            internal_retryable: error.retryable,
            shipping_option_id: Some(shipping_option_id),
            profile_slug_length: None,
            option_currency_code_length: None,
            owner_error: Some(error),
            message: None,
        }
    }

    fn currency_mismatch(
        shipping_option_id: Uuid,
        option_currency: &str,
        expected_currency: &str,
    ) -> Self {
        Self {
            option_currency_code_length: Some(option_currency.chars().count()),
            message: Some(format!(
                "Shipping option {shipping_option_id} uses currency {option_currency}, expected {expected_currency}"
            )),
            ..Self::local(
                ShippingOptionFailureKind::CurrencyMismatch,
                "validate_currency",
                "shipping_selection.currency_mismatch",
                "validation",
                Some(shipping_option_id),
            )
        }
    }

    fn channel_unavailable(shipping_option_id: Uuid) -> Self {
        Self {
            message: Some(format!(
                "Shipping option {shipping_option_id} is not available for the current channel"
            )),
            ..Self::local(
                ShippingOptionFailureKind::ChannelUnavailable,
                "validate_channel_visibility",
                "shipping_selection.channel_unavailable",
                "validation",
                Some(shipping_option_id),
            )
        }
    }

    fn profile_incompatible(shipping_option_id: Uuid, profile_slug: &str) -> Self {
        Self {
            profile_slug_length: Some(profile_slug.chars().count()),
            message: Some(format!(
                "Shipping option {shipping_option_id} is not compatible with shipping profile {profile_slug}"
            )),
            ..Self::local(
                ShippingOptionFailureKind::ProfileIncompatible,
                "validate_shipping_profile",
                "shipping_selection.profile_incompatible",
                "validation",
                Some(shipping_option_id),
            )
        }
    }

    fn inactive(shipping_option_id: Uuid) -> Self {
        Self {
            message: Some("Shipping option is not active".to_string()),
            ..Self::local(
                ShippingOptionFailureKind::Inactive,
                "validate_active_state",
                "shipping_selection.inactive",
                "conflict",
                Some(shipping_option_id),
            )
        }
    }

    fn from_selection_validation_error(error: StorefrontShippingSelectionValidationError) -> Self {
        match error {
            StorefrontShippingSelectionValidationError::MissingDeliveryGroup {
                shipping_option_id,
                shipping_profile_slug,
            } => Self::profile_incompatible(shipping_option_id, shipping_profile_slug.as_str()),
            StorefrontShippingSelectionValidationError::Owner {
                shipping_option_id,
                error,
            } => Self::owner(shipping_option_id, error),
            StorefrontShippingSelectionValidationError::Inactive { shipping_option_id } => {
                Self::inactive(shipping_option_id)
            }
            StorefrontShippingSelectionValidationError::CurrencyMismatch {
                shipping_option_id,
                option_currency_code,
                expected_currency_code,
            } => Self::currency_mismatch(
                shipping_option_id,
                option_currency_code.as_str(),
                expected_currency_code.as_str(),
            ),
            StorefrontShippingSelectionValidationError::ChannelUnavailable {
                shipping_option_id,
            } => Self::channel_unavailable(shipping_option_id),
            StorefrontShippingSelectionValidationError::ProfileIncompatible {
                shipping_option_id,
                shipping_profile_slug,
            } => Self::profile_incompatible(shipping_option_id, shipping_profile_slug.as_str()),
        }
    }

    fn local(
        kind: ShippingOptionFailureKind,
        source_operation: &'static str,
        internal_code: &'static str,
        internal_kind: &'static str,
        shipping_option_id: Option<Uuid>,
    ) -> Self {
        Self {
            kind,
            source_owner: "rustok_commerce.graphql_shipping_selection",
            source_operation,
            internal_code: internal_code.to_string(),
            internal_kind,
            internal_retryable: false,
            shipping_option_id,
            profile_slug_length: None,
            option_currency_code_length: None,
            owner_error: None,
            message: None,
        }
    }

    fn technical_owner_error(&self) -> Option<&PortError> {
        if matches!(
            self.kind,
            ShippingOptionFailureKind::StorageUnavailable
                | ShippingOptionFailureKind::OwnerInvariant
        ) {
            self.owner_error.as_ref()
        } else {
            None
        }
    }
}

fn public_graphql_error(message: impl Into<String>) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", "SHIPPING_OPTION_INVALID");
        extensions.set("retryable", false);
    })
}

#[allow(clippy::too_many_arguments)]
fn shipping_option_graphql_error(
    failure: ShippingOptionFailure,
    context: &PortContext,
    cart_id: Uuid,
    selection_count: usize,
    delivery_group_count: usize,
    requested_currency_code_length: usize,
    public_channel_slug: Option<&str>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> async_graphql::Error {
    let channel_slug_length = public_channel_slug.map(str::chars).map(Iterator::count);
    let requested_locale_length = requested_locale.map(str::chars).map(Iterator::count);
    let tenant_default_locale_length = tenant_default_locale.map(str::chars).map(Iterator::count);
    let technical_owner_error = failure.technical_owner_error();

    if matches!(
        failure.kind,
        ShippingOptionFailureKind::StorageUnavailable | ShippingOptionFailureKind::OwnerInvariant
    ) {
        tracing::error!(
            error = ?technical_owner_error,
            owner = failure.source_owner,
            owner_operation = failure.source_operation,
            internal_code = %failure.internal_code,
            internal_kind = failure.internal_kind,
            internal_retryable = failure.internal_retryable,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            context_channel_length = context.channel.as_deref().map(str::len),
            context_locale_length = context.locale.len(),
            deadline_ms = ?context.deadline_ms,
            cart_id = %cart_id,
            shipping_option_id = ?failure.shipping_option_id,
            selection_count,
            delivery_group_count,
            requested_currency_code_length,
            option_currency_code_length = ?failure.option_currency_code_length,
            profile_slug_length = ?failure.profile_slug_length,
            channel_slug_length = ?channel_slug_length,
            requested_locale_length = ?requested_locale_length,
            tenant_default_locale_length = ?tenant_default_locale_length,
            public_code = "SHIPPING_OPTION_INVALID",
            public_retryable = false,
            boundary = STOREFRONT_SHIPPING_OPTION_GRAPHQL_BOUNDARY,
            "commerce GraphQL storefront shipping option dependency failed"
        );
    } else {
        tracing::warn!(
            owner = failure.source_owner,
            owner_operation = failure.source_operation,
            internal_code = %failure.internal_code,
            internal_kind = failure.internal_kind,
            internal_retryable = failure.internal_retryable,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            context_channel_length = context.channel.as_deref().map(str::len),
            context_locale_length = context.locale.len(),
            deadline_ms = ?context.deadline_ms,
            cart_id = %cart_id,
            shipping_option_id = ?failure.shipping_option_id,
            selection_count,
            delivery_group_count,
            requested_currency_code_length,
            option_currency_code_length = ?failure.option_currency_code_length,
            profile_slug_length = ?failure.profile_slug_length,
            channel_slug_length = ?channel_slug_length,
            requested_locale_length = ?requested_locale_length,
            tenant_default_locale_length = ?tenant_default_locale_length,
            public_code = "SHIPPING_OPTION_INVALID",
            public_retryable = false,
            boundary = STOREFRONT_SHIPPING_OPTION_GRAPHQL_BOUNDARY,
            "commerce GraphQL storefront shipping option request was rejected"
        );
    }

    let message = failure
        .message
        .unwrap_or_else(|| "Selected shipping option is invalid".to_string());
    public_graphql_error(message)
}

#[cfg(test)]
mod tests {
    use super::{ShippingOptionFailure, shipping_option_graphql_error};
    use rustok_api::{PortActor, PortContext, PortError};
    use uuid::Uuid;

    #[test]
    fn owner_error_message_never_reaches_graphql_response() {
        let cart_id = Uuid::new_v4();
        let context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::service("rustok-commerce.graphql-test"),
            "en",
            format!("test:{cart_id}"),
        )
        .with_deadline(std::time::Duration::from_secs(2));
        let failure = ShippingOptionFailure::owner(
            Uuid::new_v4(),
            PortError::validation(
                "fulfillment.internal_validation",
                "internal owner detail must stay diagnostic-only",
            ),
        );

        let error = shipping_option_graphql_error(
            failure,
            &context,
            cart_id,
            1,
            1,
            3,
            None,
            Some("en"),
            Some("en"),
        );

        assert_eq!(error.message, "Selected shipping option is invalid");
        assert_ne!(
            error.message,
            "internal owner detail must stay diagnostic-only"
        );
    }
    #[test]
    fn inactive_shipping_option_uses_stable_public_message() {
        let failure = ShippingOptionFailure::inactive(Uuid::new_v4());
        let context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::service("rustok-commerce.graphql-test"),
            "en",
            "test:inactive-shipping-option",
        )
        .with_deadline(std::time::Duration::from_secs(2));

        let error = shipping_option_graphql_error(
            failure,
            &context,
            Uuid::new_v4(),
            1,
            1,
            3,
            None,
            Some("en"),
            Some("en"),
        );

        assert_eq!(error.message, "Shipping option is not active");
    }
}

fn current_shipping_selections(
    cart: &crate::dto::CartResponse,
) -> Vec<crate::dto::CartShippingSelectionInput> {
    cart.delivery_groups
        .iter()
        .map(|group| crate::dto::CartShippingSelectionInput {
            shipping_profile_slug: group.shipping_profile_slug.clone(),
            seller_id: group.seller_id.clone(),
            seller_scope: None,
            selected_shipping_option_id: group.selected_shipping_option_id,
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn validate_selected_shipping_option(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    cart: &crate::dto::CartResponse,
    selected_shipping_option_id: Option<Uuid>,
    shipping_selections: Option<&[crate::dto::CartShippingSelectionInput]>,
    currency_code: &str,
    public_channel_slug: Option<&str>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> Result<()> {
    let owner_locale = requested_locale.or(tenant_default_locale).unwrap_or("en");
    let owner_context =
        super::shipping_option_read_context::storefront_shipping_option_read_context(
            tenant_id,
            cart.id,
            owner_locale,
            public_channel_slug,
            "read-option",
        );
    let shipping_option_read_port =
        super::shipping_option_read_context::storefront_shipping_option_read_port(db.clone());
    let requested_currency_code_length = currency_code.chars().count();
    let requested_selection_count = shipping_selections
        .map(|selections| selections.len())
        .unwrap_or_else(|| {
            if selected_shipping_option_id.is_some() {
                1
            } else {
                0
            }
        });

    if shipping_selections.is_none()
        && let Some(shipping_option_id) = selected_shipping_option_id
        && cart.delivery_groups.len() > 1
    {
        return Err(shipping_option_graphql_error(
            ShippingOptionFailure::multiple_delivery_groups(shipping_option_id),
            &owner_context,
            cart.id,
            requested_selection_count,
            cart.delivery_groups.len(),
            requested_currency_code_length,
            public_channel_slug,
            requested_locale,
            tenant_default_locale,
        ));
    }

    let selections = if let Some(shipping_selections) = shipping_selections {
        if cart.delivery_groups.is_empty() && !shipping_selections.is_empty() {
            return Err(shipping_option_graphql_error(
                ShippingOptionFailure::no_delivery_groups(None),
                &owner_context,
                cart.id,
                shipping_selections.len(),
                0,
                requested_currency_code_length,
                public_channel_slug,
                requested_locale,
                tenant_default_locale,
            ));
        }
        shipping_selections.to_vec()
    } else if let Some(selected_shipping_option_id) = selected_shipping_option_id {
        if cart.delivery_groups.is_empty() {
            return Err(shipping_option_graphql_error(
                ShippingOptionFailure::no_delivery_groups(Some(selected_shipping_option_id)),
                &owner_context,
                cart.id,
                requested_selection_count,
                0,
                requested_currency_code_length,
                public_channel_slug,
                requested_locale,
                tenant_default_locale,
            ));
        }
        cart.delivery_groups
            .first()
            .map(|group| {
                vec![crate::dto::CartShippingSelectionInput {
                    shipping_profile_slug: group.shipping_profile_slug.clone(),
                    seller_id: group.seller_id.clone(),
                    seller_scope: None,
                    selected_shipping_option_id: Some(selected_shipping_option_id),
                }]
            })
            .unwrap_or_default()
    } else {
        current_shipping_selections(cart)
    };
    let selection_count = selections.len();

    for selection in selections {
        let Some(_shipping_option_id) = selection.selected_shipping_option_id else {
            continue;
        };

        validate_storefront_shipping_option_selection(
            cart,
            &selection,
            currency_code,
            public_channel_slug,
            requested_locale,
            tenant_default_locale,
            &owner_context,
            shipping_option_read_port.as_ref(),
        )
        .await
        .map_err(|error| {
            shipping_option_graphql_error(
                ShippingOptionFailure::from_selection_validation_error(error),
                &owner_context,
                cart.id,
                selection_count,
                cart.delivery_groups.len(),
                requested_currency_code_length,
                public_channel_slug,
                requested_locale,
                tenant_default_locale,
            )
        })?;
    }

    Ok(())
}
