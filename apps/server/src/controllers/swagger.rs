use axum::{
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use loco_rs::{controller::Routes, Result};

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
        // Content
        crate::controllers::content::nodes::list_nodes,
        crate::controllers::content::nodes::get_node,
        crate::controllers::content::nodes::create_node,
        crate::controllers::content::nodes::update_node,
        crate::controllers::content::nodes::delete_node,
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
        crate::controllers::commerce::products::list_products,
        crate::controllers::commerce::products::create_product,
        crate::controllers::commerce::products::show_product,
        crate::controllers::commerce::products::update_product,
        crate::controllers::commerce::products::delete_product,
        crate::controllers::commerce::products::publish_product,
        crate::controllers::commerce::products::unpublish_product,
        crate::controllers::commerce::variants::list_variants,
        crate::controllers::commerce::variants::create_variant,
        crate::controllers::commerce::variants::show_variant,
        crate::controllers::commerce::variants::update_variant,
        crate::controllers::commerce::variants::delete_variant,
        crate::controllers::commerce::variants::update_prices,
        crate::controllers::commerce::inventory::get_inventory,
        crate::controllers::commerce::inventory::adjust_inventory,
        crate::controllers::commerce::inventory::set_inventory,
        crate::controllers::commerce::inventory::check_availability,
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
        // Health
        crate::controllers::health::health,
        crate::controllers::health::live,
        crate::controllers::health::ready,
        crate::controllers::health::modules,
        // Metrics
        crate::controllers::metrics::metrics,
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

            // Content
            rustok_content::dto::NodeListItem,
            rustok_content::dto::NodeResponse,
            rustok_content::dto::CreateNodeInput,
            rustok_content::dto::UpdateNodeInput,
            rustok_content::dto::NodeTranslationInput,
            rustok_content::dto::BodyInput,
            rustok_content::dto::ListNodesFilter,
            rustok_content::dto::NodeTranslationResponse,
            rustok_content::dto::BodyResponse,
            rustok_content::entities::node::ContentStatus,

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
            rustok_commerce::dto::CreateVariantInput,
            rustok_commerce::dto::UpdateVariantInput,
            rustok_commerce::dto::VariantResponse,
            rustok_commerce::dto::PriceInput,
            rustok_commerce::dto::AdjustInventoryInput,
            rustok_commerce::entities::product::ProductStatus,
            crate::controllers::commerce::products::ListProductsParams,
            crate::controllers::commerce::products::ProductListItem,
            crate::controllers::commerce::inventory::InventoryResponse,
            crate::controllers::commerce::inventory::AdjustInput,
            crate::controllers::commerce::inventory::SetInventoryInput,
            crate::controllers::commerce::inventory::CheckAvailabilityInput,
            crate::controllers::commerce::inventory::CheckItem,
            crate::controllers::commerce::inventory::AvailabilityResult,
            crate::controllers::commerce::store::StoreListProductsParams,
            crate::controllers::commerce::store::StoreContextQuery,
            crate::controllers::commerce::store::StoreCreateCartInput,
            crate::controllers::commerce::store::StoreCartResponse,
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
            rustok_commerce::dto::ResolveStoreContextInput,
            rustok_commerce::dto::StoreContextResponse,
            rustok_commerce::dto::CompleteCheckoutInput,
            rustok_commerce::dto::CompleteCheckoutResponse,

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
        (name = "content", description = "Content Management endpoints"),
        (name = "blog", description = "Blog endpoints"),
        (name = "forum", description = "Forum endpoints"),
        (name = "pages", description = "Pages endpoints"),
        (name = "commerce", description = "Ecommerce endpoints"),
        (name = "store", description = "Storefront ecommerce endpoints"),
        (name = "health", description = "Health check endpoints"),
        (name = "observability", description = "Observability and metrics endpoints"),
        (name = "admin", description = "Admin operations")
    )
)]
pub struct ApiDoc;

/// GET /api/openapi.json — OpenAPI specification in JSON format
#[utoipa::path(
    get,
    path = "/api/openapi.json",
    tag = "observability",
    responses(
        (status = 200, description = "OpenAPI specification in JSON format", content_type = "application/json"),
    )
)]
pub async fn openapi_json() -> Result<Response> {
    let spec = ApiDoc::openapi()
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
pub async fn openapi_yaml() -> Result<Response> {
    let spec = ApiDoc::openapi()
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
