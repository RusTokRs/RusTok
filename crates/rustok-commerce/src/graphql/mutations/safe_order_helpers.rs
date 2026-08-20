use async_graphql::{Context, ErrorExtensions, FieldError, Result};
use rustok_api::{
    AuthContext, PortActor, PortContext, PortError, PortErrorKind, RequestContext, TenantContext,
    graphql::GraphQLError,
};
use rustok_order::ReadOrderProjectionRequest;
use uuid::Uuid;

pub(crate) use super::cart_safe_helpers::*;
use crate::storefront_shipping::normalize_shipping_profile_slug;
use crate::{CommerceError, ShippingProfileService};

const STOREFRONT_ORDER_GRAPHQL_OWNER: &str = "rustok_order.storefront_access";
const SHIPPING_PROFILE_GRAPHQL_OWNER: &str = "rustok_commerce.shipping_profiles";
const STOREFRONT_GRAPHQL_HELPER_BOUNDARY: &str = "commerce_storefront_graphql_helper";

type StorefrontGraphqlPolicy = (&'static str, &'static str, bool, &'static str);

#[derive(Clone, Copy)]
struct StorefrontOrderGraphqlErrorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
    customer_id: Uuid,
    order_id: Option<Uuid>,
    order_return_id: Option<Uuid>,
    order_change_id: Option<Uuid>,
    operation: &'static str,
}

impl StorefrontOrderGraphqlErrorContext {
    fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        customer_id: Uuid,
        order_id: Uuid,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            actor_id,
            customer_id,
            order_id: Some(order_id),
            order_return_id: None,
            order_change_id: None,
            operation,
        }
    }
}

#[derive(Clone, Copy)]
struct ShippingProfileGraphqlErrorContext<'a> {
    tenant_id: Uuid,
    requested_slug: Option<&'a str>,
    requested_profile_count: Option<usize>,
    shipping_profile_id: Option<Uuid>,
    operation: &'static str,
}

impl<'a> ShippingProfileGraphqlErrorContext<'a> {
    fn single(tenant_id: Uuid, requested_slug: &'a str, operation: &'static str) -> Self {
        Self {
            tenant_id,
            requested_slug: Some(requested_slug),
            requested_profile_count: Some(1),
            shipping_profile_id: None,
            operation,
        }
    }

    fn batch(tenant_id: Uuid, requested_profile_count: usize, operation: &'static str) -> Self {
        Self {
            tenant_id,
            requested_slug: None,
            requested_profile_count: Some(requested_profile_count),
            shipping_profile_id: None,
            operation,
        }
    }
}

fn public_graphql_error(
    message: impl Into<String>,
    code: &'static str,
    retryable: bool,
) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
    })
}

fn storefront_order_graphql_error_policy(error: &PortError) -> StorefrontGraphqlPolicy {
    match &error.kind {
        PortErrorKind::Validation | PortErrorKind::Forbidden => (
            "Order request is invalid",
            "ORDER_REQUEST_INVALID",
            false,
            "validation",
        ),
        PortErrorKind::NotFound => (
            "Order resource was not found",
            "ORDER_RESOURCE_NOT_FOUND",
            false,
            "order_not_found",
        ),
        PortErrorKind::Conflict => (
            "Order operation conflicts with the current state",
            "ORDER_STATE_CONFLICT",
            false,
            "state_conflict",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            "Order service is temporarily unavailable",
            "ORDER_TEMPORARILY_UNAVAILABLE",
            true,
            "temporarily_unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            "Order operation could not be completed safely",
            "ORDER_OPERATION_FAILED",
            false,
            "invariant_violation",
        ),
    }
}

fn order_graphql_error(
    context: StorefrontOrderGraphqlErrorContext,
    port_context: &PortContext,
    error: PortError,
) -> async_graphql::Error {
    let (message, code, retryable, error_kind) = storefront_order_graphql_error_policy(&error);
    tracing::error!(
        owner = STOREFRONT_ORDER_GRAPHQL_OWNER,
        tenant_id_non_nil = !context.tenant_id.is_nil(),
        actor_id_non_nil = !context.actor_id.is_nil(),
        customer_id_non_nil = !context.customer_id.is_nil(),
        order_id_non_nil = context.order_id.is_some_and(|value| !value.is_nil()),
        order_return_id_present = context.order_return_id.is_some(),
        order_change_id_present = context.order_change_id.is_some(),
        operation = %context.operation,
        correlation_id = %port_context.correlation_id,
        owner_error_kind = ?error.kind,
        owner_code_length = error.code.chars().count(),
        error_kind,
        public_code = code,
        retryable,
        boundary = STOREFRONT_GRAPHQL_HELPER_BOUNDARY,
        "commerce GraphQL storefront order owner read failed"
    );
    public_graphql_error(message, code, retryable)
}

fn storefront_order_read_context(
    ctx: &Context<'_>,
    tenant_id: Uuid,
    auth: &AuthContext,
    tenant_default_locale: &str,
    order_id: Uuid,
) -> PortContext {
    let locale = if tenant_default_locale.trim().is_empty() {
        "und"
    } else {
        tenant_default_locale
    };
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        locale,
        format!("commerce-storefront-order-access:{order_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match ctx
        .data_opt::<RequestContext>()
        .and_then(|request| request.channel_slug.as_deref())
    {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn shipping_profile_graphql_error_policy(
    context: &mut ShippingProfileGraphqlErrorContext<'_>,
    error: &CommerceError,
) -> (String, &'static str, bool, &'static str) {
    match error {
        CommerceError::Validation(detail) => (
            if detail.is_empty() {
                "Shipping profile request is invalid".to_string()
            } else {
                detail.clone()
            },
            "SHIPPING_PROFILE_REQUEST_INVALID",
            false,
            "validation",
        ),
        CommerceError::InvalidPrice(_) => (
            "Shipping profile request is invalid".to_string(),
            "SHIPPING_PROFILE_REQUEST_INVALID",
            false,
            "invalid_price",
        ),
        CommerceError::InvalidOptionCombination => (
            "Shipping profile request is invalid".to_string(),
            "SHIPPING_PROFILE_REQUEST_INVALID",
            false,
            "invalid_option_combination",
        ),
        CommerceError::NoVariants => (
            "Shipping profile request is invalid".to_string(),
            "SHIPPING_PROFILE_REQUEST_INVALID",
            false,
            "no_variants",
        ),
        CommerceError::ShippingProfileNotFound(shipping_profile_id) => {
            context.shipping_profile_id = Some(*shipping_profile_id);
            (
                "Shipping profile was not found".to_string(),
                "SHIPPING_PROFILE_NOT_FOUND",
                false,
                "shipping_profile_not_found",
            )
        }
        CommerceError::DuplicateShippingProfileSlug(_) => (
            "Shipping profile conflicts with the current state".to_string(),
            "SHIPPING_PROFILE_STATE_CONFLICT",
            false,
            "duplicate_shipping_profile_slug",
        ),
        CommerceError::Database(_) => (
            "Shipping profile service is temporarily unavailable".to_string(),
            "SHIPPING_PROFILE_TEMPORARILY_UNAVAILABLE",
            true,
            "database",
        ),
        CommerceError::ProductNotFound(_)
        | CommerceError::VariantNotFound(_)
        | CommerceError::DuplicateHandle { .. }
        | CommerceError::DuplicateSku(_)
        | CommerceError::InsufficientInventory { .. }
        | CommerceError::CannotDeletePublished
        | CommerceError::Rich(_)
        | CommerceError::Core(_) => (
            "Shipping profile operation could not be completed safely".to_string(),
            "SHIPPING_PROFILE_OPERATION_FAILED",
            false,
            "unexpected_owner_error",
        ),
    }
}

fn shipping_profile_graphql_error(
    mut context: ShippingProfileGraphqlErrorContext<'_>,
    error: CommerceError,
) -> async_graphql::Error {
    let (message, code, retryable, error_kind) =
        shipping_profile_graphql_error_policy(&mut context, &error);
    tracing::error!(
        error = ?error,
        owner = SHIPPING_PROFILE_GRAPHQL_OWNER,
        tenant_id = %context.tenant_id,
        requested_slug = ?context.requested_slug,
        requested_profile_count = ?context.requested_profile_count,
        shipping_profile_id = ?context.shipping_profile_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        retryable,
        boundary = STOREFRONT_GRAPHQL_HELPER_BOUNDARY,
        "commerce GraphQL shipping profile helper failed"
    );
    public_graphql_error(message, code, retryable)
}

pub(crate) async fn ensure_storefront_order_access(
    db: &sea_orm::DatabaseConnection,
    event_bus: &rustok_outbox::TransactionalEventBus,
    tenant_id: Uuid,
    ctx: &Context<'_>,
    order_id: Uuid,
) -> Result<()> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let tenant = ctx.data::<TenantContext>()?;
    let customer_id = super::cart_safe_helpers::resolve_optional_storefront_customer_id(
        db,
        tenant_id,
        Some(auth),
    )
    .await?
    .ok_or_else(<FieldError as GraphQLError>::unauthenticated)?;

    let runtime = crate::graphql_runtime::order_read_runtime_for_current_graphql_scope(
        db.clone(),
        event_bus.clone(),
    );
    let port_context = storefront_order_read_context(
        ctx,
        tenant_id,
        auth,
        tenant.default_locale.as_str(),
        order_id,
    );
    let order = runtime
        .order_read_port()
        .read_order_projection(
            port_context.clone(),
            ReadOrderProjectionRequest {
                order_id,
                tenant_default_locale: None,
            },
        )
        .await
        .map_err(|error| {
            order_graphql_error(
                StorefrontOrderGraphqlErrorContext::new(
                    tenant_id,
                    auth.user_id,
                    customer_id,
                    order_id,
                    "ensure_storefront_order_access",
                ),
                &port_context,
                error,
            )
        })?;

    if order.customer_id != Some(customer_id) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Order does not belong to the current customer",
        ));
    }

    Ok(())
}

pub(crate) async fn validate_product_shipping_profile_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    shipping_profile_slug: Option<&str>,
) -> Result<()> {
    let Some(slug) = shipping_profile_slug.and_then(normalize_shipping_profile_slug) else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slug_exists(tenant_id, &slug)
        .await
        .map_err(|error| {
            shipping_profile_graphql_error(
                ShippingProfileGraphqlErrorContext::single(
                    tenant_id,
                    &slug,
                    "validate_product_shipping_profile_input",
                ),
                error,
            )
        })?;

    Ok(())
}

pub(crate) async fn validate_shipping_option_profile_inputs(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    allowed_shipping_profile_slugs: Option<&Vec<String>>,
) -> Result<()> {
    let Some(slugs) = allowed_shipping_profile_slugs else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slugs_exist(tenant_id, slugs.iter())
        .await
        .map_err(|error| {
            shipping_profile_graphql_error(
                ShippingProfileGraphqlErrorContext::batch(
                    tenant_id,
                    slugs.len(),
                    "validate_shipping_option_profile_inputs",
                ),
                error,
            )
        })?;

    Ok(())
}
