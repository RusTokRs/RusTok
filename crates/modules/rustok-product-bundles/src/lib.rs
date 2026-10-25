use sha2::{Digest, Sha256};
use uuid::Uuid;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod services;

pub use dto::*;
pub use error::{BundleError, BundleResult};
pub use ports::BundlePort;
pub use services::{
    ADD_BUNDLE_ITEM_OPERATION, BundleCommandContext, BundleCommandError, BundleService,
    CREATE_BUNDLE_OPERATION, DELETE_BUNDLE_OPERATION, PRODUCT_BUNDLE_OWNER_SLUG,
    REMOVE_BUNDLE_ITEM_OPERATION, UPDATE_BUNDLE_OPERATION, bundle_command_error,
};

/// Stable namespace for caller keys accepted by every Product Bundle ingress.
///
/// Kept compatible with the initial Commerce GraphQL namespace so existing retries retain their
/// receipt identity as native and GraphQL adapters converge on this owner helper.
pub const BUNDLE_CALLER_IDEMPOTENCY_NAMESPACE: &str = "commerce-graphql-bundle";

/// Derives the owner receipt identity for a caller-supplied Bundle command key.
///
/// This deployed Product Bundle namespace deliberately retains its original digest framing:
/// receipt keys are durable, so switching the formula would orphan in-flight and replayable
/// Commerce GraphQL receipts after an upgrade. A transition to a newly framed key must support
/// both receipt identities atomically rather than silently replacing this formula. Callers must
/// validate the caller key (non-empty, bounded length) before calling this.
pub fn scoped_caller_idempotency_key(
    tenant_id: Uuid,
    user_id: Uuid,
    operation: &str,
    subject_id: Option<Uuid>,
    caller_key: &str,
) -> String {
    let mut digest = Sha256::new();
    digest.update(tenant_id.as_bytes());
    digest.update(user_id.as_bytes());
    digest.update(operation.as_bytes());
    if let Some(subject_id) = subject_id {
        digest.update(subject_id.as_bytes());
    }
    digest.update(caller_key.trim().as_bytes());
    format!(
        "{BUNDLE_CALLER_IDEMPOTENCY_NAMESPACE}:{operation}:{}",
        hex::encode(digest.finalize())
    )
}

pub mod module;
pub use module::ProductBundlesModule;

#[cfg(test)]
mod contract_tests;


#[cfg(test)]
mod tests {
    use super::scoped_caller_idempotency_key;
    use uuid::Uuid;

    #[test]
    fn deployed_bundle_receipt_key_formula_stays_stable() {
        let key = scoped_caller_idempotency_key(
            Uuid::from_u128(0x11111111111111111111111111111111),
            Uuid::from_u128(0x22222222222222222222222222222222),
            "add_bundle_item",
            Some(Uuid::from_u128(0x33333333333333333333333333333333)),
            " retry-123 ",
        );

        assert_eq!(
            key,
            "commerce-graphql-bundle:add_bundle_item:a4bb27f8e41e57f2b7697f53acfe3503b0d8649acdd9ab1f1faba7e769f11740"
        );
    }
}
