mod async_graphql_shim {
    pub use ::async_graphql::{Context, Json, Object};

    pub type Error = super::super::query_error_boundary::BoundaryError;
    pub type FieldError = super::super::query_error_boundary::BoundaryError;
    pub type Result<T> = std::result::Result<T, super::super::query_error_boundary::BoundaryError>;
}

mod rustok_api_shim {
    pub use ::rustok_api::{
        AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
        TenantContext, locale_tags_match,
    };

    /// The scoped alias surface `query.rs` consumes when it is compiled inside this
    /// boundary. Only the two constructors the query implementation actually calls are
    /// exposed; the rest of the upstream surface is dead here and would need a lint
    /// suppression to keep, which AGENTS.md §14 refuses.
    pub mod graphql {
        use super::super::super::query_error_boundary::BoundaryError;

        pub trait GraphQLError {
            fn unauthenticated() -> BoundaryError;
            fn permission_denied(message: &str) -> BoundaryError;
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

// Scoped facade aliases: the textually included query implementation resolves each
// owner crate name against this boundary's shim instead of the concrete crate. The
// fulfillment and pricing facades are aliased by the included query source itself.
use self::async_graphql_shim as async_graphql;
use self::rustok_api_shim as rustok_api;
use self::rustok_cart_shim as rustok_cart;
use self::rustok_channel_shim as rustok_channel;
use self::rustok_customer_shim as rustok_customer;
use self::rustok_order_shim as rustok_order;
use self::rustok_payment_shim as rustok_payment;

// The legacy region compatibility format is intercepted inside this boundary so the
// typed region message owns the public payload; every other format! invocation keeps
// the standard library behavior through the generic fallback arm.
macro_rules! format {
    ("{}: {}", $error:ident.code, $error_msg:ident.message) => {
        super::query_error_boundary::RegionGraphqlMessage::new($error)
    };
    ($($tokens:tt)*) => {
        ::std::format!($($tokens)*)
    };
}

include!("../query.rs");
