use utoipa::OpenApi;
use utoipa::openapi::path::OperationBuilder;
use utoipa::openapi::request_body::RequestBodyBuilder;
use utoipa::openapi::response::{ResponseBuilder, ResponsesBuilder};
use utoipa::openapi::{Content, Ref};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::controllers::store::list_products,
        crate::controllers::store::show_product,
        crate::controllers::store::list_regions,
        crate::controllers::store::list_shipping_options,
        crate::controllers::store::create_cart,
        crate::controllers::store::get_cart,
        crate::controllers::store::add_cart_line_item,
        crate::controllers::store::update_cart_line_item,
        crate::controllers::store::remove_cart_line_item,
        crate::controllers::store::create_payment_collection,
        crate::controllers::store::complete_cart_checkout,
        crate::controllers::store::get_order,
        crate::controllers::store::get_me,
        crate::controllers::admin::list_products,
        crate::controllers::admin::create_product,
        crate::controllers::admin::show_product,
        crate::controllers::admin::update_product,
        crate::controllers::admin::delete_product,
        crate::controllers::admin::publish_product,
        crate::controllers::admin::unpublish_product,
        crate::controllers::admin::list_orders,
        crate::controllers::admin::show_order,
        crate::controllers::admin::mark_order_paid,
        crate::controllers::admin::ship_order,
        crate::controllers::admin::deliver_order,
        crate::controllers::admin::cancel_order,
        crate::controllers::admin::create_order_return,
        crate::controllers::admin::create_order_return_decision,
        crate::controllers::admin::create_order_change,
        crate::controllers::admin::list_order_changes,
        crate::controllers::admin::show_order_change,
        crate::controllers::admin::apply_order_change,
        crate::controllers::admin::cancel_order_change,
        crate::controllers::admin::list_order_returns,
        crate::controllers::admin::show_order_return,
        crate::controllers::admin::complete_order_return,
        crate::controllers::admin::cancel_order_return,
        crate::controllers::admin::list_payment_collections,
        crate::controllers::admin::show_payment_collection,
        crate::controllers::admin::authorize_payment_collection,
        crate::controllers::admin::capture_payment_collection,
        crate::controllers::admin::cancel_payment_collection,
        crate::controllers::admin::create_refund,
        crate::controllers::admin::list_refunds,
        crate::controllers::admin::show_refund,
        crate::controllers::admin::complete_refund,
        crate::controllers::admin::cancel_refund,
        crate::controllers::admin::list_fulfillments,
        crate::controllers::admin::show_fulfillment,
        crate::controllers::admin::ship_fulfillment,
        crate::controllers::admin::deliver_fulfillment,
        crate::controllers::admin::reopen_fulfillment,
        crate::controllers::admin::reship_fulfillment,
        crate::controllers::admin::cancel_fulfillment,
        crate::controllers::checkout_operations::show_checkout_operation,
        crate::controllers::checkout_operations::compensate_checkout_operation,
        crate::controllers::checkout_operations::sweep_checkout_compensations,
        crate::controllers::return_completion_operations::list_return_completion_operations,
        crate::controllers::return_completion_operations::show_return_completion_operation,
        crate::controllers::return_completion_operations::retry_return_completion_operation,
        crate::controllers::marketplace_financial::list_financial_operator_review,
        crate::controllers::marketplace_financial::show_financial_operation,
        crate::controllers::marketplace_financial::retry_financial_operation,
        crate::controllers::marketplace_financial::list_paid_event_operator_review,
        crate::controllers::marketplace_financial::show_paid_event,
        crate::controllers::marketplace_financial::retry_paid_event,
        crate::controllers::marketplace_financial::run_recovery_sweep,
        crate::controllers::marketplace_reversal_financial::list_operator_review,
        crate::controllers::marketplace_reversal_financial::show_event,
        crate::controllers::marketplace_reversal_financial::retry_event,
        crate::controllers::marketplace_reversal_financial::run_recovery_sweep,
        crate::controllers::marketplace_reversal_financial::list_adaptation_failures_operator_review,
        crate::controllers::marketplace_reversal_financial::show_adaptation_failure,
        crate::controllers::marketplace_reversal_financial::retry_adaptation_failure,
    ),
    components(
        schemas(
            rustok_product::dto::CreateProductInput,
            rustok_product::dto::UpdateProductInput,
            rustok_product::dto::ProductResponse,
            rustok_product::dto::ProductTranslationInput,
            rustok_product::dto::ProductOptionInput,
            rustok_product::dto::ProductTranslationResponse,
            rustok_product::dto::ProductOptionResponse,
            rustok_product::dto::ProductImageResponse,
            rustok_product::dto::PriceResponse,
            rustok_product::entities::product::ProductStatus,
            crate::controllers::products::ListProductsParams,
            crate::controllers::store::StoreListProductsParams,
            crate::controllers::store::StoreContextQuery,
            crate::controllers::store::StoreCreateCartInput,
            crate::controllers::store::StoreCartResponse,
            crate::controllers::store::StoreUpdateCartInput,
            crate::controllers::store::StoreAddCartLineItemInput,
            crate::controllers::store::StoreUpdateCartLineItemInput,
            crate::controllers::store::StoreCreatePaymentCollectionInput,
            crate::controllers::store::StoreCompleteCartInput,
            rustok_cart::dto::CartResponse,
            rustok_cart::dto::CartLineItemResponse,
            rustok_region::dto::RegionResponse,
            rustok_customer::dto::CustomerResponse,
            rustok_fulfillment::dto::ShippingOptionResponse,
            rustok_payment::dto::PaymentCollectionResponse,
            rustok_payment::dto::PaymentResponse,
            rustok_order::dto::OrderResponse,
            rustok_order::dto::OrderLineItemResponse,
            rustok_order::dto::MarkPaidOrderInput,
            rustok_order::dto::ShipOrderInput,
            rustok_order::dto::DeliverOrderInput,
            rustok_order::dto::CancelOrderInput,
            rustok_order::dto::CreateOrderReturnInput,
            crate::CreateReturnDecisionInput,
            crate::ReturnDecisionResponse,
            rustok_order::dto::CreateOrderChangeInput,
            rustok_order::dto::ApplyOrderChangeInput,
            rustok_order::dto::CancelOrderChangeInput,
            rustok_order::dto::OrderChangeResponse,
            rustok_order::dto::CompleteOrderReturnInput,
            rustok_order::dto::CancelOrderReturnInput,
            rustok_order::dto::OrderReturnResponse,
            rustok_payment::dto::AuthorizePaymentInput,
            rustok_payment::dto::CapturePaymentInput,
            rustok_payment::dto::CancelPaymentInput,
            rustok_payment::dto::CreateRefundInput,
            rustok_payment::dto::CompleteRefundInput,
            rustok_payment::dto::CancelRefundInput,
            rustok_payment::dto::RefundResponse,
            crate::controllers::admin::ListPaymentCollectionsParams,
            crate::controllers::admin::ListRefundsParams,
            crate::controllers::admin::ListOrderChangesParams,
            crate::controllers::admin::ListOrderReturnsParams,
            rustok_fulfillment::dto::FulfillmentResponse,
            rustok_fulfillment::dto::ShipFulfillmentInput,
            rustok_fulfillment::dto::DeliverFulfillmentInput,
            rustok_fulfillment::dto::CancelFulfillmentInput,
            crate::controllers::admin::ListFulfillmentsParams,
            crate::dto::ResolveStoreContextInput,
            crate::dto::StoreContextResponse,
            crate::dto::CompleteCheckoutInput,
            crate::dto::CompleteCheckoutResponse,
            crate::controllers::admin::AdminOrderDetailResponse,
            crate::controllers::checkout_operations::AdminCheckoutOperationResponse,
            crate::controllers::checkout_operations::AdminCheckoutCompensationSweepInput,
            crate::controllers::checkout_operations::AdminCheckoutCompensationSweepFailure,
            crate::controllers::checkout_operations::AdminCheckoutCompensationSweepResponse,
            crate::controllers::return_completion_operations::AdminListReturnCompletionOperationsParams,
            crate::services::ReturnCompletionOperationResponse,
            crate::controllers::marketplace_financial::MarketplaceFinancialSweepInput,
            crate::controllers::marketplace_financial::MarketplaceFinancialOperationResponse,
            crate::controllers::marketplace_financial::MarketplacePaidEventResponse,
            crate::controllers::marketplace_financial::MarketplaceFinancialSweepFailureResponse,
            crate::controllers::marketplace_financial::MarketplaceFinancialSweepResponse,
            crate::controllers::marketplace_reversal_financial::MarketplaceReversalSweepInput,
            crate::controllers::marketplace_reversal_financial::MarketplaceReversalEventResponse,
            crate::controllers::marketplace_reversal_financial::MarketplaceReversalAdaptationFailureResponse,
            crate::controllers::marketplace_reversal_financial::MarketplaceReversalSweepFailureResponse,
            crate::controllers::marketplace_reversal_financial::MarketplaceReversalSweepResponse,
        )
    ),
    modifiers(&CommerceOpenApiAddon),
    tags(
        (name = "commerce", description = "Ecommerce endpoints"),
        (name = "store", description = "Storefront ecommerce endpoints"),
        (name = "admin", description = "Administrative ecommerce endpoints"),
        (name = "admin-marketplace-financial", description = "Marketplace financial recovery and reconciliation endpoints")
    )
)]
pub struct CommerceApiDoc;

pub fn openapi_document() -> utoipa::openapi::OpenApi {
    CommerceApiDoc::openapi()
}

pub struct CommerceOpenApiAddon;

impl utoipa::Modify for CommerceOpenApiAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(path_item) = openapi.paths.paths.get_mut("/store/carts/{id}") {
            path_item.post.get_or_insert_with(|| {
                OperationBuilder::new()
                    .request_body(Some(
                        RequestBodyBuilder::new()
                            .content(
                                "application/json",
                                Content::new(Some(Ref::from_schema_name("StoreUpdateCartInput"))),
                            )
                            .build(),
                    ))
                    .responses(
                        ResponsesBuilder::new()
                            .response(
                                "200",
                                ResponseBuilder::new()
                                    .description("Updated cart context")
                                    .content(
                                        "application/json",
                                        Content::new(Some(Ref::from_schema_name(
                                            "StoreCartResponse",
                                        ))),
                                    ),
                            )
                            .build(),
                    )
                    .build()
            });
        }
    }
}
