#![allow(dead_code)]

#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use rustok_ui_core::normalize_ui_text as optional_text;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{ProductDetail, ProductDraft};

const PRODUCT_ADMIN_GRAPHQL_OWNER: &str = "rustok_product.admin";
const PRODUCT_ADMIN_MUTATION_GRAPHQL_BOUNDARY: &str =
    "product_admin_primary_graphql_mutations";
const PRODUCT_ADMIN_HTTP_PUBLIC_MESSAGE: &str =
    "Product admin service is temporarily unavailable";
const PRODUCT_ADMIN_GRAPHQL_PUBLIC_MESSAGE: &str =
    "Product admin request could not be completed";

const CREATE_PRODUCT_MUTATION: &str = "mutation ProductAdminCreateProduct($idempotencyKey: String!, $input: CreateProductInput!) { createProduct(idempotencyKey: $idempotencyKey, input: $input) { id status sellerId vendor productType shippingProfileSlug primaryCategoryId tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } options { id name values position } } }";
const UPDATE_PRODUCT_MUTATION: &str = "mutation ProductAdminUpdateProduct($idempotencyKey: String!, $id: UUID!, $input: UpdateProductInput!) { updateProduct(idempotencyKey: $idempotencyKey, id: $id, input: $input) { id status sellerId vendor productType shippingProfileSlug primaryCategoryId tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } options { id name values position } } }";
const DELETE_PRODUCT_MUTATION: &str = "mutation ProductAdminDeleteProduct($idempotencyKey: String!, $id: UUID!) { deleteProduct(idempotencyKey: $idempotencyKey, id: $id) }";

#[derive(Debug, Deserialize)]
struct CreateProductResponse {
    #[serde(rename = "createProduct")]
    create_product: ProductDetail,
}

#[derive(Debug, Deserialize)]
struct UpdateProductResponse {
    #[serde(rename = "updateProduct")]
    update_product: ProductDetail,
}

#[derive(Debug, Deserialize)]
struct DeleteProductResponse {
    #[serde(rename = "deleteProduct")]
    delete_product: bool,
}

#[derive(Debug, Serialize)]
struct CreateProductVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    input: CreateProductInput,
}

#[derive(Debug, Serialize)]
struct UpdateProductVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    id: String,
    input: UpdateProductInput,
}

#[derive(Debug, Serialize)]
struct DeleteProductVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    id: String,
}

#[derive(Debug, Serialize)]
struct CreateProductInput {
    translations: Vec<ProductTranslationInput>,
    options: Vec<ProductOptionInput>,
    variants: Vec<CreateVariantInput>,
    #[serde(rename = "sellerId")]
    seller_id: Option<String>,
    vendor: Option<String>,
    #[serde(rename = "productType")]
    product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: Option<String>,
    #[serde(rename = "primaryCategoryId")]
    primary_category_id: Option<String>,
    publish: Option<bool>,
}

#[derive(Debug, Serialize)]
struct UpdateProductInput {
    translations: Option<Vec<ProductTranslationInput>>,
    #[serde(rename = "sellerId")]
    seller_id: Option<String>,
    vendor: Option<String>,
    #[serde(rename = "productType")]
    product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: Option<String>,
    #[serde(rename = "primaryCategoryId")]
    primary_category_id: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProductTranslationInput {
    locale: String,
    title: String,
    handle: Option<String>,
    description: Option<String>,
    #[serde(rename = "metaTitle")]
    meta_title: Option<String>,
    #[serde(rename = "metaDescription")]
    meta_description: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProductOptionInput {
    translations: Vec<ProductOptionTranslationInput>,
}

#[derive(Debug, Serialize)]
struct ProductOptionTranslationInput {
    locale: String,
    name: String,
    values: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CreateVariantInput {
    sku: Option<String>,
    barcode: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: Option<String>,
    option1: Option<String>,
    option2: Option<String>,
    option3: Option<String>,
    prices: Vec<PriceInput>,
    #[serde(rename = "inventoryQuantity")]
    inventory_quantity: Option<i32>,
    #[serde(rename = "inventoryPolicy")]
    inventory_policy: Option<String>,
}

#[derive(Debug, Serialize)]
struct PriceInput {
    #[serde(rename = "currencyCode")]
    currency_code: String,
    amount: String,
    #[serde(rename = "compareAtAmount")]
    compare_at_amount: Option<String>,
}

struct MutationErrorContext {
    operation: &'static str,
    correlation_id: String,
    token_present: bool,
    tenant_slug_length: Option<usize>,
    tenant_id_length: usize,
    actor_id_length: usize,
    resource_id_length: Option<usize>,
    status_length: Option<usize>,
    draft_present: bool,
}

impl MutationErrorContext {
    fn new(
        operation: &'static str,
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self {
            operation,
            correlation_id: format!("product-admin-mutation:{operation}:{}", Uuid::new_v4()),
            token_present: token.is_some(),
            tenant_slug_length: tenant_slug.map(str::chars).map(Iterator::count),
            tenant_id_length: tenant_id.chars().count(),
            actor_id_length: actor_id.chars().count(),
            resource_id_length: None,
            status_length: None,
            draft_present: false,
        }
    }

    fn with_resource(mut self, resource_id: &str) -> Self {
        self.resource_id_length = Some(resource_id.chars().count());
        self
    }

    fn with_status(mut self, status: &str) -> Self {
        self.status_length = Some(status.chars().count());
        self
    }

    fn with_draft(mut self) -> Self {
        self.draft_present = true;
        self
    }

    fn map_error(&self, error: GraphqlHttpError) -> GraphqlHttpError {
        let (error_kind, code, public_error, technical_failure) = match &error {
            GraphqlHttpError::Network => (
                "network",
                "product.admin_graphql_network_unavailable",
                GraphqlHttpError::Network,
                true,
            ),
            GraphqlHttpError::Http(_) => (
                "http",
                "product.admin_graphql_http_unavailable",
                GraphqlHttpError::Http(PRODUCT_ADMIN_HTTP_PUBLIC_MESSAGE.to_string()),
                true,
            ),
            GraphqlHttpError::Unauthorized => (
                "unauthorized",
                "product.admin_graphql_authentication_required",
                GraphqlHttpError::Unauthorized,
                false,
            ),
            GraphqlHttpError::Graphql(_) => (
                "graphql",
                "product.admin_graphql_request_rejected",
                GraphqlHttpError::Graphql(PRODUCT_ADMIN_GRAPHQL_PUBLIC_MESSAGE.to_string()),
                false,
            ),
        };
        let error_payload_length = match &error {
            GraphqlHttpError::Http(value) | GraphqlHttpError::Graphql(value) => {
                Some(value.chars().count())
            }
            GraphqlHttpError::Network | GraphqlHttpError::Unauthorized => None,
        };
        let error_payload_present = error_payload_length.is_some_and(|length| length > 0);

        if technical_failure {
            tracing::error!(
                error_payload_present,
                error_payload_length = ?error_payload_length,
                owner = PRODUCT_ADMIN_GRAPHQL_OWNER,
                owner_operation = self.operation,
                correlation_id = %self.correlation_id,
                token_present = self.token_present,
                tenant_slug_present = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                tenant_id_length = self.tenant_id_length,
                actor_id_length = self.actor_id_length,
                resource_id_present = self.resource_id_length.is_some(),
                resource_id_length = ?self.resource_id_length,
                status_present = self.status_length.is_some(),
                status_length = ?self.status_length,
                draft_present = self.draft_present,
                error_kind,
                code,
                boundary = PRODUCT_ADMIN_MUTATION_GRAPHQL_BOUNDARY,
                "product admin GraphQL lifecycle mutation failed"
            );
        } else {
            tracing::warn!(
                error_payload_present,
                error_payload_length = ?error_payload_length,
                owner = PRODUCT_ADMIN_GRAPHQL_OWNER,
                owner_operation = self.operation,
                correlation_id = %self.correlation_id,
                token_present = self.token_present,
                tenant_slug_present = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                tenant_id_length = self.tenant_id_length,
                actor_id_length = self.actor_id_length,
                resource_id_present = self.resource_id_length.is_some(),
                resource_id_length = ?self.resource_id_length,
                status_present = self.status_length.is_some(),
                status_length = ?self.status_length,
                draft_present = self.draft_present,
                error_kind,
                code,
                boundary = PRODUCT_ADMIN_MUTATION_GRAPHQL_BOUNDARY,
                "product admin GraphQL lifecycle mutation was rejected"
            );
        }

        public_error
    }
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, GraphqlHttpError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        None,
    )
    .await
}

pub(crate) async fn create_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    actor_id: String,
    idempotency_key: String,
    draft: ProductDraft,
) -> Result<ProductDetail, GraphqlHttpError> {
    let context = MutationErrorContext::new(
        "create_product",
        token.as_deref(),
        tenant_slug.as_deref(),
        &tenant_id,
        &actor_id,
    )
    .with_draft();
    let response: CreateProductResponse = request(
        CREATE_PRODUCT_MUTATION,
        CreateProductVariables {
            idempotency_key,
            input: build_create_product_input(draft),
        },
        token,
        tenant_slug,
    )
    .await
    .map_err(|error| context.map_error(error))?;
    Ok(response.create_product)
}

pub(crate) async fn update_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    actor_id: String,
    id: String,
    idempotency_key: String,
    draft: ProductDraft,
) -> Result<ProductDetail, GraphqlHttpError> {
    let context = MutationErrorContext::new(
        "update_product",
        token.as_deref(),
        tenant_slug.as_deref(),
        &tenant_id,
        &actor_id,
    )
    .with_resource(&id)
    .with_draft();
    let response: UpdateProductResponse = request(
        UPDATE_PRODUCT_MUTATION,
        UpdateProductVariables {
            idempotency_key,
            id,
            input: UpdateProductInput {
                translations: Some(vec![build_translation_input(&draft)]),
                seller_id: optional_text(draft.seller_id.as_str()),
                vendor: optional_text(draft.vendor.as_str()),
                product_type: optional_text(draft.product_type.as_str()),
                shipping_profile_slug: draft.shipping_profile_slug.clone(),
                primary_category_id: draft.primary_category_id.clone(),
                status: None,
            },
        },
        token,
        tenant_slug,
    )
    .await
    .map_err(|error| context.map_error(error))?;
    Ok(response.update_product)
}

pub(crate) async fn change_product_status(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    actor_id: String,
    id: String,
    status: &str,
    idempotency_key: String,
) -> Result<ProductDetail, GraphqlHttpError> {
    let context = MutationErrorContext::new(
        "change_product_status",
        token.as_deref(),
        tenant_slug.as_deref(),
        &tenant_id,
        &actor_id,
    )
    .with_resource(&id)
    .with_status(status);
    let response: UpdateProductResponse = request(
        UPDATE_PRODUCT_MUTATION,
        UpdateProductVariables {
            idempotency_key,
            id,
            input: UpdateProductInput {
                translations: None,
                seller_id: None,
                vendor: None,
                product_type: None,
                shipping_profile_slug: None,
                primary_category_id: None,
                status: Some(status.to_string()),
            },
        },
        token,
        tenant_slug,
    )
    .await
    .map_err(|error| context.map_error(error))?;
    Ok(response.update_product)
}

pub(crate) async fn delete_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    actor_id: String,
    id: String,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let context = MutationErrorContext::new(
        "delete_product",
        token.as_deref(),
        tenant_slug.as_deref(),
        &tenant_id,
        &actor_id,
    )
    .with_resource(&id);
    let response: DeleteProductResponse = request(
        DELETE_PRODUCT_MUTATION,
        DeleteProductVariables {
            idempotency_key,
            id,
        },
        token,
        tenant_slug,
    )
    .await
    .map_err(|error| context.map_error(error))?;
    Ok(response.delete_product)
}

fn build_create_product_input(draft: ProductDraft) -> CreateProductInput {
    CreateProductInput {
        translations: vec![build_translation_input(&draft)],
        options: Vec::new(),
        variants: vec![CreateVariantInput {
            sku: optional_text(draft.sku.as_str()),
            barcode: optional_text(draft.barcode.as_str()),
            shipping_profile_slug: None,
            option1: None,
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: if draft.currency_code.trim().is_empty() {
                    "USD".to_string()
                } else {
                    draft.currency_code.trim().to_uppercase()
                },
                amount: if draft.amount.trim().is_empty() {
                    "0.00".to_string()
                } else {
                    draft.amount.trim().to_string()
                },
                compare_at_amount: optional_text(draft.compare_at_amount.as_str()),
            }],
            inventory_quantity: Some(draft.inventory_quantity),
            inventory_policy: Some("deny".to_string()),
        }],
        seller_id: optional_text(draft.seller_id.as_str()),
        vendor: optional_text(draft.vendor.as_str()),
        product_type: optional_text(draft.product_type.as_str()),
        shipping_profile_slug: draft.shipping_profile_slug,
        primary_category_id: draft.primary_category_id,
        publish: Some(draft.publish_now),
    }
}

fn build_translation_input(draft: &ProductDraft) -> ProductTranslationInput {
    ProductTranslationInput {
        locale: draft.locale.clone(),
        title: draft.title.trim().to_string(),
        handle: optional_text(draft.handle.as_str()),
        description: optional_text(draft.description.as_str()),
        meta_title: None,
        meta_description: None,
    }
}
