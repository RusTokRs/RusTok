use std::{future::Future, sync::Arc};

use async_graphql::extensions::ExtensionContext;
use rustok_api::{AuthContext, PortActor, RequestContext};
use rustok_payment::{
    PaymentAdminReadPort, PaymentAdminReadRuntime, PaymentCartReadPort, PaymentCartReadRuntime,
    PaymentOrderReadPort, PaymentOrderReadRuntime,
};
use sea_orm::DatabaseConnection;

/// Host-selected Payment owner reads consumed by mounted Commerce GraphQL resolvers.
///
/// The three underlying owner capabilities intentionally retain their existing narrow ownership:
/// admin collection/refund projections, order-associated lookup, and storefront cart-associated
/// lookup. Commerce only composes them for the resolver compatibility facade.
#[derive(Clone)]
pub struct CommercePaymentReadRuntime {
    admin_reads: PaymentAdminReadRuntime,
    order_reads: PaymentOrderReadRuntime,
    cart_reads: PaymentCartReadRuntime,
}

impl CommercePaymentReadRuntime {
    pub fn new(
        admin_reads: PaymentAdminReadRuntime,
        order_reads: PaymentOrderReadRuntime,
        cart_reads: PaymentCartReadRuntime,
    ) -> Self {
        Self {
            admin_reads,
            order_reads,
            cart_reads,
        }
    }

    pub fn in_process(db: DatabaseConnection) -> Self {
        Self::new(
            PaymentAdminReadRuntime::in_process(db.clone()),
            PaymentOrderReadRuntime::in_process(db.clone()),
            PaymentCartReadRuntime::in_process(db),
        )
    }

    pub fn admin_read_port(&self) -> Arc<dyn PaymentAdminReadPort> {
        self.admin_reads.read_port()
    }

    pub fn order_read_port(&self) -> Arc<dyn PaymentOrderReadPort> {
        self.order_reads.read_port()
    }

    pub fn cart_read_port(&self) -> Arc<dyn PaymentCartReadPort> {
        self.cart_reads.read_port()
    }
}

/// Trusted request-owned identity facts used for Payment owner read `PortContext` values.
#[derive(Clone)]
pub(crate) struct CommercePaymentReadCallContext {
    actor: PortActor,
    channel: Option<String>,
    locale: Option<String>,
}

impl CommercePaymentReadCallContext {
    pub(crate) fn from_extension_context(ctx: &ExtensionContext<'_>) -> Self {
        let actor = ctx
            .data_opt::<AuthContext>()
            .map(|auth| PortActor::user(auth.user_id.to_string()))
            .unwrap_or_else(|| PortActor::service("rustok-commerce.graphql-payment-query"));
        let request = ctx.data_opt::<RequestContext>();
        Self {
            actor,
            channel: request.and_then(|request| request.channel_slug.clone()),
            locale: request.map(|request| request.locale.clone()),
        }
    }

    pub(crate) fn actor(&self) -> PortActor {
        self.actor.clone()
    }

    pub(crate) fn channel(&self) -> Option<&str> {
        self.channel.as_deref()
    }

    pub(crate) fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }
}

impl Default for CommercePaymentReadCallContext {
    fn default() -> Self {
        Self {
            actor: PortActor::service("rustok-commerce.graphql-payment-query"),
            channel: None,
            locale: None,
        }
    }
}

tokio::task_local! {
    static CURRENT_COMMERCE_PAYMENT_READ_RUNTIME: CommercePaymentReadRuntime;
    static CURRENT_COMMERCE_PAYMENT_READ_CALL_CONTEXT: CommercePaymentReadCallContext;
}

pub(crate) async fn scope_current_payment_reads<F, T>(
    runtime: CommercePaymentReadRuntime,
    call_context: CommercePaymentReadCallContext,
    future: F,
) -> T
where
    F: Future<Output = T>,
{
    CURRENT_COMMERCE_PAYMENT_READ_RUNTIME
        .scope(
            runtime,
            CURRENT_COMMERCE_PAYMENT_READ_CALL_CONTEXT.scope(call_context, future),
        )
        .await
}

pub(crate) fn runtime_for_current_graphql_scope(
    db: DatabaseConnection,
) -> CommercePaymentReadRuntime {
    CURRENT_COMMERCE_PAYMENT_READ_RUNTIME
        .try_with(Clone::clone)
        .unwrap_or_else(|_| CommercePaymentReadRuntime::in_process(db))
}

pub(crate) fn call_context_for_current_graphql_scope() -> CommercePaymentReadCallContext {
    CURRENT_COMMERCE_PAYMENT_READ_CALL_CONTEXT
        .try_with(Clone::clone)
        .unwrap_or_default()
}
