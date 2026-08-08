use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminLifecycleOperation {
    CreateProduct,
    UpdateProduct,
    ChangeStatus,
    DeleteProduct,
}

impl ProductAdminLifecycleOperation {
    const fn key_segment(self) -> &'static str {
        match self {
            Self::CreateProduct => "create-product",
            Self::UpdateProduct => "update-product",
            Self::ChangeStatus => "change-status",
            Self::DeleteProduct => "delete-product",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingLifecycleInvocation<I> {
    operation: ProductAdminLifecycleOperation,
    intent: I,
    idempotency_key: String,
}

/// FFA-owned caller identity retained across an explicit retry of one logical Product command.
///
/// The same operation + intent reuses one caller key after transport/server failure. Changing the
/// operation or intent starts a new logical invocation and therefore rotates the caller key. A
/// successful owner response must call `mark_succeeded`, which clears the pending identity so a
/// later identical user action cannot alias the completed command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminLifecycleRetryIdentity<I> {
    pending: Option<PendingLifecycleInvocation<I>>,
}

impl<I> Default for ProductAdminLifecycleRetryIdentity<I> {
    fn default() -> Self {
        Self { pending: None }
    }
}

impl<I> ProductAdminLifecycleRetryIdentity<I>
where
    I: Clone + PartialEq,
{
    pub(crate) fn idempotency_key_for(
        &mut self,
        operation: ProductAdminLifecycleOperation,
        intent: &I,
    ) -> String {
        if let Some(pending) = self.pending.as_ref()
            && pending.operation == operation
            && &pending.intent == intent
        {
            return pending.idempotency_key.clone();
        }

        let idempotency_key = format!(
            "product-admin:{}:{}",
            operation.key_segment(),
            Uuid::new_v4()
        );
        self.pending = Some(PendingLifecycleInvocation {
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
    fn explicit_retry_reuses_the_same_caller_key() {
        let mut identity = ProductAdminLifecycleRetryIdentity::default();
        let intent = "tenant/user/product/status:published".to_string();

        let first = identity.idempotency_key_for(
            ProductAdminLifecycleOperation::ChangeStatus,
            &intent,
        );
        let retry = identity.idempotency_key_for(
            ProductAdminLifecycleOperation::ChangeStatus,
            &intent,
        );

        assert_eq!(first, retry);
        assert_eq!(identity.pending_key(), Some(first.as_str()));
    }

    #[test]
    fn changed_intent_rotates_the_caller_key() {
        let mut identity = ProductAdminLifecycleRetryIdentity::default();
        let draft_a = "tenant/user/create/title:a".to_string();
        let draft_b = "tenant/user/create/title:b".to_string();

        let first = identity.idempotency_key_for(
            ProductAdminLifecycleOperation::CreateProduct,
            &draft_a,
        );
        let changed = identity.idempotency_key_for(
            ProductAdminLifecycleOperation::CreateProduct,
            &draft_b,
        );

        assert_ne!(first, changed);
    }

    #[test]
    fn changed_operation_rotates_even_when_intent_matches() {
        let mut identity = ProductAdminLifecycleRetryIdentity::default();
        let intent = "tenant/user/product:123".to_string();

        let update = identity.idempotency_key_for(
            ProductAdminLifecycleOperation::UpdateProduct,
            &intent,
        );
        let remove = identity.idempotency_key_for(
            ProductAdminLifecycleOperation::DeleteProduct,
            &intent,
        );

        assert_ne!(update, remove);
    }

    #[test]
    fn successful_completion_releases_identity_for_a_later_equal_command() {
        let mut identity = ProductAdminLifecycleRetryIdentity::default();
        let intent = "tenant/user/product:123/delete".to_string();

        let first = identity.idempotency_key_for(
            ProductAdminLifecycleOperation::DeleteProduct,
            &intent,
        );
        identity.mark_succeeded();
        assert_eq!(identity.pending_key(), None);

        let later = identity.idempotency_key_for(
            ProductAdminLifecycleOperation::DeleteProduct,
            &intent,
        );
        assert_ne!(first, later);
    }

    #[test]
    fn generated_keys_fit_the_graphql_contract_limit() {
        let mut identity = ProductAdminLifecycleRetryIdentity::default();
        let key = identity.idempotency_key_for(
            ProductAdminLifecycleOperation::CreateProduct,
            &"intent".to_string(),
        );

        assert!(key.starts_with("product-admin:create-product:"));
        assert!(key.len() <= 191);
    }
}
