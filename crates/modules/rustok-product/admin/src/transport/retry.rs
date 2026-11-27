use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use rustok_graphql::{GraphqlHttpError, GraphqlOperationError};

use super::product_lifecycle_graphql;
use crate::lifecycle_retry_identity::{
    ProductAdminLifecycleOperation, ProductAdminLifecycleRetryIdentity,
};
use crate::model::{
    ProductDetail, ProductDraft, ProductImage, ProductImageDraft, UpdateProductImageDraft,
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

fn clear_lifecycle_identity_if_current(slot: &str, idempotency_key: &str) {
    let mut registry = retry_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let should_remove = registry
        .get_mut(slot)
        .is_some_and(|identity| identity.clear_if_key_matches(idempotency_key));
    if should_remove {
        registry.remove(slot);
    }
}

fn finish_lifecycle_result<T>(
    slot: &str,
    idempotency_key: &str,
    result: Result<T, GraphqlOperationError>,
) -> Result<T, GraphqlHttpError> {
    match result {
        Ok(value) => {
            clear_lifecycle_identity_if_current(slot, idempotency_key);
            Ok(value)
        }
        Err(error) => {
            let durable_rejection = error.extensions.as_ref().is_some_and(|extensions| {
                extensions.outcome.as_deref() == Some("rejected")
                    && extensions.retryable == Some(false)
            });
            if durable_rejection {
                clear_lifecycle_identity_if_current(slot, idempotency_key);
            }
            Err(error.error)
        }
    }
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
    let result = product_lifecycle_graphql::create_product(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        idempotency_key.clone(),
        draft,
    )
    .await;
    finish_lifecycle_result(&slot, &idempotency_key, result)
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
    let result = product_lifecycle_graphql::update_product(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        id,
        idempotency_key.clone(),
        draft,
    )
    .await;
    finish_lifecycle_result(&slot, &idempotency_key, result)
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
    let result = product_lifecycle_graphql::change_product_status(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        id,
        status,
        idempotency_key.clone(),
    )
    .await;
    finish_lifecycle_result(&slot, &idempotency_key, result)
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
    let result = product_lifecycle_graphql::delete_product(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        id,
        idempotency_key.clone(),
    )
    .await;
    finish_lifecycle_result(&slot, &idempotency_key, result)
}

pub(crate) async fn add_product_image(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    draft: ProductImageDraft,
) -> Result<ProductImage, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::AddImage;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&product_id));
    let intent = format!(
        "operation=add-image;tenant={tenant_id:?};actor={user_id:?};product={product_id:?};media={:?}",
        draft.media_id
    );
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = product_lifecycle_graphql::add_product_image(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        product_id,
        idempotency_key.clone(),
        draft,
    )
    .await;
    finish_lifecycle_result(&slot, &idempotency_key, result)
}

pub(crate) async fn update_product_image(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    image_id: String,
    draft: UpdateProductImageDraft,
) -> Result<ProductImage, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::UpdateImage;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&image_id));
    let intent =
        format!("operation=update-image;tenant={tenant_id:?};actor={user_id:?};image={image_id:?}");
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = product_lifecycle_graphql::update_product_image(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        product_id,
        image_id,
        idempotency_key.clone(),
        draft,
    )
    .await;
    finish_lifecycle_result(&slot, &idempotency_key, result)
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
    let intent =
        format!("operation=delete-image;tenant={tenant_id:?};actor={user_id:?};image={image_id:?}");
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = product_lifecycle_graphql::delete_product_image(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        product_id,
        image_id,
        idempotency_key.clone(),
    )
    .await;
    finish_lifecycle_result(&slot, &idempotency_key, result)
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
    let intent = format!(
        "operation=reorder-images;tenant={tenant_id:?};actor={user_id:?};product={product_id:?};images={image_ids:?}"
    );
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = product_lifecycle_graphql::reorder_product_images(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        product_id,
        image_ids,
        idempotency_key.clone(),
    )
    .await;
    finish_lifecycle_result(&slot, &idempotency_key, result)
}

#[cfg(test)]
mod lifecycle_retry_tests {
    use super::*;

    #[test]
    fn terminal_graphql_rejection_rotates_retry_key() {
        let slot = "product-terminal-rejection-retry-test";
        let intent = "same-product-delete-intent".to_owned();
        let first = retained_caller_key(
            slot,
            ProductAdminLifecycleOperation::DeleteProduct,
            intent.clone(),
        );
        let error = GraphqlOperationError {
            error: GraphqlHttpError::Graphql("request rejected".to_owned()),
            extensions: Some(rustok_graphql::GraphqlErrorExtensions {
                code: Some("PRODUCT_CONFLICT".to_owned()),
                retryable: Some(false),
                outcome: Some("rejected".to_owned()),
            }),
        };

        assert!(finish_lifecycle_result::<()>(slot, &first, Err(error)).is_err());
        let retry =
            retained_caller_key(slot, ProductAdminLifecycleOperation::DeleteProduct, intent);

        assert_ne!(first, retry);
        clear_lifecycle_identity_if_current(slot, &retry);
    }

    #[test]
    fn late_success_from_an_old_request_cannot_clear_a_newer_pending_identity() {
        let slot = "product-late-success-retry-test";
        let operation = ProductAdminLifecycleOperation::DeleteProduct;
        let intent = "same-product-delete-intent".to_owned();
        let first = retained_caller_key(slot, operation, intent.clone());

        assert!(finish_lifecycle_result::<()>(slot, &first, Ok(())).is_ok());
        let second = retained_caller_key(slot, operation, intent.clone());
        assert_ne!(first, second);

        assert!(finish_lifecycle_result::<()>(slot, &first, Ok(())).is_ok());
        assert_eq!(second, retained_caller_key(slot, operation, intent));
        clear_lifecycle_identity_if_current(slot, &second);
    }

    #[test]
    fn late_terminal_rejection_from_an_old_intent_cannot_clear_a_newer_pending_identity() {
        let slot = "product-late-rejection-retry-test";
        let operation = ProductAdminLifecycleOperation::UpdateProduct;
        let old_intent = "old-product-edit".to_owned();
        let new_intent = "new-product-edit".to_owned();
        let first = retained_caller_key(slot, operation, old_intent);
        let second = retained_caller_key(slot, operation, new_intent.clone());
        assert_ne!(first, second);

        let error = GraphqlOperationError {
            error: GraphqlHttpError::Graphql("old request rejected".to_owned()),
            extensions: Some(rustok_graphql::GraphqlErrorExtensions {
                code: Some("PRODUCT_CONFLICT".to_owned()),
                retryable: Some(false),
                outcome: Some("rejected".to_owned()),
            }),
        };
        assert!(finish_lifecycle_result::<()>(slot, &first, Err(error)).is_err());

        assert_eq!(second, retained_caller_key(slot, operation, new_intent));
        clear_lifecycle_identity_if_current(slot, &second);
    }

    #[test]
    fn unknown_graphql_outcome_retains_retry_key_for_recovery() {
        let slot = "product-unknown-outcome-retry-test";
        let intent = "same-product-update-intent".to_owned();
        let first = retained_caller_key(
            slot,
            ProductAdminLifecycleOperation::UpdateProduct,
            intent.clone(),
        );
        let error = GraphqlOperationError {
            error: GraphqlHttpError::Network,
            extensions: None,
        };

        assert!(finish_lifecycle_result::<()>(slot, &first, Err(error)).is_err());
        let retry =
            retained_caller_key(slot, ProductAdminLifecycleOperation::UpdateProduct, intent);

        assert_eq!(first, retry);
        clear_lifecycle_identity_if_current(slot, &retry);
    }
}
