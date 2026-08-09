use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextResolve, ResolveInfo,
};
use async_graphql::{Context, ServerResult, Value};
use rustok_api::{AuthContext, PortActor, RequestContext};
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use rustok_fulfillment::{
    FulfillmentReadPort, ShippingOptionAdminReadPort, ShippingOptionReadPort,
    in_process_fulfillment_read_port, in_process_shipping_option_admin_read_port,
    in_process_shipping_option_read_port,
};
use rustok_order::{OrderReadPort, in_process_order_read_port};
use rustok_payment::providers::PaymentProviderRegistry;
use rustok_product::{ProductCatalogCommandRuntime, ProductCatalogReadRuntime};
use sea_orm::DatabaseConnection;

mod fulfillment_commands;
mod payment_commands;
mod payment_reads;
pub use fulfillment_commands::CommerceFulfillmentCommandRuntime;
pub use payment_commands::CommercePaymentCommandRuntime;
pub use payment_reads::CommercePaymentReadRuntime;

/// Host-selected shipping-option read ports used by mounted commerce GraphQL resolvers.
///
/// The application host composes this runtime once and carries it through
/// `HostRuntimeContext`. Directly embedded schemas that do not install the mounted
/// extension retain an explicit in-process compatibility fallback outside the query facade.
#[derive(Clone)]
pub struct CommerceShippingOptionReadRuntime {
    shipping_option_reads: Arc<dyn ShippingOptionReadPort>,
    shipping_option_admin_reads: Arc<dyn ShippingOptionAdminReadPort>,
}

impl CommerceShippingOptionReadRuntime {
    pub fn new(
        shipping_option_reads: Arc<dyn ShippingOptionReadPort>,
        shipping_option_admin_reads: Arc<dyn ShippingOptionAdminReadPort>,
    ) -> Self {
        Self {
            shipping_option_reads,
            shipping_option_admin_reads,
        }
    }

    pub fn in_process(db: DatabaseConnection) -> Self {
        Self::new(
            in_process_shipping_option_read_port(db.clone()),
            in_process_shipping_option_admin_read_port(db),
        )
    }

    pub fn shipping_option_read_port(&self) -> Arc<dyn ShippingOptionReadPort> {
        self.shipping_option_reads.clone()
    }

    pub fn shipping_option_admin_read_port(&self) -> Arc<dyn ShippingOptionAdminReadPort> {
        self.shipping_option_admin_reads.clone()
    }
}

/// Host-selected fulfillment lifecycle projection read port.
///
/// The runtime is separate from shipping-option reads so hosts can select different adapters.
/// Mounted GraphQL resolvers consume it through the shared resolver scope; directly embedded
/// compatibility schemas retain an explicit in-process fallback.
#[derive(Clone)]
pub struct CommerceFulfillmentLifecycleReadRuntime {
    fulfillment_reads: Arc<dyn FulfillmentReadPort>,
}

impl CommerceFulfillmentLifecycleReadRuntime {
    pub fn new(fulfillment_reads: Arc<dyn FulfillmentReadPort>) -> Self {
        Self { fulfillment_reads }
    }

    pub fn in_process(db: DatabaseConnection) -> Self {
        Self::new(in_process_fulfillment_read_port(db))
    }

    pub fn fulfillment_read_port(&self) -> Arc<dyn FulfillmentReadPort> {
        self.fulfillment_reads.clone()
    }
}

/// Host-selected complete order projection read port.
///
/// HTTP admin routes and mounted GraphQL resolvers consume this runtime. GraphQL schema data carries
/// the host-selected value through the resolver scope so compatibility facades cannot silently
/// construct a different owner adapter.
#[derive(Clone)]
pub struct CommerceOrderReadRuntime {
    order_reads: Arc<dyn OrderReadPort>,
}

impl CommerceOrderReadRuntime {
    pub fn new(order_reads: Arc<dyn OrderReadPort>) -> Self {
        Self { order_reads }
    }

    pub fn in_process(
        db: DatabaseConnection,
        event_bus: rustok_outbox::TransactionalEventBus,
    ) -> Self {
        Self::new(in_process_order_read_port(db, event_bus))
    }

    pub fn order_read_port(&self) -> Arc<dyn OrderReadPort> {
        self.order_reads.clone()
    }
}

/// Request-owned identity, channel, and locale facts used to build order read `PortContext` values.
///
/// Mounted GraphQL requests derive the actor only from validated `AuthContext` data. An absent
/// principal uses a stable service actor. Channel and locale come from the host-resolved request
/// context; caller-supplied identity headers are never consulted here.
#[derive(Clone)]
pub(crate) struct CommerceOrderReadCallContext {
    actor: PortActor,
    channel: Option<String>,
    locale: Option<String>,
}

impl CommerceOrderReadCallContext {
    fn from_extension_context(ctx: &ExtensionContext<'_>) -> Self {
        let actor = ctx
            .data_opt::<AuthContext>()
            .map(|auth| PortActor::user(auth.user_id.to_string()))
            .unwrap_or_else(|| PortActor::service("rustok-commerce.graphql-order-query"));
        let request = ctx.data_opt::<RequestContext>();
        let channel = request.and_then(|request| request.channel_slug.clone());
        let locale = request.map(|request| request.locale.clone());
        Self {
            actor,
            channel,
            locale,
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

impl Default for CommerceOrderReadCallContext {
    fn default() -> Self {
        Self {
            actor: PortActor::service("rustok-commerce.graphql-order-query"),
            channel: None,
            locale: None,
        }
    }
}

/// Request-owned normalized public channel used by GraphQL fulfillment owner reads.
///
/// The value is derived from trusted `RequestContext` data by the mounted GraphQL extension.
/// Directly embedded compatibility schemas that do not mount the extension retain `None`.
#[derive(Clone, Default)]
pub(crate) struct CommerceFulfillmentReadCallContext {
    channel: Option<String>,
}

impl CommerceFulfillmentReadCallContext {
    fn from_extension_context(ctx: &ExtensionContext<'_>) -> Self {
        let channel = ctx
            .data_opt::<RequestContext>()
            .and_then(crate::storefront_channel::public_channel_slug_from_request);
        Self { channel }
    }

    pub(crate) fn channel(&self) -> Option<&str> {
        self.channel.as_deref()
    }
}

tokio::task_local! {
    static CURRENT_COMMERCE_SHIPPING_OPTION_READ_RUNTIME: CommerceShippingOptionReadRuntime;
    static CURRENT_COMMERCE_FULFILLMENT_LIFECYCLE_READ_RUNTIME: CommerceFulfillmentLifecycleReadRuntime;
    static CURRENT_COMMERCE_FULFILLMENT_READ_CALL_CONTEXT: CommerceFulfillmentReadCallContext;
    static CURRENT_COMMERCE_ORDER_READ_RUNTIME: CommerceOrderReadRuntime;
    static CURRENT_COMMERCE_ORDER_READ_CALL_CONTEXT: CommerceOrderReadCallContext;
    static CURRENT_COMMERCE_PRODUCT_CATALOG_READ_RUNTIME: ProductCatalogReadRuntime;
    static CURRENT_COMMERCE_PRODUCT_CATALOG_COMMAND_RUNTIME: ProductCatalogCommandRuntime;
}

/// Resolver-scoped bridge from schema runtime data to private compatibility facades.
///
/// The mounted extension carries Payment, shipping-option, fulfillment-lifecycle, order, and
/// Product owner capabilities plus validated request-owned identity/channel/locale facts so every
/// included Commerce resolver uses host-selected adapters for the current async task.
#[derive(Default)]
pub struct CommerceShippingOptionReadScope;

impl ExtensionFactory for CommerceShippingOptionReadScope {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(CommerceShippingOptionReadScopeExtension)
    }
}

struct CommerceShippingOptionReadScopeExtension;

#[async_trait::async_trait]
impl Extension for CommerceShippingOptionReadScopeExtension {
    async fn resolve(
        &self,
        ctx: &ExtensionContext<'_>,
        info: ResolveInfo<'_>,
        next: NextResolve<'_>,
    ) -> ServerResult<Option<Value>> {
        let Some(runtime_data) = ctx.data_opt::<CommerceGraphqlRuntimeData>() else {
            return next.run(ctx, info).await;
        };
        let order_call_context = CommerceOrderReadCallContext::from_extension_context(ctx);
        let fulfillment_call_context =
            CommerceFulfillmentReadCallContext::from_extension_context(ctx);
        let payment_call_context =
            payment_reads::CommercePaymentReadCallContext::from_extension_context(ctx);
        payment_reads::scope_current_payment_reads(
            runtime_data.payment_read_runtime(),
            payment_call_context,
            CURRENT_COMMERCE_SHIPPING_OPTION_READ_RUNTIME.scope(
                runtime_data.shipping_option_read_runtime(),
                CURRENT_COMMERCE_FULFILLMENT_LIFECYCLE_READ_RUNTIME.scope(
                    runtime_data.fulfillment_lifecycle_read_runtime(),
                    CURRENT_COMMERCE_FULFILLMENT_READ_CALL_CONTEXT.scope(
                        fulfillment_call_context,
                        CURRENT_COMMERCE_ORDER_READ_CALL_CONTEXT.scope(
                            order_call_context,
                            CURRENT_COMMERCE_ORDER_READ_RUNTIME.scope(
                                runtime_data.order_read_runtime(),
                                CURRENT_COMMERCE_PRODUCT_CATALOG_READ_RUNTIME.scope(
                                    runtime_data.product_catalog_read_runtime(),
                                    CURRENT_COMMERCE_PRODUCT_CATALOG_COMMAND_RUNTIME.scope(
                                        runtime_data.product_catalog_command_runtime(),
                                        next.run(ctx, info),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        )
        .await
    }
}

pub(crate) fn shipping_option_read_runtime_for_current_graphql_scope(
    db: DatabaseConnection,
) -> CommerceShippingOptionReadRuntime {
    CURRENT_COMMERCE_SHIPPING_OPTION_READ_RUNTIME
        .try_with(Clone::clone)
        .unwrap_or_else(|_| CommerceShippingOptionReadRuntime::in_process(db))
}

pub(crate) fn fulfillment_lifecycle_read_runtime_for_current_graphql_scope(
    db: DatabaseConnection,
) -> CommerceFulfillmentLifecycleReadRuntime {
    CURRENT_COMMERCE_FULFILLMENT_LIFECYCLE_READ_RUNTIME
        .try_with(Clone::clone)
        .unwrap_or_else(|_| CommerceFulfillmentLifecycleReadRuntime::in_process(db))
}

pub(crate) fn fulfillment_read_call_context_for_current_graphql_scope(
) -> CommerceFulfillmentReadCallContext {
    CURRENT_COMMERCE_FULFILLMENT_READ_CALL_CONTEXT
        .try_with(Clone::clone)
        .unwrap_or_default()
}

pub(crate) fn order_read_runtime_for_current_graphql_scope(
    db: DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
) -> CommerceOrderReadRuntime {
    CURRENT_COMMERCE_ORDER_READ_RUNTIME
        .try_with(Clone::clone)
        .unwrap_or_else(|_| CommerceOrderReadRuntime::in_process(db, event_bus))
}

pub(crate) fn order_read_call_context_for_current_graphql_scope() -> CommerceOrderReadCallContext {
    CURRENT_COMMERCE_ORDER_READ_CALL_CONTEXT
        .try_with(Clone::clone)
        .unwrap_or_default()
}

pub(crate) fn payment_read_runtime_for_current_graphql_scope(
    db: DatabaseConnection,
) -> CommercePaymentReadRuntime {
    payment_reads::runtime_for_current_graphql_scope(db)
}

pub(crate) fn payment_read_call_context_for_current_graphql_scope(
) -> (PortActor, Option<String>, Option<String>) {
    let context = payment_reads::call_context_for_current_graphql_scope();
    (
        context.actor(),
        context.channel().map(str::to_owned),
        context.locale().map(str::to_owned),
    )
}

pub(crate) fn product_catalog_read_runtime_for_current_graphql_scope(
    db: DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
) -> ProductCatalogReadRuntime {
    CURRENT_COMMERCE_PRODUCT_CATALOG_READ_RUNTIME
        .try_with(Clone::clone)
        .unwrap_or_else(|_| ProductCatalogReadRuntime::in_process(db, event_bus))
}

pub(crate) fn product_catalog_command_runtime_for_current_graphql_scope(
    db: DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
) -> ProductCatalogCommandRuntime {
    CURRENT_COMMERCE_PRODUCT_CATALOG_COMMAND_RUNTIME
        .try_with(Clone::clone)
        .unwrap_or_else(|_| ProductCatalogCommandRuntime::in_process(db, event_bus))
}

/// Provider registries and host-selectable ports available to every commerce GraphQL resolver.
///
/// Hosts supply composed capabilities through `HostRuntimeContext`. The built-in manual provider
/// registries remain deterministic fallbacks. Mounted Payment/Fulfillment reads and commands,
/// shipping-option, Product catalog, and order reads consume host-selected runtime data. Directly
/// embedded compatibility schemas retain explicit in-process owner-runtime fallbacks.
#[derive(Clone)]
pub struct CommerceGraphqlRuntimeData {
    payment_provider_registry: PaymentProviderRegistry,
    fulfillment_provider_registry: FulfillmentProviderRegistry,
    #[cfg(feature = "marketplace-financial")]
    marketplace_financial_runtime: crate::MarketplaceFinancialRuntime,
    payment_read_runtime: CommercePaymentReadRuntime,
    payment_command_runtime: CommercePaymentCommandRuntime,
    fulfillment_command_runtime: CommerceFulfillmentCommandRuntime,
    shipping_option_read_runtime: CommerceShippingOptionReadRuntime,
    fulfillment_lifecycle_read_runtime: CommerceFulfillmentLifecycleReadRuntime,
    order_read_runtime: CommerceOrderReadRuntime,
    product_catalog_read_runtime: ProductCatalogReadRuntime,
    product_catalog_command_runtime: ProductCatalogCommandRuntime,
}

impl CommerceGraphqlRuntimeData {
    pub fn payment_provider_registry(&self) -> PaymentProviderRegistry {
        self.payment_provider_registry.clone()
    }

    pub fn fulfillment_provider_registry(&self) -> FulfillmentProviderRegistry {
        self.fulfillment_provider_registry.clone()
    }

    #[cfg(feature = "marketplace-financial")]
    pub fn marketplace_financial_runtime(&self) -> crate::MarketplaceFinancialRuntime {
        self.marketplace_financial_runtime.clone()
    }

    pub fn payment_read_runtime(&self) -> CommercePaymentReadRuntime {
        self.payment_read_runtime.clone()
    }

    pub fn payment_command_runtime(&self) -> CommercePaymentCommandRuntime {
        self.payment_command_runtime.clone()
    }

    pub fn fulfillment_command_runtime(&self) -> CommerceFulfillmentCommandRuntime {
        self.fulfillment_command_runtime.clone()
    }

    pub fn shipping_option_read_runtime(&self) -> CommerceShippingOptionReadRuntime {
        self.shipping_option_read_runtime.clone()
    }

    pub fn fulfillment_lifecycle_read_runtime(&self) -> CommerceFulfillmentLifecycleReadRuntime {
        self.fulfillment_lifecycle_read_runtime.clone()
    }

    pub fn order_read_runtime(&self) -> CommerceOrderReadRuntime {
        self.order_read_runtime.clone()
    }

    pub fn product_catalog_read_runtime(&self) -> ProductCatalogReadRuntime {
        self.product_catalog_read_runtime.clone()
    }

    pub fn product_catalog_command_runtime(&self) -> ProductCatalogCommandRuntime {
        self.product_catalog_command_runtime.clone()
    }
}

/// Capability-owned factory consumed by manifest-generated schema composition.
pub fn attach_schema_data(
    inputs: &rustok_api::graphql::GraphqlRuntimeInputs,
) -> Result<CommerceGraphqlRuntimeData, String> {
    Ok(CommerceGraphqlRuntimeData {
        payment_provider_registry: inputs
            .shared_get::<PaymentProviderRegistry>()
            .unwrap_or_else(PaymentProviderRegistry::with_manual_provider),
        fulfillment_provider_registry: inputs
            .shared_get::<FulfillmentProviderRegistry>()
            .unwrap_or_else(FulfillmentProviderRegistry::with_manual_provider),
        #[cfg(feature = "marketplace-financial")]
        marketplace_financial_runtime: inputs
            .shared_get::<crate::MarketplaceFinancialRuntime>()
            .ok_or_else(|| {
                "commerce marketplace-financial GraphQL requires MarketplaceFinancialRuntime in host composition"
                    .to_string()
            })?,
        payment_read_runtime: inputs
            .shared_get::<CommercePaymentReadRuntime>()
            .unwrap_or_else(|| {
                CommercePaymentReadRuntime::new(
                    inputs
                        .shared_get::<rustok_payment::PaymentAdminReadRuntime>()
                        .unwrap_or_else(|| {
                            rustok_payment::PaymentAdminReadRuntime::in_process(inputs.db_clone())
                        }),
                    inputs
                        .shared_get::<rustok_payment::PaymentOrderReadRuntime>()
                        .unwrap_or_else(|| {
                            rustok_payment::PaymentOrderReadRuntime::in_process(inputs.db_clone())
                        }),
                    inputs
                        .shared_get::<rustok_payment::PaymentCartReadRuntime>()
                        .unwrap_or_else(|| {
                            rustok_payment::PaymentCartReadRuntime::in_process(inputs.db_clone())
                        }),
                )
            }),
        payment_command_runtime: inputs
            .shared_get::<CommercePaymentCommandRuntime>()
            .unwrap_or_else(|| CommercePaymentCommandRuntime::from_graphql_inputs(inputs)),
        fulfillment_command_runtime: inputs
            .shared_get::<CommerceFulfillmentCommandRuntime>()
            .unwrap_or_else(|| CommerceFulfillmentCommandRuntime::from_graphql_inputs(inputs)),
        shipping_option_read_runtime: inputs
            .shared_get::<CommerceShippingOptionReadRuntime>()
            .ok_or_else(|| {
                "commerce GraphQL requires CommerceShippingOptionReadRuntime in host composition"
                    .to_string()
            })?,
        fulfillment_lifecycle_read_runtime: inputs
            .shared_get::<CommerceFulfillmentLifecycleReadRuntime>()
            .unwrap_or_else(|| {
                CommerceFulfillmentLifecycleReadRuntime::in_process(inputs.db_clone())
            }),
        order_read_runtime: inputs
            .shared_get::<CommerceOrderReadRuntime>()
            .ok_or_else(|| {
                "commerce GraphQL requires CommerceOrderReadRuntime in host composition".to_string()
            })?,
        product_catalog_read_runtime: inputs
            .shared_get::<ProductCatalogReadRuntime>()
            .ok_or_else(|| {
                "commerce GraphQL requires ProductCatalogReadRuntime in host composition"
                    .to_string()
            })?,
        product_catalog_command_runtime: inputs
            .shared_get::<ProductCatalogCommandRuntime>()
            .ok_or_else(|| {
                "commerce GraphQL requires ProductCatalogCommandRuntime in host composition"
                    .to_string()
            })?,
    })
}

pub(crate) fn payment_provider_registry_from_context(ctx: &Context<'_>) -> PaymentProviderRegistry {
    ctx.data_opt::<CommerceGraphqlRuntimeData>()
        .map(CommerceGraphqlRuntimeData::payment_provider_registry)
        .unwrap_or_else(PaymentProviderRegistry::with_manual_provider)
}

pub(crate) fn payment_command_runtime_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
) -> CommercePaymentCommandRuntime {
    ctx.data_opt::<CommerceGraphqlRuntimeData>()
        .map(CommerceGraphqlRuntimeData::payment_command_runtime)
        .unwrap_or_else(|| {
            CommercePaymentCommandRuntime::in_process(
                db,
                payment_provider_registry_from_context(ctx),
            )
        })
}

pub(crate) fn fulfillment_command_runtime_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
) -> CommerceFulfillmentCommandRuntime {
    ctx.data_opt::<CommerceGraphqlRuntimeData>()
        .map(CommerceGraphqlRuntimeData::fulfillment_command_runtime)
        .unwrap_or_else(|| {
            let provider_registry = ctx
                .data_opt::<CommerceGraphqlRuntimeData>()
                .map(CommerceGraphqlRuntimeData::fulfillment_provider_registry)
                .unwrap_or_else(FulfillmentProviderRegistry::with_manual_provider);
            CommerceFulfillmentCommandRuntime::in_process(db, provider_registry)
        })
}

pub(crate) fn manual_fulfillment_owner_orchestration_from_context(
    ctx: &Context<'_>,
) -> Option<crate::services::AdminManualFulfillmentOrchestrationService> {
    ctx.data_opt::<CommerceGraphqlRuntimeData>().map(|runtime| {
        crate::services::AdminManualFulfillmentOrchestrationService::new(
            runtime.order_read_runtime().order_read_port(),
            runtime
                .fulfillment_lifecycle_read_runtime()
                .fulfillment_read_port(),
            runtime
                .shipping_option_read_runtime()
                .shipping_option_read_port(),
            runtime.fulfillment_command_runtime().create_command_port(),
        )
    })
}

pub(crate) fn refund_reconciliation_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
) -> crate::RefundReconciliationService {
    crate::RefundReconciliationService::new(db)
        .with_provider_registry(payment_provider_registry_from_context(ctx))
}

pub(crate) fn fulfillment_orchestration_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
) -> crate::FulfillmentOrchestrationService {
    let service = crate::FulfillmentOrchestrationService::new(db);
    match ctx.data_opt::<CommerceGraphqlRuntimeData>() {
        Some(runtime) => service.with_provider_registry(runtime.fulfillment_provider_registry()),
        None => service,
    }
}

pub(crate) fn post_order_orchestration_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
) -> crate::PostOrderOrchestrationService {
    crate::PostOrderOrchestrationService::new(db, event_bus)
        .with_payment_provider_registry(payment_provider_registry_from_context(ctx))
}

pub(crate) fn order_change_orchestration_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
) -> crate::OrderChangeOrchestrationService {
    crate::OrderChangeOrchestrationService::new(db, event_bus)
        .with_payment_provider_registry(payment_provider_registry_from_context(ctx))
}

pub(crate) fn return_completion_orchestration_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
) -> crate::ReturnCompletionOrchestrationService {
    crate::ReturnCompletionOrchestrationService::new(db, event_bus)
        .with_payment_provider_registry(payment_provider_registry_from_context(ctx))
}
