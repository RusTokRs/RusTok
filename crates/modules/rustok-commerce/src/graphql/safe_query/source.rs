mod async_graphql_shim {
    pub use ::async_graphql::{Context, Object};

    pub type Error = super::super::query_error_boundary::BoundaryError;
    pub type FieldError = super::super::query_error_boundary::BoundaryError;
    pub type Result<T> = std::result::Result<T, super::super::query_error_boundary::BoundaryError>;
}

mod rustok_api_shim {
    pub use ::rustok_api::{
        AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
        TenantContext, locale_tags_match,
    };

    pub mod graphql {
        use super::super::super::query_error_boundary::BoundaryError;

        #[allow(dead_code)]
        pub trait GraphQLError {
            fn unauthenticated() -> BoundaryError;
            fn permission_denied(message: &str) -> BoundaryError;
            fn internal_error(message: &str) -> BoundaryError;
            fn bad_user_input(message: &str) -> BoundaryError;
            fn not_found(message: &str) -> BoundaryError;
        }

        impl GraphQLError for BoundaryError {
            fn unauthenticated() -> BoundaryError {
                BoundaryError::from(
                    <::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::unauthenticated(),
                )
            }

            fn permission_denied(message: &str) -> BoundaryError {
                BoundaryError::from(
                    <::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::permission_denied(message),
                )
            }

            fn internal_error(message: &str) -> BoundaryError {
                BoundaryError::from(
                    <::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::internal_error(message),
                )
            }

            fn bad_user_input(message: &str) -> BoundaryError {
                BoundaryError::from(
                    <::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::bad_user_input(message),
                )
            }

            fn not_found(message: &str) -> BoundaryError {
                BoundaryError::from(
                    <::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::not_found(
                        message,
                    ),
                )
            }
        }

        pub async fn require_module_enabled(
            ctx: &::async_graphql::Context<'_>,
            module_slug: &str,
        ) -> Result<(), BoundaryError> {
            ::rustok_api::graphql::require_module_enabled(ctx, module_slug)
                .await
                .map_err(Into::into)
        }
    }
}

#[path = "source/rustok_cart_shim.rs"]
mod rustok_cart_shim;
#[path = "source/rustok_channel_shim.rs"]
mod rustok_channel_shim;
#[path = "source/rustok_customer_shim.rs"]
mod rustok_customer_shim;
#[path = "source/rustok_fulfillment_shim.rs"]
mod rustok_fulfillment_shim;
#[path = "source/rustok_order_shim.rs"]
mod rustok_order_shim;
#[path = "source/rustok_payment_shim.rs"]
mod rustok_payment_shim;
#[path = "source/rustok_pricing_shim.rs"]
mod rustok_pricing_shim;

// Query implementation dependencies are re-exported from this source boundary so the
// implementation module can consume the same scoped aliases without textual inclusion.
pub(crate) use super::{
    require_commerce_permission, require_storefront_channel_enabled, product_query_tenant,
    types, MODULE_SLUG, PRODUCT_MODULE_SLUG,
};

#[path = "../query.rs"]
mod query_impl;

pub(crate) use query_impl::CommerceQuery;
