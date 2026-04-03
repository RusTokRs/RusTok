use axum::{
    extract::State,
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use loco_rs::app::AppContext;
use loco_rs::{controller::Routes, Result};
use utoipa::openapi::path::OperationBuilder;
use utoipa::openapi::request_body::RequestBodyBuilder;
use utoipa::openapi::response::{ResponseBuilder, ResponsesBuilder};
use utoipa::openapi::{Content, OpenApi as OpenApiDoc, Ref};

use crate::common::settings::RustokSettings;
use crate::error::Error;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "RusTok API",
        version = "1.0.0",
        description = "Unified API for RusTok CMS & Commerce"
    ),
    paths(
        // Auth
        crate::controllers::auth::login,
        crate::controllers::auth::register,
        crate::controllers::auth::refresh,
        crate::controllers::auth::logout,
        crate::controllers::auth::me,
        crate::controllers::auth::accept_invite,
        crate::controllers::auth::request_verification,
        crate::controllers::auth::confirm_verification,
        // Blog
        crate::controllers::blog::posts::list_posts,
        crate::controllers::blog::posts::get_post,
        crate::controllers::blog::posts::create_post,
        crate::controllers::blog::posts::update_post,
        crate::controllers::blog::posts::delete_post,
        crate::controllers::blog::posts::publish_post,
        crate::controllers::blog::posts::unpublish_post,
        // Forum
        crate::controllers::forum::categories::list_categories,
        crate::controllers::forum::categories::get_category,
        crate::controllers::forum::categories::create_category,
        crate::controllers::forum::categories::update_category,
        crate::controllers::forum::categories::delete_category,
        crate::controllers::forum::topics::list_topics,
        crate::controllers::forum::topics::get_topic,
        crate::controllers::forum::topics::create_topic,
        crate::controllers::forum::topics::update_topic,
        crate::controllers::forum::topics::delete_topic,
        crate::controllers::forum::replies::list_replies,
        crate::controllers::forum::replies::get_reply,
        crate::controllers::forum::replies::create_reply,
        crate::controllers::forum::replies::update_reply,
        crate::controllers::forum::replies::delete_reply,
        // Pages
        crate::controllers::pages::get_page,
        crate::controllers::pages::create_page,
        crate::controllers::pages::update_page,
        crate::controllers::pages::delete_page,
        crate::controllers::pages::create_block,
        crate::controllers::pages::update_block,
        crate::controllers::pages::delete_block,
        crate::controllers::pages::reorder_blocks,
        // Commerce
        crate::controllers::commerce::store::list_products,
        crate::controllers::commerce::store::show_product,
        crate::controllers::commerce::store::list_regions,
        crate::controllers::commerce::store::list_shipping_options,
        crate::controllers::commerce::store::create_cart,
        crate::controllers::commerce::store::get_cart,
        crate::controllers::commerce::store::add_cart_line_item,
        crate::controllers::commerce::store::update_cart_line_item,
        crate::controllers::commerce::store::remove_cart_line_item,
        crate::controllers::commerce::store::create_payment_collection,
        crate::controllers::commerce::store::complete_cart_checkout,
        crate::controllers::commerce::store::get_order,
        crate::controllers::commerce::store::get_me,
        crate::controllers::commerce::admin::list_products,
        crate::controllers::commerce::admin::create_product,
        crate::controllers::commerce::admin::show_product,
        crate::controllers::commerce::admin::update_product,
        crate::controllers::commerce::admin::delete_product,
        crate::controllers::commerce::admin::publish_product,
        crate::controllers::commerce::admin::unpublish_product,
        crate::controllers::commerce::admin::list_orders,
        crate::controllers::commerce::admin::show_order,
        crate::controllers::commerce::admin::mark_order_paid,
        crate::controllers::commerce::admin::ship_order,
        crate::controllers::commerce::admin::deliver_order,
        crate::controllers::commerce::admin::cancel_order,
        crate::controllers::commerce::admin::list_payment_collections,
        crate::controllers::commerce::admin::show_payment_collection,
        crate::controllers::commerce::admin::authorize_payment_collection,
        crate::controllers::commerce::admin::capture_payment_collection,
        crate::controllers::commerce::admin::cancel_payment_collection,
        crate::controllers::commerce::admin::list_fulfillments,
        crate::controllers::commerce::admin::show_fulfillment,
        crate::controllers::commerce::admin::ship_fulfillment,
        crate::controllers::commerce::admin::deliver_fulfillment,
        crate::controllers::commerce::admin::cancel_fulfillment,
        // Health
        crate::controllers::health::health,
        crate::controllers::health::live,
        crate::controllers::health::ready,
        crate::controllers::health::modules,
        // Metrics
        crate::controllers::metrics::metrics,
        // Marketplace
        crate::controllers::marketplace_registry::catalog,
        crate::controllers::marketplace_registry::catalog_module,
        crate::controllers::marketplace_registry::publish,
        crate::controllers::marketplace_registry::publish_status,
        crate::controllers::marketplace_registry::upload_publish_artifact,
        crate::controllers::marketplace_registry::approve_publish_request,
        crate::controllers::marketplace_registry::reject_publish_request,
        crate::controllers::marketplace_registry::yank,
        // Swagger
        crate::controllers::swagger::openapi_json,
        crate::controllers::swagger::openapi_yaml,
        // Admin Events
        crate::controllers::admin_events::list_dlq,
        crate::controllers::admin_events::replay_dlq_event,
    ),
    components(
        schemas(
            crate::controllers::auth::LoginParams,
            crate::controllers::auth::RegisterParams,
            crate::controllers::auth::RefreshRequest,
            crate::controllers::auth::AcceptInviteParams,
            crate::controllers::auth::InviteAcceptResponse,
            crate::controllers::auth::RequestVerificationParams,
            crate::controllers::auth::ConfirmVerificationParams,
            crate::controllers::auth::VerificationRequestResponse,
            crate::controllers::auth::GenericStatusResponse,
            crate::controllers::auth::UserResponse,
            crate::controllers::auth::AuthResponse,
            crate::controllers::auth::UserInfo,
            crate::controllers::auth::LogoutResponse,

            // Common
            crate::common::PaginationMeta,
            crate::common::ApiError,

            // Blog
            rustok_blog::dto::CreatePostInput,
            rustok_blog::dto::UpdatePostInput,
            rustok_blog::dto::PostResponse,
            rustok_blog::dto::PostSummary,
            rustok_blog::dto::PostListQuery,
            rustok_blog::dto::PostListResponse,
            rustok_blog::state_machine::BlogPostStatus,

            // Forum
            rustok_forum::CreateCategoryInput,
            rustok_forum::UpdateCategoryInput,
            rustok_forum::CategoryResponse,
            rustok_forum::CategoryListItem,
            rustok_forum::CreateTopicInput,
            rustok_forum::UpdateTopicInput,
            rustok_forum::ListTopicsFilter,
            rustok_forum::TopicResponse,
            rustok_forum::TopicListItem,
            rustok_forum::CreateReplyInput,
            rustok_forum::UpdateReplyInput,
            rustok_forum::ListRepliesFilter,
            rustok_forum::ReplyResponse,
            rustok_forum::ReplyListItem,

            // Pages
            rustok_pages::CreatePageInput,
            rustok_pages::UpdatePageInput,
            rustok_pages::CreateBlockInput,
            rustok_pages::UpdateBlockInput,
            rustok_pages::BlockResponse,
            rustok_pages::PageResponse,
            crate::controllers::pages::GetPageParams,
            crate::controllers::pages::ReorderBlocksInput,

            // Commerce
            rustok_commerce::dto::CreateProductInput,
            rustok_commerce::dto::UpdateProductInput,
            rustok_commerce::dto::ProductResponse,
            rustok_commerce::dto::ProductTranslationInput,
            rustok_commerce::dto::ProductOptionInput,
            rustok_commerce::dto::ProductTranslationResponse,
            rustok_commerce::dto::ProductOptionResponse,
            rustok_commerce::dto::ProductImageResponse,
            rustok_commerce::dto::PriceResponse,
            rustok_commerce::entities::product::ProductStatus,
            crate::controllers::commerce::products::ListProductsParams,
            crate::controllers::commerce::products::ProductListItem,
            crate::controllers::commerce::store::StoreListProductsParams,
            crate::controllers::commerce::store::StoreContextQuery,
            crate::controllers::commerce::store::StoreCreateCartInput,
            crate::controllers::commerce::store::StoreCartResponse,
            crate::controllers::commerce::store::StoreUpdateCartInput,
            crate::controllers::commerce::store::StoreAddCartLineItemInput,
            crate::controllers::commerce::store::StoreUpdateCartLineItemInput,
            crate::controllers::commerce::store::StoreCreatePaymentCollectionInput,
            crate::controllers::commerce::store::StoreCompleteCartInput,
            rustok_commerce::dto::CartResponse,
            rustok_commerce::dto::CartLineItemResponse,
            rustok_commerce::dto::RegionResponse,
            rustok_commerce::dto::CustomerResponse,
            rustok_commerce::dto::ShippingOptionResponse,
            rustok_commerce::dto::PaymentCollectionResponse,
            rustok_commerce::dto::PaymentResponse,
            rustok_commerce::dto::OrderResponse,
            rustok_commerce::dto::OrderLineItemResponse,
            rustok_commerce::dto::MarkPaidOrderInput,
            rustok_commerce::dto::ShipOrderInput,
            rustok_commerce::dto::DeliverOrderInput,
            rustok_commerce::dto::CancelOrderInput,
            rustok_commerce::dto::AuthorizePaymentInput,
            rustok_commerce::dto::CapturePaymentInput,
            rustok_commerce::dto::CancelPaymentInput,
            crate::controllers::commerce::admin::ListPaymentCollectionsParams,
            rustok_commerce::dto::FulfillmentResponse,
            rustok_commerce::dto::ShipFulfillmentInput,
            rustok_commerce::dto::DeliverFulfillmentInput,
            rustok_commerce::dto::CancelFulfillmentInput,
            crate::controllers::commerce::admin::ListFulfillmentsParams,
            rustok_commerce::dto::ResolveStoreContextInput,
            rustok_commerce::dto::StoreContextResponse,
            rustok_commerce::dto::CompleteCheckoutInput,
            rustok_commerce::dto::CompleteCheckoutResponse,
            crate::controllers::commerce::admin::AdminOrderDetailResponse,

            // Marketplace
            crate::services::marketplace_catalog::RegistryCatalogResponse,
            crate::services::marketplace_catalog::RegistryCatalogModule,
            crate::services::marketplace_catalog::RegistryCatalogVersion,
            crate::services::marketplace_catalog::RegistryMutationResponse,
            crate::services::marketplace_catalog::RegistryPublishRequest,
            crate::services::marketplace_catalog::RegistryPublishDecisionRequest,
            crate::services::marketplace_catalog::RegistryPublishStatusResponse,
            crate::services::marketplace_catalog::RegistryPublishModuleRequest,
            crate::services::marketplace_catalog::RegistryPublishMarketplaceRequest,
            crate::services::marketplace_catalog::RegistryPublishUiPackagesRequest,
            crate::services::marketplace_catalog::RegistryPublishUiPackageRequest,
            crate::services::marketplace_catalog::RegistryYankRequest,
            crate::modules::ModuleSettingSpec,

            // Health
            crate::controllers::health::HealthResponse,
            crate::controllers::health::ModuleHealth,
            crate::controllers::health::ModulesHealthResponse,

            // Admin Events
            crate::controllers::admin_events::DlqEventItem,
            crate::controllers::admin_events::DlqListResponse,
            crate::controllers::admin_events::DlqReplayResponse,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "blog", description = "Blog endpoints"),
        (name = "forum", description = "Forum endpoints"),
        (name = "pages", description = "Pages endpoints"),
        (name = "commerce", description = "Ecommerce endpoints"),
        (name = "store", description = "Storefront ecommerce endpoints"),
        (name = "marketplace", description = "Marketplace registry and catalog endpoints"),
        (name = "health", description = "Health check endpoints"),
        (name = "observability", description = "Observability and metrics endpoints"),
        (name = "admin", description = "Admin operations")
    )
)]
pub struct ApiDoc;

const REGISTRY_ONLY_OPENAPI_PATHS: &[&str] = &[
    "/health",
    "/health/live",
    "/health/ready",
    "/health/runtime",
    "/health/modules",
    "/metrics",
    "/v1/catalog",
    "/v1/catalog/{slug}",
    "/api/openapi.json",
    "/api/openapi.yaml",
];

fn build_openapi_document(settings: &RustokSettings) -> OpenApiDoc {
    let mut openapi = ApiDoc::openapi();
    if settings.runtime.is_registry_only() {
        openapi
            .paths
            .paths
            .retain(|path, _| REGISTRY_ONLY_OPENAPI_PATHS.contains(&path.as_str()));
    }
    openapi
}

/// GET /api/openapi.json — OpenAPI specification in JSON format
#[utoipa::path(
    get,
    path = "/api/openapi.json",
    tag = "observability",
    responses(
        (status = 200, description = "OpenAPI specification in JSON format", content_type = "application/json"),
    )
)]
pub async fn openapi_json(State(ctx): State<AppContext>) -> Result<Response> {
    let settings = RustokSettings::from_settings(&ctx.config.settings).unwrap_or_default();
    let spec = build_openapi_document(&settings)
        .to_json()
        .map_err(|e| Error::Message(format!("Failed to serialize OpenAPI spec: {e}")))?;
    Ok((
        StatusCode::OK,
        [(CONTENT_TYPE, "application/json; charset=utf-8")],
        spec,
    )
        .into_response())
}

/// GET /api/openapi.yaml — OpenAPI specification in YAML format
#[utoipa::path(
    get,
    path = "/api/openapi.yaml",
    tag = "observability",
    responses(
        (status = 200, description = "OpenAPI specification in YAML format", content_type = "text/yaml"),
    )
)]
pub async fn openapi_yaml(State(ctx): State<AppContext>) -> Result<Response> {
    let settings = RustokSettings::from_settings(&ctx.config.settings).unwrap_or_default();
    let spec = build_openapi_document(&settings)
        .to_yaml()
        .map_err(|e| Error::Message(format!("Failed to serialize OpenAPI spec to YAML: {e}")))?;
    Ok((
        StatusCode::OK,
        [(CONTENT_TYPE, "text/yaml; charset=utf-8")],
        spec,
    )
        .into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/api/openapi.json", get(openapi_json))
        .add("/api/openapi.yaml", get(openapi_yaml))
}

pub struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
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

        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{build_openapi_document, ApiDoc};
    use crate::common::settings::{RuntimeHostMode, RustokSettings};
    use utoipa::OpenApi;

    #[test]
    fn openapi_includes_registry_catalog_path() {
        let openapi = ApiDoc::openapi();

        assert!(
            openapi.paths.paths.contains_key("/v1/catalog"),
            "OpenAPI spec must include /v1/catalog"
        );
        assert!(
            openapi.paths.paths.contains_key("/v1/catalog/{slug}"),
            "OpenAPI spec must include /v1/catalog/{slug}"
        );
        assert!(
            openapi.paths.paths.contains_key("/v2/catalog/publish"),
            "OpenAPI spec must include /v2/catalog/publish"
        );
        assert!(
            openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}"),
            "OpenAPI spec must include /v2/catalog/publish/{request_id}"
        );
        assert!(
            openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/artifact"),
            "OpenAPI spec must include /v2/catalog/publish/{request_id}/artifact"
        );
        assert!(
            openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/approve"),
            "OpenAPI spec must include /v2/catalog/publish/{request_id}/approve"
        );
        assert!(
            openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/reject"),
            "OpenAPI spec must include /v2/catalog/publish/{request_id}/reject"
        );
        assert!(
            openapi.paths.paths.contains_key("/v2/catalog/yank"),
            "OpenAPI spec must include /v2/catalog/yank"
        );
    }

    #[test]
    fn registry_only_openapi_filters_non_registry_surface() {
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::RegistryOnly;

        let openapi = build_openapi_document(&settings);

        assert!(openapi.paths.paths.contains_key("/v1/catalog"));
        assert!(openapi.paths.paths.contains_key("/metrics"));
        assert!(openapi.paths.paths.contains_key("/api/openapi.json"));
        assert!(!openapi.paths.paths.contains_key("/v2/catalog/publish"));
        assert!(!openapi
            .paths
            .paths
            .contains_key("/v2/catalog/publish/{request_id}"));
        assert!(!openapi
            .paths
            .paths
            .contains_key("/v2/catalog/publish/{request_id}/artifact"));
        assert!(!openapi
            .paths
            .paths
            .contains_key("/v2/catalog/publish/{request_id}/approve"));
        assert!(!openapi
            .paths
            .paths
            .contains_key("/v2/catalog/publish/{request_id}/reject"));
        assert!(!openapi.paths.paths.contains_key("/api/auth/login"));
        assert!(!openapi.paths.paths.contains_key("/api/admin/events/dlq"));
    }
}
