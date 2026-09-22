#![allow(dead_code)]

use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql, graphql_url};
use rustok_ui_core::normalize_ui_text as optional_text;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{ProductDetail, ProductDraft};

const PRODUCT_ADMIN_GRAPHQL_OWNER: &str = "rustok_product.admin";
const PRODUCT_ADMIN_MUTATION_GRAPHQL_BOUNDARY: &str = "product_admin_primary_graphql_mutations";
const PRODUCT_ADMIN_HTTP_PUBLIC_MESSAGE: &str = "Product admin service is temporarily unavailable";
const PRODUCT_ADMIN_GRAPHQL_PUBLIC_MESSAGE: &str = "Product admin request could not be completed";

const CREATE_PRODUCT_MUTATION: &str = "mutation ProductAdminCreateProduct($idempotencyKey: String!, $input: CreateProductInput!) { createProduct(idempotencyKey: $idempotencyKey, input: $input) { id status sellerId vendor productType shippingProfileSlug primaryCategoryId tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } variants { id sku barcode shippingProfileSlug title combinationIdentity axisValues { attributeId optionId code label } inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } variantAxes { id attributeId code name position allowedValues { optionId value position } } images { id mediaId url altText position } } }";
const UPDATE_PRODUCT_MUTATION: &str = "mutation ProductAdminUpdateProduct($idempotencyKey: String!, $id: UUID!, $input: UpdateProductInput!) { updateProduct(idempotencyKey: $idempotencyKey, id: $id, input: $input) { id status sellerId vendor productType shippingProfileSlug primaryCategoryId tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } variants { id sku barcode shippingProfileSlug title combinationIdentity axisValues { attributeId optionId code label } inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } variantAxes { id attributeId code name position allowedValues { optionId value position } } images { id mediaId url altText position } } }";
const DELETE_PRODUCT_MUTATION: &str = "mutation ProductAdminDeleteProduct($idempotencyKey: String!, $id: UUID!) { deleteProduct(idempotencyKey: $idempotencyKey, id: $id) }";
const CREATE_VARIANT_MUTATION: &str = "mutation ProductAdminCreateVariant($idempotencyKey: String!, $productId: UUID!, $input: CreateVariantInput!) { createProductVariant(idempotencyKey: $idempotencyKey, productId: $productId, input: $input) { id sku barcode shippingProfileSlug title combinationIdentity axisValues { attributeId optionId code label } inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } }";
const UPDATE_VARIANT_MUTATION: &str = "mutation ProductAdminUpdateVariant($idempotencyKey: String!, $id: UUID!, $input: UpdateVariantInput!) { updateProductVariant(idempotencyKey: $idempotencyKey, id: $id, input: $input) { id sku barcode shippingProfileSlug title combinationIdentity axisValues { attributeId optionId code label } inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } }";
const DELETE_VARIANT_MUTATION: &str = "mutation ProductAdminDeleteVariant($idempotencyKey: String!, $id: UUID!) { deleteProductVariant(idempotencyKey: $idempotencyKey, id: $id) }";
const ADD_PRODUCT_IMAGE_MUTATION: &str = "mutation ProductAdminAddImage($idempotencyKey: String!, $productId: UUID!, $input: AddProductImageInput!) { addProductImage(idempotencyKey: $idempotencyKey, productId: $productId, input: $input) { id mediaId url altText position } }";
const UPDATE_PRODUCT_IMAGE_MUTATION: &str = "mutation ProductAdminUpdateImage($idempotencyKey: String!, $productId: UUID!, $id: UUID!, $input: UpdateProductImageInput!) { updateProductImage(idempotencyKey: $idempotencyKey, productId: $productId, id: $id, input: $input) { id mediaId url altText position } }";
const DELETE_PRODUCT_IMAGE_MUTATION: &str = "mutation ProductAdminDeleteImage($idempotencyKey: String!, $productId: UUID!, $id: UUID!) { deleteProductImage(idempotencyKey: $idempotencyKey, productId: $productId, id: $id) }";
const REORDER_PRODUCT_IMAGES_MUTATION: &str = "mutation ProductAdminReorderImages($idempotencyKey: String!, $productId: UUID!, $imageIds: [UUID!]!) { reorderProductImages(idempotencyKey: $idempotencyKey, productId: $productId, imageIds: $imageIds) }";

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

#[derive(Debug, Deserialize)]
struct CreateVariantResponse {
    #[serde(rename = "createProductVariant")]
    create_product_variant: crate::model::ProductVariant,
}

#[derive(Debug, Deserialize)]
struct UpdateVariantResponse {
    #[serde(rename = "updateProductVariant")]
    update_product_variant: crate::model::ProductVariant,
}

#[derive(Debug, Deserialize)]
struct DeleteVariantResponse {
    #[serde(rename = "deleteProductVariant")]
    delete_product_variant: bool,
}

#[derive(Debug, Deserialize)]
struct AddProductImageResponse {
    #[serde(rename = "addProductImage")]
    add_product_image: crate::model::ProductImage,
}

#[derive(Debug, Deserialize)]
struct UpdateProductImageResponse {
    #[serde(rename = "updateProductImage")]
    update_product_image: crate::model::ProductImage,
}

#[derive(Debug, Deserialize)]
struct DeleteProductImageResponse {
    #[serde(rename = "deleteProductImage")]
    delete_product_image: bool,
}

#[derive(Debug, Deserialize)]
struct ReorderProductImagesResponse {
    #[serde(rename = "reorderProductImages")]
    reorder_product_images: bool,
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
struct CreateVariantVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "productId")]
    product_id: String,
    input: CreateVariantInput,
}

#[derive(Debug, Serialize)]
struct UpdateVariantVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    id: String,
    input: UpdateVariantInput,
}

#[derive(Debug, Serialize)]
struct DeleteVariantVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    id: String,
}

#[derive(Debug, Serialize)]
struct AddProductImageVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "productId")]
    product_id: String,
    input: AddProductImageInput,
}

#[derive(Debug, Serialize)]
struct UpdateProductImageVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "productId")]
    product_id: String,
    id: String,
    input: UpdateProductImageInput,
}

#[derive(Debug, Serialize)]
struct DeleteProductImageVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "productId")]
    product_id: String,
    id: String,
}

#[derive(Debug, Serialize)]
struct ReorderProductImagesVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "productId")]
    product_id: String,
    #[serde(rename = "imageIds")]
    image_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct VariantAxisValueInput {
    #[serde(rename = "attributeId")]
    attribute_id: String,
    #[serde(rename = "optionId")]
    option_id: String,
}

#[derive(Debug, Serialize)]
struct VariantAxisInput {
    #[serde(rename = "attributeId")]
    attribute_id: String,
    position: Option<i32>,
    #[serde(rename = "allowedOptionIds", skip_serializing_if = "Option::is_none")]
    allowed_option_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct UpdateVariantInput {
    sku: Option<String>,
    barcode: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: Option<String>,
    #[serde(rename = "axisValues", skip_serializing_if = "Option::is_none")]
    axis_values: Option<Vec<VariantAxisValueInput>>,
    prices: Option<Vec<PriceInput>>,
    #[serde(rename = "inventoryQuantity")]
    inventory_quantity: Option<i32>,
    #[serde(rename = "inventoryPolicy")]
    inventory_policy: Option<String>,
}

#[derive(Debug, Serialize)]
struct AddProductImageInput {
    #[serde(rename = "mediaId")]
    media_id: String,
    position: Option<i32>,
    #[serde(rename = "altText")]
    alt_text: Option<String>,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdateProductImageInput {
    position: Option<i32>,
    #[serde(rename = "altText")]
    alt_text: Option<String>,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateProductInput {
    translations: Vec<ProductTranslationInput>,
    #[serde(rename = "variantAxes", skip_serializing_if = "Option::is_none")]
    variant_axes: Option<Vec<VariantAxisInput>>,
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
struct CreateVariantInput {
    sku: Option<String>,
    barcode: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: Option<String>,
    #[serde(rename = "axisValues", skip_serializing_if = "Option::is_none")]
    axis_values: Option<Vec<VariantAxisValueInput>>,
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

    fn with_product(mut self, product_id: &str) -> Self {
        self.resource_id_length = Some(product_id.chars().count());
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

pub(crate) async fn create_product_variant(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    actor_id: String,
    product_id: String,
    idempotency_key: String,
    draft: crate::model::VariantDraft,
) -> Result<crate::model::ProductVariant, GraphqlHttpError> {
    let context = MutationErrorContext::new(
        "create_product_variant",
        token.as_deref(),
        tenant_slug.as_deref(),
        &tenant_id,
        &actor_id,
    )
    .with_product(&product_id);

    let input = CreateVariantInput {
        sku: draft.sku.as_deref().and_then(optional_text),
        barcode: draft.barcode.as_deref().and_then(optional_text),
        shipping_profile_slug: draft.shipping_profile_slug.as_deref().and_then(optional_text),
        axis_values: if draft.axis_values.is_empty() {
            None
        } else {
            Some(
                draft
                    .axis_values
                    .into_iter()
                    .map(|av| VariantAxisValueInput {
                        attribute_id: av.attribute_id,
                        option_id: av.option_id,
                    })
                    .collect(),
            )
        },
        prices: draft
            .prices
            .into_iter()
            .map(|p| PriceInput {
                currency_code: p.currency_code,
                amount: p.amount,
                compare_at_amount: p.compare_at_amount.as_deref().and_then(optional_text),
            })
            .collect(),
        inventory_quantity: draft.inventory_quantity,
        inventory_policy: draft.inventory_policy.as_deref().and_then(optional_text),
    };

    let response: CreateVariantResponse = request(
        CREATE_VARIANT_MUTATION,
        CreateVariantVariables {
            idempotency_key,
            product_id,
            input,
        },
        token,
        tenant_slug,
    )
    .await
    .map_err(|error| context.map_error(error))?;

    Ok(response.create_product_variant)
}

pub(crate) async fn update_product_variant(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    actor_id: String,
    variant_id: String,
    idempotency_key: String,
    draft: crate::model::VariantDraft,
) -> Result<crate::model::ProductVariant, GraphqlHttpError> {
    let context = MutationErrorContext::new(
        "update_product_variant",
        token.as_deref(),
        tenant_slug.as_deref(),
        &tenant_id,
        &actor_id,
    );

    let prices = if draft.prices.is_empty() {
        None
    } else {
        Some(
            draft
                .prices
                .into_iter()
                .map(|p| PriceInput {
                    currency_code: p.currency_code,
                    amount: p.amount,
                    compare_at_amount: p.compare_at_amount.as_deref().and_then(optional_text),
                })
                .collect(),
        )
    };

    let input = UpdateVariantInput {
        sku: draft.sku.as_deref().and_then(optional_text),
        barcode: draft.barcode.as_deref().and_then(optional_text),
        shipping_profile_slug: draft.shipping_profile_slug.as_deref().and_then(optional_text),
        axis_values: if draft.axis_values.is_empty() {
            None
        } else {
            Some(
                draft
                    .axis_values
                    .into_iter()
                    .map(|av| VariantAxisValueInput {
                        attribute_id: av.attribute_id,
                        option_id: av.option_id,
                    })
                    .collect(),
            )
        },
        prices,
        inventory_quantity: draft.inventory_quantity,
        inventory_policy: draft.inventory_policy.as_deref().and_then(optional_text),
    };

    let response: UpdateVariantResponse = request(
        UPDATE_VARIANT_MUTATION,
        UpdateVariantVariables {
            idempotency_key,
            id: variant_id,
            input,
        },
        token,
        tenant_slug,
    )
    .await
    .map_err(|error| context.map_error(error))?;

    Ok(response.update_product_variant)
}

pub(crate) async fn delete_product_variant(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    actor_id: String,
    variant_id: String,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let context = MutationErrorContext::new(
        "delete_product_variant",
        token.as_deref(),
        tenant_slug.as_deref(),
        &tenant_id,
        &actor_id,
    );

    let response: DeleteVariantResponse = request(
        DELETE_VARIANT_MUTATION,
        DeleteVariantVariables {
            idempotency_key,
            id: variant_id,
        },
        token,
        tenant_slug,
    )
    .await
    .map_err(|error| context.map_error(error))?;

    Ok(response.delete_product_variant)
}

pub(crate) async fn add_product_image(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    actor_id: String,
    product_id: String,
    idempotency_key: String,
    draft: crate::model::ProductImageDraft,
) -> Result<crate::model::ProductImage, GraphqlHttpError> {
    let context = MutationErrorContext::new(
        "add_product_image",
        token.as_deref(),
        tenant_slug.as_deref(),
        &tenant_id,
        &actor_id,
    )
    .with_product(&product_id);

    let input = AddProductImageInput {
        media_id: draft.media_id,
        position: draft.position,
        alt_text: draft.alt_text.as_deref().and_then(optional_text),
        locale: draft.locale.as_deref().and_then(optional_text),
    };

    let response: AddProductImageResponse = request(
        ADD_PRODUCT_IMAGE_MUTATION,
        AddProductImageVariables {
            idempotency_key,
            product_id,
            input,
        },
        token,
        tenant_slug,
    )
    .await
    .map_err(|error| context.map_error(error))?;

    Ok(response.add_product_image)
}

pub(crate) async fn update_product_image(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    actor_id: String,
    product_id: String,
    image_id: String,
    idempotency_key: String,
    draft: crate::model::UpdateProductImageDraft,
) -> Result<crate::model::ProductImage, GraphqlHttpError> {
    let context = MutationErrorContext::new(
        "update_product_image",
        token.as_deref(),
        tenant_slug.as_deref(),
        &tenant_id,
        &actor_id,
    )
    .with_product(&product_id);

    let input = UpdateProductImageInput {
        position: draft.position,
        alt_text: draft.alt_text.as_deref().and_then(optional_text),
        locale: draft.locale.as_deref().and_then(optional_text),
    };

    let response: UpdateProductImageResponse = request(
        UPDATE_PRODUCT_IMAGE_MUTATION,
        UpdateProductImageVariables {
            idempotency_key,
            product_id,
            id: image_id,
            input,
        },
        token,
        tenant_slug,
    )
    .await
    .map_err(|error| context.map_error(error))?;

    Ok(response.update_product_image)
}

pub(crate) async fn delete_product_image(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    actor_id: String,
    product_id: String,
    image_id: String,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let context = MutationErrorContext::new(
        "delete_product_image",
        token.as_deref(),
        tenant_slug.as_deref(),
        &tenant_id,
        &actor_id,
    )
    .with_product(&product_id);

    let response: DeleteProductImageResponse = request(
        DELETE_PRODUCT_IMAGE_MUTATION,
        DeleteProductImageVariables {
            idempotency_key,
            product_id,
            id: image_id,
        },
        token,
        tenant_slug,
    )
    .await
    .map_err(|error| context.map_error(error))?;

    Ok(response.delete_product_image)
}

pub(crate) async fn reorder_product_images(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    actor_id: String,
    product_id: String,
    image_ids: Vec<String>,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let context = MutationErrorContext::new(
        "reorder_product_images",
        token.as_deref(),
        tenant_slug.as_deref(),
        &tenant_id,
        &actor_id,
    )
    .with_product(&product_id);

    let response: ReorderProductImagesResponse = request(
        REORDER_PRODUCT_IMAGES_MUTATION,
        ReorderProductImagesVariables {
            idempotency_key,
            product_id,
            image_ids,
        },
        token,
        tenant_slug,
    )
    .await
    .map_err(|error| context.map_error(error))?;

    Ok(response.reorder_product_images)
}

fn build_create_product_input(draft: ProductDraft) -> CreateProductInput {
    CreateProductInput {
        translations: vec![build_translation_input(&draft)],
        variant_axes: None,
        variants: vec![CreateVariantInput {
            sku: optional_text(draft.sku.as_str()),
            barcode: optional_text(draft.barcode.as_str()),
            shipping_profile_slug: None,
            axis_values: None,
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
