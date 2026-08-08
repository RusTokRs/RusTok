use uuid::Uuid;

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

/// Product Admin caller identity retained across one explicit retry of the same schema write.
///
/// A failed transport/server call keeps the key. Changing the operation or exact intent rotates
/// it. A successful response must clear the pending identity so a later identical user action is
/// a new logical write. Owner-side durable replay is a separate contract and is not implied here.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_retry_reuses_schema_write_caller_key() {
        let mut identity = ProductAdminSchemaRetryIdentity::default();
        let intent = "tenant/user/category:123/mode:inherit".to_string();
        let first = identity.idempotency_key_for(
            ProductAdminSchemaOperation::SetCategorySchemaMode,
            &intent,
        );
        let retry = identity.idempotency_key_for(
            ProductAdminSchemaOperation::SetCategorySchemaMode,
            &intent,
        );
        assert_eq!(first, retry);
        assert_eq!(identity.pending_key(), Some(first.as_str()));
    }

    #[test]
    fn changed_schema_write_intent_rotates_key() {
        let mut identity = ProductAdminSchemaRetryIdentity::default();
        let first = identity.idempotency_key_for(
            ProductAdminSchemaOperation::CreateAttribute,
            &"code=color".to_string(),
        );
        let changed = identity.idempotency_key_for(
            ProductAdminSchemaOperation::CreateAttribute,
            &"code=size".to_string(),
        );
        assert_ne!(first, changed);
    }

    #[test]
    fn success_releases_schema_retry_identity() {
        let mut identity = ProductAdminSchemaRetryIdentity::default();
        let intent = "product:123/save-values".to_string();
        let first = identity.idempotency_key_for(
            ProductAdminSchemaOperation::SaveAttributeValues,
            &intent,
        );
        identity.mark_succeeded();
        let later = identity.idempotency_key_for(
            ProductAdminSchemaOperation::SaveAttributeValues,
            &intent,
        );
        assert_ne!(first, later);
    }

    #[test]
    fn generated_schema_keys_fit_graphql_limit() {
        let mut identity = ProductAdminSchemaRetryIdentity::default();
        let key = identity.idempotency_key_for(
            ProductAdminSchemaOperation::ClearDetachedAttributeValues,
            &"intent".to_string(),
        );
        assert!(key.starts_with("product-admin-schema:clear-detached-attribute-values:"));
        assert!(key.len() <= 191);
    }
}
