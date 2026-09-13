use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use rustok_graphql::GraphqlHttpError;

use crate::lifecycle_retry_identity::{
    ProductAdminLifecycleOperation, ProductAdminLifecycleRetryIdentity,
};
use crate::model::{ProductDetail, ProductDraft};

pub use crate::legacy_transport::fetch_catalog_search_options;
pub(crate) use crate::legacy_transport::{
    fetch_bootstrap, fetch_catalog_categories, fetch_effective_product_form, fetch_product,
    fetch_product_attribute_values, fetch_product_pricing, fetch_products, fetch_shipping_profiles,
};
pub(crate) use crate::product_schema_graphql::{
    clear_detached_product_attribute_values, save_product_attribute_values,
};
pub(crate) use crate::product_lifecycle_graphql::{
    add_product_relation, fetch_product_relations, remove_product_relation,
    reorder_product_relations,
};

type RetryIdentity = ProductAdminLifecycleRetryIdentity<String>;

fn retry_registry() -> &'static Mutex<HashMap<String, RetryIdentity>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, RetryIdentity>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lifecycle_operation_segment(operation: ProductAdminLifecycleOperation) -> &'static str {
    match operation {
        ProductAdminLifecycleOperation::CreateProduct => "create-product",
        ProductAdminLifecycleOperation::UpdateProduct => "update-product",
        ProductAdminLifecycleOperation::ChangeStatus => "change-status",
        ProductAdminLifecycleOperation::DeleteProduct => "delete-product",
        ProductAdminLifecycleOperation::CreateVariant => "create-variant",
        ProductAdminLifecycleOperation::UpdateVariant => "update-variant",
        ProductAdminLifecycleOperation::DeleteVariant => "delete-variant",
        ProductAdminLifecycleOperation::AddImage => "add-image",
        ProductAdminLifecycleOperation::UpdateImage => "update-image",
        ProductAdminLifecycleOperation::DeleteImage => "delete-image",
        ProductAdminLifecycleOperation::ReorderImages => "reorder-images",
    }
}

fn lifecycle_slot(
    operation: ProductAdminLifecycleOperation,
    tenant_id: &str,
    actor_id: &str,
    product_id: Option<&str>,
) -> String {
    format!(
        "{}:{}:{}:{}",
        lifecycle_operation_segment(operation),
        tenant_id,
        actor_id,
        product_id.unwrap_or("new")
    )
}

fn retained_caller_key(
    slot: &str,
    operation: ProductAdminLifecycleOperation,
    intent: String,
) -> String {
    let mut registry = retry_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    registry
        .entry(slot.to_string())
        .or_default()
        .idempotency_key_for(operation, &intent)
}

fn mark_lifecycle_succeeded(slot: &str) {
    let mut registry = retry_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(identity) = registry.get_mut(slot) {
        identity.mark_succeeded();
    }
    registry.remove(slot);
}

fn draft_intent(
    operation: ProductAdminLifecycleOperation,
    tenant_id: &str,
    actor_id: &str,
    product_id: Option<&str>,
    draft: &ProductDraft,
) -> String {
    format!(
        "operation={};tenant={tenant_id:?};actor={actor_id:?};product={product_id:?};draft={draft:?}",
        lifecycle_operation_segment(operation),
    )
}

fn status_intent(tenant_id: &str, actor_id: &str, product_id: &str, status: &str) -> String {
    format!(
        "operation=change-status;tenant={tenant_id:?};actor={actor_id:?};product={product_id:?};status={status:?}"
    )
}

fn delete_intent(tenant_id: &str, actor_id: &str, product_id: &str) -> String {
    format!(
        "operation=delete-product;tenant={tenant_id:?};actor={actor_id:?};product={product_id:?}"
    )
}

pub(crate) async fn create_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: ProductDraft,
) -> Result<ProductDetail, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::CreateProduct;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, None);
    let intent = draft_intent(operation, &tenant_id, &user_id, None, &draft);
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = crate::product_lifecycle_graphql::create_product(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        idempotency_key,
        draft,
    )
    .await;
    if result.is_ok() {
        mark_lifecycle_succeeded(&slot);
    }
    result
}

pub(crate) async fn update_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    draft: ProductDraft,
) -> Result<ProductDetail, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::UpdateProduct;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&id));
    let intent = draft_intent(operation, &tenant_id, &user_id, Some(&id), &draft);
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = crate::product_lifecycle_graphql::update_product(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        id,
        idempotency_key,
        draft,
    )
    .await;
    if result.is_ok() {
        mark_lifecycle_succeeded(&slot);
    }
    result
}

pub(crate) async fn change_product_status(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    status: &str,
) -> Result<ProductDetail, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::ChangeStatus;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&id));
    let intent = status_intent(&tenant_id, &user_id, &id, status);
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = crate::product_lifecycle_graphql::change_product_status(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        id,
        status,
        idempotency_key,
    )
    .await;
    if result.is_ok() {
        mark_lifecycle_succeeded(&slot);
    }
    result
}

pub(crate) async fn delete_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
) -> Result<bool, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::DeleteProduct;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&id));
    let intent = delete_intent(&tenant_id, &user_id, &id);
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = crate::product_lifecycle_graphql::delete_product(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        id,
        idempotency_key,
    )
    .await;
    if result.is_ok() {
        mark_lifecycle_succeeded(&slot);
    }
    result
}

pub(crate) async fn create_product_variant(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    draft: crate::model::VariantDraft,
) -> Result<crate::model::ProductVariant, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::CreateVariant;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&product_id));
    let intent = format!("operation=create-variant;tenant={tenant_id:?};actor={user_id:?};product={product_id:?};sku={:?}", draft.sku);
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = crate::product_lifecycle_graphql::create_product_variant(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        product_id,
        idempotency_key,
        draft,
    )
    .await;
    if result.is_ok() {
        mark_lifecycle_succeeded(&slot);
    }
    result
}

pub(crate) async fn update_product_variant(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    variant_id: String,
    draft: crate::model::VariantDraft,
) -> Result<crate::model::ProductVariant, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::UpdateVariant;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&variant_id));
    let intent = format!("operation=update-variant;tenant={tenant_id:?};actor={user_id:?};variant={variant_id:?};sku={:?}", draft.sku);
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = crate::product_lifecycle_graphql::update_product_variant(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        variant_id,
        idempotency_key,
        draft,
    )
    .await;
    if result.is_ok() {
        mark_lifecycle_succeeded(&slot);
    }
    result
}

pub(crate) async fn delete_product_variant(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    variant_id: String,
) -> Result<bool, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::DeleteVariant;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&variant_id));
    let intent = format!("operation=delete-variant;tenant={tenant_id:?};actor={user_id:?};variant={variant_id:?}");
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = crate::product_lifecycle_graphql::delete_product_variant(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        variant_id,
        idempotency_key,
    )
    .await;
    if result.is_ok() {
        mark_lifecycle_succeeded(&slot);
    }
    result
}

pub(crate) async fn add_product_image(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    draft: crate::model::ProductImageDraft,
) -> Result<crate::model::ProductImage, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::AddImage;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&product_id));
    let intent = format!("operation=add-image;tenant={tenant_id:?};actor={user_id:?};product={product_id:?};media={:?}", draft.media_id);
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = crate::product_lifecycle_graphql::add_product_image(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        product_id,
        idempotency_key,
        draft,
    )
    .await;
    if result.is_ok() {
        mark_lifecycle_succeeded(&slot);
    }
    result
}

pub(crate) async fn update_product_image(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    image_id: String,
    draft: crate::model::UpdateProductImageDraft,
) -> Result<crate::model::ProductImage, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::UpdateImage;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&image_id));
    let intent = format!("operation=update-image;tenant={tenant_id:?};actor={user_id:?};image={image_id:?}");
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = crate::product_lifecycle_graphql::update_product_image(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        product_id,
        image_id,
        idempotency_key,
        draft,
    )
    .await;
    if result.is_ok() {
        mark_lifecycle_succeeded(&slot);
    }
    result
}

pub(crate) async fn delete_product_image(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    image_id: String,
) -> Result<bool, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::DeleteImage;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&image_id));
    let intent = format!("operation=delete-image;tenant={tenant_id:?};actor={user_id:?};image={image_id:?}");
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = crate::product_lifecycle_graphql::delete_product_image(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        product_id,
        image_id,
        idempotency_key,
    )
    .await;
    if result.is_ok() {
        mark_lifecycle_succeeded(&slot);
    }
    result
}

pub(crate) async fn reorder_product_images(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    image_ids: Vec<String>,
) -> Result<bool, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::ReorderImages;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&product_id));
    let intent = format!("operation=reorder-images;tenant={tenant_id:?};actor={user_id:?};product={product_id:?};images={image_ids:?}");
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = crate::product_lifecycle_graphql::reorder_product_images(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        product_id,
        image_ids,
        idempotency_key,
    )
    .await;
    if result.is_ok() {
        mark_lifecycle_succeeded(&slot);
    }
    result
}
