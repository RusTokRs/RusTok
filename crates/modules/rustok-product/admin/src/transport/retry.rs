use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use rustok_graphql::{GraphqlHttpError, GraphqlOperationError};

use super::product_lifecycle_graphql;
use crate::lifecycle_retry_identity::{
    ProductAdminLifecycleOperation, ProductAdminLifecycleRetryIdentity,
};
use crate::model::{
    ProductDetail, ProductDraft, ProductImage, ProductImageDraft, ProductVariant,
    UpdateProductImageDraft, VariantUpdateDraft,
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

fn mark_lifecycle_rejected(slot: &str) {
    retry_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(slot);
}

fn finish_lifecycle_result<T>(
    slot: &str,
    result: Result<T, GraphqlOperationError>,
) -> Result<T, GraphqlHttpError> {
    match result {
        Ok(value) => {
            mark_lifecycle_succeeded(slot);
            Ok(value)
        }
        Err(error) => {
            let durable_rejection = error.extensions.as_ref().is_some_and(|extensions| {
                extensions.outcome.as_deref() == Some("rejected")
                    && extensions.retryable == Some(false)
            });
            if durable_rejection {
                mark_lifecycle_rejected(slot);
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
        idempotency_key,
        draft,
    )
    .await;
    finish_lifecycle_result(&slot, result)
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
        idempotency_key,
        draft,
    )
    .await;
    finish_lifecycle_result(&slot, result)
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
        idempotency_key,
    )
    .await;
    finish_lifecycle_result(&slot, result)
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
        idempotency_key,
    )
    .await;
    finish_lifecycle_result(&slot, result)
}

fn effective_optional_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn variant_update_intent(
    tenant_id: &str,
    user_id: &str,
    variant_id: &str,
    draft: &VariantUpdateDraft,
) -> String {
    // Match the exact effective GraphQL input. An untouched price editor buffer is not
    // sent unless prices_changed is true, and an empty axis buffer is omitted.
    let axis_values = (!draft.axis_values.is_empty()).then_some(&draft.axis_values);
    let prices = draft.prices_changed.then(|| {
        draft
            .prices
            .iter()
            .map(|price| {
                (
                    price.currency_code.as_str(),
                    price.amount.as_str(),
                    effective_optional_text(price.compare_at_amount.as_deref()),
                )
            })
            .collect::<Vec<_>>()
    });
    let inventory_policy = effective_optional_text(draft.inventory_policy.as_deref());

    format!(
        "operation=update-variant;tenant={tenant_id:?};actor={user_id:?};variant={variant_id:?};sku={:?};barcode={:?};shipping_profile_slug={:?};weight={:?};weight_unit={:?};axis_values={axis_values:?};prices={prices:?};inventory_quantity={:?};expected_inventory_quantity={:?};expected_revision={:?};inventory_policy={inventory_policy:?}",
        draft.sku,
        draft.barcode,
        draft.shipping_profile_slug,
        draft.weight,
        draft.weight_unit,
        draft.inventory_quantity,
        draft.expected_inventory_quantity,
        draft.expected_revision
    )
}

fn variant_delete_intent(
    tenant_id: &str,
    user_id: &str,
    variant_id: &str,
    expected_revision: &str,
) -> String {
    format!(
        "operation=delete-variant;tenant={tenant_id:?};actor={user_id:?};variant={variant_id:?};expected_revision={expected_revision:?}"
    )
}

pub(crate) async fn update_product_variant(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    variant_id: String,
    draft: VariantUpdateDraft,
) -> Result<ProductVariant, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::UpdateVariant;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&variant_id));
    let intent = variant_update_intent(&tenant_id, &user_id, &variant_id, &draft);
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = product_lifecycle_graphql::update_product_variant(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        variant_id,
        idempotency_key,
        draft,
    )
    .await;
    finish_lifecycle_result(&slot, result)
}

pub(crate) async fn delete_product_variant(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    variant_id: String,
    expected_revision: String,
) -> Result<bool, GraphqlHttpError> {
    let operation = ProductAdminLifecycleOperation::DeleteVariant;
    let slot = lifecycle_slot(operation, &tenant_id, &user_id, Some(&variant_id));
    let intent = variant_delete_intent(
        &tenant_id,
        &user_id,
        &variant_id,
        &expected_revision,
    );
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = product_lifecycle_graphql::delete_product_variant(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        variant_id,
        expected_revision,
        idempotency_key,
    )
    .await;
    finish_lifecycle_result(&slot, result)
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
        idempotency_key,
        draft,
    )
    .await;
    finish_lifecycle_result(&slot, result)
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
        idempotency_key,
        draft,
    )
    .await;
    finish_lifecycle_result(&slot, result)
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
        idempotency_key,
    )
    .await;
    finish_lifecycle_result(&slot, result)
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
        idempotency_key,
    )
    .await;
    finish_lifecycle_result(&slot, result)
}

#[cfg(test)]
mod variant_retry_tests {
    use super::*;
    use rustok_api::Patch;

    #[test]
    fn retries_reuse_identical_intent_but_rotate_for_non_sku_edits_and_clears() {
        let operation = ProductAdminLifecycleOperation::UpdateVariant;
        let mut identity = RetryIdentity::default();
        let mut draft = VariantUpdateDraft::default();
        let original = variant_update_intent("tenant", "actor", "variant", &draft);
        let key = identity.idempotency_key_for(operation, &original);
        assert_eq!(key, identity.idempotency_key_for(operation, &original));
        draft.barcode = Patch::Clear;
        let clear = variant_update_intent("tenant", "actor", "variant", &draft);
        assert_ne!(key, identity.idempotency_key_for(operation, &clear));
        let previous = clear;
        draft.shipping_profile_slug = Patch::Set("parcel".into());
        assert_ne!(
            previous,
            variant_update_intent("tenant", "actor", "variant", &draft)
        );
        let previous = variant_update_intent("tenant", "actor", "variant", &draft);
        draft.weight = Patch::Clear;
        assert_ne!(
            previous,
            variant_update_intent("tenant", "actor", "variant", &draft)
        );
        let previous = variant_update_intent("tenant", "actor", "variant", &draft);
        draft.weight_unit = Patch::Set("kg".into());
        assert_ne!(
            previous,
            variant_update_intent("tenant", "actor", "variant", &draft)
        );
        let previous = variant_update_intent("tenant", "actor", "variant", &draft);
        draft.inventory_quantity = Some(3);
        assert_ne!(
            previous,
            variant_update_intent("tenant", "actor", "variant", &draft)
        );
        let previous = variant_update_intent("tenant", "actor", "variant", &draft);
        draft.prices.push(crate::model::VariantPriceDraft {
            amount: "1".into(),
            currency_code: "USD".into(),
            compare_at_amount: None,
        });
        assert_eq!(
            previous,
            variant_update_intent("tenant", "actor", "variant", &draft),
            "unmarked prices are only an editor buffer and are not part of the owner request"
        );
        draft.prices_changed = true;
        assert_ne!(
            previous,
            variant_update_intent("tenant", "actor", "variant", &draft),
            "an explicit price replacement must produce a different retry identity"
        );
    }

    #[test]
    fn variant_update_intent_omits_empty_price_replacements_and_blank_inventory_policy() {
        let draft = VariantUpdateDraft::default();
        let original = variant_update_intent("tenant", "actor", "variant", &draft);

        let mut blank_policy = draft.clone();
        blank_policy.inventory_policy = Some("  ".to_owned());
        assert_eq!(
            original,
            variant_update_intent("tenant", "actor", "variant", &blank_policy)
        );

        let mut clear_prices = draft;
        clear_prices.prices_changed = true;
        assert_ne!(
            original,
            variant_update_intent("tenant", "actor", "variant", &clear_prices),
            "an explicit empty replacement clears prices and is not equivalent to Keep"
        );
    }

    #[test]
    fn variant_delete_attempt_identity_includes_the_expected_revision() {
        let first = variant_delete_intent("tenant", "actor", "variant", "7");
        let retry = variant_delete_intent("tenant", "actor", "variant", "7");
        let refreshed = variant_delete_intent("tenant", "actor", "variant", "8");

        assert_eq!(first, retry);
        assert_ne!(first, refreshed);
    }

    #[test]
    fn terminal_graphql_rejection_rotates_retry_key() {
        let slot = "product-terminal-rejection-retry-test";
        let intent = "same-variant-delete-intent".to_owned();
        let first = retained_caller_key(
            slot,
            ProductAdminLifecycleOperation::DeleteVariant,
            intent.clone(),
        );
        let error = GraphqlOperationError {
            error: GraphqlHttpError::Graphql("request rejected".to_owned()),
            extensions: Some(rustok_graphql::GraphqlErrorExtensions {
                code: Some("PRODUCT_VARIANT_INVENTORY_BLOCKED".to_owned()),
                retryable: Some(false),
                outcome: Some("rejected".to_owned()),
            }),
        };

        assert!(finish_lifecycle_result::<()>(slot, Err(error)).is_err());
        let retry = retained_caller_key(
            slot,
            ProductAdminLifecycleOperation::DeleteVariant,
            intent,
        );

        assert_ne!(first, retry);
        mark_lifecycle_rejected(slot);
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

        assert!(finish_lifecycle_result::<()>(slot, Err(error)).is_err());
        let retry = retained_caller_key(
            slot,
            ProductAdminLifecycleOperation::UpdateProduct,
            intent,
        );

        assert_eq!(first, retry);
        mark_lifecycle_rejected(slot);
    }

}
