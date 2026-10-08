use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::sync::{Mutex, OnceLock};

use uuid::Uuid;

/// Schema-authoring writes that carry a caller idempotency key on the owner boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminSchemaOperation {
    CreateAttribute,
    CreateAttributeOption,
    CreateCategory,
    CreateSchema,
    CreateSchemaGroup,
    CreateCategoryGroup,
    SetCategorySchemaMode,
    BindSchemaAttribute,
    BindCategoryAttribute,
    SaveAttributeValues,
    ClearDetachedAttributeValues,
}

impl ProductAdminSchemaOperation {
    pub(crate) const fn key_segment(self) -> &'static str {
        match self {
            Self::CreateAttribute => "create-attribute",
            Self::CreateAttributeOption => "create-attribute-option",
            Self::CreateCategory => "create-category",
            Self::CreateSchema => "create-schema",
            Self::CreateSchemaGroup => "create-schema-group",
            Self::CreateCategoryGroup => "create-category-group",
            Self::SetCategorySchemaMode => "set-category-schema-mode",
            Self::BindSchemaAttribute => "bind-schema-attribute",
            Self::BindCategoryAttribute => "bind-category-attribute",
            Self::SaveAttributeValues => "save-attribute-values",
            Self::ClearDetachedAttributeValues => "clear-detached-attribute-values",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingSchemaInvocation<I> {
    operation: ProductAdminSchemaOperation,
    intent: I,
    idempotency_key: String,
}

/// FFA-owned caller identity retained across an explicit retry of one logical schema write.
///
/// The same operation + intent reuses one caller key after a transport or server failure, so the
/// owner can deduplicate the retry. Changing the operation or the intent starts a new logical
/// invocation and rotates the key. A successful owner response must call `mark_succeeded`, which
/// releases the pending identity so a later identical user action is not aliased to the completed
/// command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminSchemaRetryIdentity<I> {
    pending: Option<PendingSchemaInvocation<I>>,
}

impl<I> Default for ProductAdminSchemaRetryIdentity<I> {
    fn default() -> Self {
        Self { pending: None }
    }
}

impl<I> ProductAdminSchemaRetryIdentity<I>
where
    I: Clone + PartialEq,
{
    pub(crate) fn idempotency_key_for(
        &mut self,
        operation: ProductAdminSchemaOperation,
        intent: &I,
    ) -> String {
        if let Some(pending) = self.pending.as_ref()
            && pending.operation == operation
            && &pending.intent == intent
        {
            return pending.idempotency_key.clone();
        }

        let idempotency_key = format!(
            "product-admin-schema:{}:{}",
            operation.key_segment(),
            Uuid::new_v4()
        );
        self.pending = Some(PendingSchemaInvocation {
            operation,
            intent: intent.clone(),
            idempotency_key: idempotency_key.clone(),
        });
        idempotency_key
    }

    pub(crate) fn mark_succeeded(&mut self) {
        self.pending = None;
    }

    #[cfg(test)]
    fn pending_key(&self) -> Option<&str> {
        self.pending
            .as_ref()
            .map(|pending| pending.idempotency_key.as_str())
    }
}

/// Process-wide registry of retained caller identities, keyed by logical invocation slot.
fn retry_registry() -> &'static Mutex<HashMap<String, RetryIdentity>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, RetryIdentity>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Slot of one logical schema invocation: operation plus tenant, actor and optional scope.
pub(crate) fn schema_slot(
    operation: ProductAdminSchemaOperation,
    tenant_id: &str,
    actor_id: &str,
    scope: Option<&str>,
) -> String {
    format!(
        "{}:{}:{}:{}",
        operation.key_segment(),
        tenant_id,
        actor_id,
        scope.unwrap_or("catalog")
    )
}

/// Intent fingerprint: a changed payload or locale rotates the retained caller key.
pub(crate) fn schema_intent(
    operation: ProductAdminSchemaOperation,
    tenant_id: &str,
    actor_id: &str,
    locale: Option<&str>,
    payload: &impl Debug,
) -> String {
    format!(
        "operation={};tenant={tenant_id:?};actor={actor_id:?};locale={locale:?};payload={payload:?}",
        operation.key_segment(),
    )
}

type RetryIdentity = ProductAdminSchemaRetryIdentity<String>;

/// Returns the caller key retained for this logical invocation (reused on explicit retry).
fn retained_caller_key(
    slot: &str,
    operation: ProductAdminSchemaOperation,
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

/// Releases the caller identity after the owner accepted the write.
fn mark_succeeded(slot: &str) {
    let mut registry = retry_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(identity) = registry.get_mut(slot) {
        identity.mark_succeeded();
    }
    registry.remove(slot);
}

/// Runs one logical schema write with the caller key retained for `slot`.
///
/// The key is minted once here and handed to `write`, which may try the native owner path and
/// then the GraphQL fallback with the same key. The identity is released only on `Ok`; an error
/// keeps it so an explicit retry with an unchanged intent reuses the same owner receipt.
pub(crate) async fn run_keyed_schema_write<T, E, Fut>(
    slot: String,
    operation: ProductAdminSchemaOperation,
    intent: String,
    write: impl FnOnce(String) -> Fut,
) -> Result<T, E>
where
    Fut: Future<Output = Result<T, E>>,
{
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result = write(idempotency_key).await;
    if result.is_ok() {
        mark_succeeded(&slot);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_retry_reuses_the_same_caller_key() {
        let mut identity = ProductAdminSchemaRetryIdentity::default();
        let intent = "tenant/user/attribute:color".to_string();

        let first =
            identity.idempotency_key_for(ProductAdminSchemaOperation::CreateAttribute, &intent);
        let retry =
            identity.idempotency_key_for(ProductAdminSchemaOperation::CreateAttribute, &intent);

        assert_eq!(first, retry);
        assert_eq!(identity.pending_key(), Some(first.as_str()));
    }

    #[test]
    fn changed_intent_rotates_the_caller_key() {
        let mut identity = ProductAdminSchemaRetryIdentity::default();
        let draft_a = "tenant/user/attribute:color".to_string();
        let draft_b = "tenant/user/attribute:size".to_string();

        let first =
            identity.idempotency_key_for(ProductAdminSchemaOperation::CreateAttribute, &draft_a);
        let changed =
            identity.idempotency_key_for(ProductAdminSchemaOperation::CreateAttribute, &draft_b);

        assert_ne!(first, changed);
    }

    #[test]
    fn changed_operation_rotates_even_when_intent_matches() {
        let mut identity = ProductAdminSchemaRetryIdentity::default();
        let intent = "tenant/user/category:7".to_string();

        let group =
            identity.idempotency_key_for(ProductAdminSchemaOperation::CreateCategoryGroup, &intent);
        let mode = identity
            .idempotency_key_for(ProductAdminSchemaOperation::SetCategorySchemaMode, &intent);

        assert_ne!(group, mode);
    }

    #[test]
    fn successful_completion_releases_identity_for_a_later_equal_command() {
        let mut identity = ProductAdminSchemaRetryIdentity::default();
        let intent = "tenant/user/schema:books".to_string();

        let first =
            identity.idempotency_key_for(ProductAdminSchemaOperation::CreateSchema, &intent);
        identity.mark_succeeded();
        assert_eq!(identity.pending_key(), None);

        let later =
            identity.idempotency_key_for(ProductAdminSchemaOperation::CreateSchema, &intent);
        assert_ne!(first, later);
    }

    #[test]
    fn generated_keys_use_the_schema_namespace_and_fit_the_limit() {
        let mut identity = ProductAdminSchemaRetryIdentity::default();
        let key = identity.idempotency_key_for(
            ProductAdminSchemaOperation::ClearDetachedAttributeValues,
            &"intent".to_string(),
        );

        assert!(key.starts_with("product-admin-schema:clear-detached-attribute-values:"));
        assert!(key.len() <= 191);
    }
}
