//! Predecessor-revision control for the Product catalog aggregate.
//!
//! Every write to the `products` row bumps `revision`. A document update may carry
//! `expected_revision`: when it does not match the stored revision, the owner refuses the write
//! instead of overwriting another operator's change. The refusal travels as a reserved
//! `CommerceError::Validation` message class, because the closed `CommerceError` vocabulary is
//! published (see `public_error.rs` and its diagnostic-safety evidence); the port and GraphQL
//! mappers translate the class into the public `PRODUCT_REVISION_CONFLICT` code.

use crate::CommerceError;
use crate::CommerceResult;
use crate::entities;

/// Reserved message prefix of one predecessor-revision refusal.
pub(crate) const REVISION_CONFLICT_PREFIX: &str = "product revision conflict";

/// Structured facts carried by one predecessor-revision refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RevisionConflictFacts {
    pub expected_revision: i32,
    pub current_revision: i32,
}

/// Builds the reserved refusal for one stale predecessor revision.
pub(crate) fn revision_conflict(expected_revision: i32, current_revision: i32) -> CommerceError {
    CommerceError::Validation(format!(
        "{REVISION_CONFLICT_PREFIX}: expected revision {expected_revision}, \
         current revision {current_revision}"
    ))
}

/// Recognizes the reserved refusal and returns its facts.
pub(crate) fn revision_conflict_of(message: &str) -> Option<RevisionConflictFacts> {
    let rest = message.strip_prefix(REVISION_CONFLICT_PREFIX)?;
    let rest = rest.strip_prefix(": expected revision ")?;
    let (expected, rest) = rest.split_once(", current revision ")?;
    Some(RevisionConflictFacts {
        expected_revision: expected.parse().ok()?,
        current_revision: rest.parse().ok()?,
    })
}

/// Returns the next aggregate revision, refusing an exhausted counter.
///
/// Published for the host-side compatibility writers that still mutate the `products` row
/// directly (`apps/server` Flex attached values). Those writers must bump the revision in the same
/// transaction as their document write, otherwise a predecessor revision read by an operator would
/// stay valid across their change.
pub fn next_revision(current_revision: i32) -> CommerceResult<i32> {
    current_revision
        .checked_add(1)
        .filter(|next| *next > 0)
        .ok_or_else(|| {
            CommerceError::Validation(
                "Product revision is exhausted; the product can no longer be edited".to_owned(),
            )
        })
}

/// Enforces the caller's predecessor revision against the locked aggregate row.
pub(crate) fn ensure_expected_revision(
    product: &entities::product::Model,
    expected_revision: Option<i32>,
) -> CommerceResult<()> {
    let Some(expected_revision) = expected_revision else {
        return Ok(());
    };
    if product.revision != expected_revision {
        return Err(revision_conflict(expected_revision, product.revision));
    }
    Ok(())
}

/// Bounded length check for one `Patch::Set` text value.
///
/// `Patch` deliberately has no `validator` integration, so the migrated fields are checked here
/// before the transaction opens, with the same limits the previous `#[validate]` attributes used.
pub(crate) fn validate_patch_text_length(
    patch: &rustok_api::Patch<String>,
    max_characters: usize,
    field: &'static str,
) -> CommerceResult<()> {
    if let rustok_api::Patch::Set(value) = patch
        && value.chars().count() > max_characters
    {
        return Err(CommerceError::Validation(format!(
            "{field} must be max {max_characters} characters"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        REVISION_CONFLICT_PREFIX, ensure_expected_revision, next_revision, revision_conflict,
        revision_conflict_of, validate_patch_text_length,
    };
    use crate::entities;
    use rustok_api::Patch;

    fn product(revision: i32) -> entities::product::Model {
        entities::product::Model {
            id: uuid::Uuid::from_u128(1),
            tenant_id: uuid::Uuid::from_u128(2),
            status: entities::product::ProductStatus::Draft,
            seller_id: None,
            vendor: None,
            product_type: None,
            shipping_profile_slug: None,
            primary_category_id: None,
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
            published_at: None,
            revision,
        }
    }

    #[test]
    fn predecessor_revision_round_trips_through_the_reserved_message() {
        let error = revision_conflict(4, 7);
        let message = error.to_string();
        assert!(message.contains(REVISION_CONFLICT_PREFIX));

        let facts = revision_conflict_of(&message).expect("reserved facts");
        assert_eq!(facts.expected_revision, 4);
        assert_eq!(facts.current_revision, 7);
        assert!(revision_conflict_of("Product request is invalid").is_none());
    }

    #[test]
    fn stale_predecessor_is_refused_and_matching_predecessor_is_accepted() {
        assert!(ensure_expected_revision(&product(3), Some(3)).is_ok());
        assert!(ensure_expected_revision(&product(3), None).is_ok());

        let error = ensure_expected_revision(&product(3), Some(2)).expect_err("stale predecessor");
        assert_eq!(
            revision_conflict_of(&error.to_string())
                .expect("facts")
                .current_revision,
            3
        );
    }

    #[test]
    fn exhausted_revision_counter_is_refused() {
        assert_eq!(next_revision(1).expect("next"), 2);
        assert!(next_revision(i32::MAX).is_err());
        assert!(next_revision(-1).is_err());
    }

    #[test]
    fn patch_text_length_is_bounded_only_for_set_values() {
        assert!(validate_patch_text_length(&Patch::Keep, 3, "Vendor").is_ok());
        assert!(validate_patch_text_length(&Patch::Clear, 3, "Vendor").is_ok());
        assert!(validate_patch_text_length(&Patch::Set("abc".to_owned()), 3, "Vendor").is_ok());
        assert!(validate_patch_text_length(&Patch::Set("abcd".to_owned()), 3, "Vendor").is_err());
    }
}
