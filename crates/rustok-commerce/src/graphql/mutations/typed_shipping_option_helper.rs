use std::collections::BTreeSet;

use async_graphql::{ErrorExtensions, Result};
use rustok_fulfillment::{FulfillmentError, FulfillmentService};
use uuid::Uuid;

use crate::{
    storefront_channel::is_metadata_visible_for_public_channel,
    storefront_shipping::{
        is_shipping_option_compatible_with_profiles, normalize_shipping_profile_slug,
    },
};

const STOREFRONT_SHIPPING_OPTION_GRAPHQL_BOUNDARY: &str =
    "commerce_graphql_storefront_shipping_option";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShippingOptionFailureKind {
    MultipleDeliveryGroups,
    OwnerValidation,
    OwnerNotFound,
    OwnerConflict,
    StorageUnavailable,
    CurrencyMismatch,
    ChannelUnavailable,
    ProfileIncompatible,
}

#[derive(Debug)]
struct ShippingOptionFailure {
    kind: ShippingOptionFailureKind,
    source_owner: &'static str,
    source_operation: &'static str,
    internal_code: &'static str,
    internal_kind: &'static str,
    internal_retryable: bool,
    shipping_option_id: Option<Uuid>,
    profile_slug_length: Option<usize>,
    option_currency_code_length: Option<usize>,
    owner_error: Option<FulfillmentError>,
}

impl ShippingOptionFailure {
    fn multiple_delivery_groups(shipping_option_id: Uuid) -> Self {
        Self::local(
            ShippingOptionFailureKind::MultipleDeliveryGroups,
            "validate_single_delivery_group",
            "shipping_selection.multiple_delivery_groups",
            "validation",
            Some(shipping_option_id),
        )
    }

    fn owner(shipping_option_id: Uuid, error: FulfillmentError) -> Self {
        let (kind, internal_code, internal_kind, internal_retryable) = match &error {
            FulfillmentError::Validation(_) => (
                ShippingOptionFailureKind::OwnerValidation,
                "fulfillment.validation",
                "validation",
                false,
            ),
            FulfillmentError::ShippingOptionNotFound(_) => (
                ShippingOptionFailureKind::OwnerNotFound,
                "fulfillment.shipping_option_not_found",
                "not_found",
                false,
            ),
            FulfillmentError::FulfillmentNotFound(_) => (
                ShippingOptionFailureKind::OwnerNotFound,
                "fulfillment.fulfillment_not_found",
                "not_found",
                false,
            ),
            FulfillmentError::InvalidTransition { .. } => (
                ShippingOptionFailureKind::OwnerConflict,
                "fulfillment.invalid_transition",
                "conflict",
                false,
            ),
            FulfillmentError::Database(_) => (
                ShippingOptionFailureKind::StorageUnavailable,
                "fulfillment.database_unavailable",
                "unavailable",
                true,
            ),
        };

        Self {
            kind,
            source_owner: "rustok_fulfillment",
            source_operation: "get_shipping_option",
            internal_code,
            internal_kind,
            internal_retryable,
            shipping_option_id: Some(shipping_option_id),
            profile_slug_length: None,
            option_currency_code_length: None,
            owner_error: Some(error),
        }
    }

    fn currency_mismatch(shipping_option_id: Uuid, option_currency_code_length: usize) -> Self {
        Self {
            option_currency_code_length: Some(option_currency_code_length),
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
        Self::local(
            ShippingOptionFailureKind::ChannelUnavailable,
            "validate_channel_visibility",
            "shipping_selection.channel_unavailable",
            "validation",
            Some(shipping_option_id),
        )
    }

    fn profile_incompatible(shipping_option_id: Uuid, profile_slug_length: usize) -> Self {
        Self {
            profile_slug_length: Some(profile_slug_length),
            ..Self::local(
                ShippingOptionFailureKind::ProfileIncompatible,
                "validate_shipping_profile",
                "shipping_selection.profile_incompatible",
                "validation",
                Some(shipping_option_id),
            )
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
            internal_code,
            internal_kind,
            internal_retryable: false,
            shipping_option_id,
            profile_slug_length: None,
            option_currency_code_length: None,
            owner_error: None,
        }
    }

    fn technical_owner_error(&self) -> Option<&FulfillmentError> {
        matches!(self.kind, ShippingOptionFailureKind::StorageUnavailable)
            .then(|| self.owner_error.as_ref())
            .flatten()
    }
}

fn public_graphql_error() -> async_graphql::Error {
    async_graphql::Error::new("Selected shipping option is invalid").extend_with(
        |_, extensions| {
            extensions.set("code", "SHIPPING_OPTION_INVALID");
            extensions.set("retryable", false);
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn shipping_option_graphql_error(
    failure: ShippingOptionFailure,
    tenant_id: Uuid,
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

    if matches!(failure.kind, ShippingOptionFailureKind::StorageUnavailable) {
        tracing::error!(
            error = ?technical_owner_error,
            owner = failure.source_owner,
            owner_operation = failure.source_operation,
            internal_code = failure.internal_code,
            internal_kind = failure.internal_kind,
            internal_retryable = failure.internal_retryable,
            tenant_id = %tenant_id,
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
            internal_code = failure.internal_code,
            internal_kind = failure.internal_kind,
            internal_retryable = failure.internal_retryable,
            tenant_id = %tenant_id,
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

    public_graphql_error()
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
    let requested_currency_code_length = currency_code.chars().count();
    let requested_selection_count = shipping_selections
        .map(<[_]>::len)
        .unwrap_or_else(|| usize::from(selected_shipping_option_id.is_some()));

    if shipping_selections.is_none()
        && selected_shipping_option_id.is_some()
        && cart.delivery_groups.len() > 1
    {
        return Err(shipping_option_graphql_error(
            ShippingOptionFailure::multiple_delivery_groups(
                selected_shipping_option_id.expect("checked selected shipping option"),
            ),
            tenant_id,
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
        shipping_selections.to_vec()
    } else if let Some(selected_shipping_option_id) = selected_shipping_option_id {
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
    let fulfillment_service = FulfillmentService::new(db.clone());

    for selection in selections {
        let Some(shipping_option_id) = selection.selected_shipping_option_id else {
            continue;
        };
        let option = fulfillment_service
            .get_shipping_option(
                tenant_id,
                shipping_option_id,
                requested_locale,
                tenant_default_locale,
            )
            .await
            .map_err(|error| {
                shipping_option_graphql_error(
                    ShippingOptionFailure::owner(shipping_option_id, error),
                    tenant_id,
                    cart.id,
                    selection_count,
                    cart.delivery_groups.len(),
                    requested_currency_code_length,
                    public_channel_slug,
                    requested_locale,
                    tenant_default_locale,
                )
            })?;
        if !option.currency_code.eq_ignore_ascii_case(currency_code) {
            return Err(shipping_option_graphql_error(
                ShippingOptionFailure::currency_mismatch(
                    option.id,
                    option.currency_code.chars().count(),
                ),
                tenant_id,
                cart.id,
                selection_count,
                cart.delivery_groups.len(),
                requested_currency_code_length,
                public_channel_slug,
                requested_locale,
                tenant_default_locale,
            ));
        }
        if !is_metadata_visible_for_public_channel(&option.metadata, public_channel_slug) {
            return Err(shipping_option_graphql_error(
                ShippingOptionFailure::channel_unavailable(option.id),
                tenant_id,
                cart.id,
                selection_count,
                cart.delivery_groups.len(),
                requested_currency_code_length,
                public_channel_slug,
                requested_locale,
                tenant_default_locale,
            ));
        }
        let required_shipping_profiles = BTreeSet::from([
            normalize_shipping_profile_slug(selection.shipping_profile_slug.as_str())
                .unwrap_or_else(|| "default".to_string()),
        ]);
        if !is_shipping_option_compatible_with_profiles(&option, &required_shipping_profiles) {
            return Err(shipping_option_graphql_error(
                ShippingOptionFailure::profile_incompatible(
                    option.id,
                    selection.shipping_profile_slug.chars().count(),
                ),
                tenant_id,
                cart.id,
                selection_count,
                cart.delivery_groups.len(),
                requested_currency_code_length,
                public_channel_slug,
                requested_locale,
                tenant_default_locale,
            ));
        }
    }

    Ok(())
}
