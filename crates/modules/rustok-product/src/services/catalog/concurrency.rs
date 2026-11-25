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
    let message = message
        .strip_prefix("Product validation error: ")
        .unwrap_or(message);
    let rest = message.strip_prefix(REVISION_CONFLICT_PREFIX)?;
    let rest = rest.strip_prefix(": expected revision ")?;
    let (expected, current) = rest.split_once(", current revision ")?;
    let expected_revision = expected.parse::<i32>().ok()?;
    let current_revision = current.parse::<i32>().ok()?;
    // Invalid caller predecessors may be zero or negative. The stored
    // revision must be positive and a refusal must describe a mismatch.
    if current_revision <= 0
        || expected_revision == current_revision
        || expected != expected_revision.to_string()
        || current != current_revision.to_string()
    {
        return None;
    }
    Some(RevisionConflictFacts {
        expected_revision,
        current_revision,
    })
}

/// Returns the next aggregate revision, refusing an exhausted counter.
pub(crate) fn next_revision(current_revision: i32) -> CommerceResult<i32> {
    (current_revision > 0)
        .then(|| current_revision.checked_add(1))
        .flatten()
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


const VARIANT_REVISION_CONFLICT_PREFIX: &str = "product variant revision conflict";

pub(crate) fn expected_variant_revision(value: Option<&str>) -> CommerceResult<i64> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Err(CommerceError::Validation(
            "variant writes require the revision that was read".to_owned(),
        ));
    };
    if value.len() > 19
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(CommerceError::Validation(
            "variant revision must be a positive integer".to_owned(),
        ));
    }

    value
        .parse::<i64>()
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| {
            CommerceError::Validation("variant revision must be a positive integer".to_owned())
        })
}

pub(crate) fn ensure_expected_variant_revision(
    variant: &entities::product_variant::Model,
    expected_revision: Option<&str>,
) -> CommerceResult<()> {
    let expected = expected_variant_revision(expected_revision)?;
    if variant.index_revision != expected {
        return Err(CommerceError::Validation(format!(
            "{VARIANT_REVISION_CONFLICT_PREFIX}: expected revision {expected}, current revision {}",
            variant.index_revision
        )));
    }
    Ok(())
}

pub(crate) fn variant_revision_conflict_of(message: &str) -> bool {
    let message = message
        .strip_prefix("Product validation error: ")
        .unwrap_or(message);
    let Some(rest) = message.strip_prefix(VARIANT_REVISION_CONFLICT_PREFIX) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(": expected revision ") else {
        return false;
    };
    let Some((expected, current)) = rest.split_once(", current revision ") else {
        return false;
    };

    match (
        expected_variant_revision(Some(expected)),
        expected_variant_revision(Some(current)),
    ) {
        (Ok(expected), Ok(current)) => expected != current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        REVISION_CONFLICT_PREFIX, ensure_expected_revision, ensure_expected_variant_revision,
        expected_variant_revision, next_revision, revision_conflict, revision_conflict_of,
        validate_patch_text_length, variant_revision_conflict_of, CommerceError,
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
    fn product_revision_conflict_requires_an_exact_owner_refusal() {
        for message in [
            "user input: product revision conflict: expected revision 4, current revision 7",
            "Product validation error: user input: product revision conflict: expected revision 4, current revision 7",
            "product revision conflict: expected revision 4, current revision 7 trailing",
            "product revision conflict: expected revision 4, current revision 0",
            "product revision conflict: expected revision 7, current revision 7",
            "product revision conflict: expected revision 04, current revision 7",
        ] {
            assert!(revision_conflict_of(message).is_none(), "{message}");
            let public = crate::map_product_public_error(
                &CommerceError::Validation(message.to_owned()),
                "test",
                "test",
            );
            assert_eq!(public.code, "PRODUCT_VALIDATION");
        }
        for expected in [-1, 0, 4] {
            let error = revision_conflict(expected, 7);
            let facts = revision_conflict_of(&error.to_string()).expect("owner refusal");
            assert_eq!(facts.expected_revision, expected);
            assert_eq!(facts.current_revision, 7);
            assert_eq!(
                crate::map_product_public_error(&error, "test", "test").code,
                "PRODUCT_REVISION_CONFLICT"
            );
        }
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
    fn variant_revision_requires_a_positive_database_revision() {
        assert_eq!(expected_variant_revision(Some("7")).unwrap(), 7);
        assert!(expected_variant_revision(None).is_err());
        assert!(expected_variant_revision(Some("0")).is_err());
        assert!(expected_variant_revision(Some("07")).is_err());
        assert!(expected_variant_revision(Some("not-a-revision")).is_err());
    }

    fn variant(index_revision: i64) -> entities::product_variant::Model {
        let now = chrono::Utc::now().into();
        entities::product_variant::Model {
            id: uuid::Uuid::from_u128(1),
            tenant_id: uuid::Uuid::from_u128(2),
            product_id: uuid::Uuid::from_u128(3),
            sku: Some("SKU-1".to_owned()),
            barcode: None,
            ean: None,
            upc: None,
            weight: None,
            weight_unit: None,
            inventory_management: "manual".to_owned(),
            inventory_quantity: 0,
            inventory_policy: "deny".to_owned(),
            position: 0,
            combination_identity: None,
            shipping_profile_slug: None,
            created_at: now,
            updated_at: now,
            index_revision,
        }
    }

    #[test]
    fn stale_variant_revision_is_refused_and_current_revision_is_accepted() {
        assert!(ensure_expected_variant_revision(&variant(5), Some("5")).is_ok());

        let error = ensure_expected_variant_revision(&variant(5), Some("4"))
            .expect_err("stale variant predecessor");
        assert!(variant_revision_conflict_of(&error.to_string()));
        assert!(ensure_expected_variant_revision(&variant(5), None).is_err());
    }

    #[test]
    fn variant_revision_conflicts_require_complete_positive_revision_facts() {
        let error = CommerceError::Validation(
            "product variant revision conflict: expected revision 4, current revision 5".into(),
        );
        assert!(variant_revision_conflict_of(&error.to_string()));
        assert!(!variant_revision_conflict_of("ordinary validation error"));
        assert!(!variant_revision_conflict_of(
            "Product validation error: user text mentions product variant revision conflict"
        ));
        assert!(!variant_revision_conflict_of(
            "user value: product variant revision conflict: expected revision 4, current revision 5"
        ));
        assert!(!variant_revision_conflict_of(
            "product variant revision conflict: expected revision invalid, current revision 5"
        ));
        assert!(!variant_revision_conflict_of(
            "product variant revision conflict: expected revision 4, current revision 5 trailing"
        ));
        for message in [
            "product variant revision conflict: expected revision 5, current revision 5",
            "product variant revision conflict: expected revision 04, current revision 5",
            "product variant revision conflict: expected revision 4, current revision 05",
            "product variant revision conflict: expected revision +4, current revision 5",
            "product variant revision conflict: expected revision 4, current revision +5",
            "product variant revision conflict: expected revision 0, current revision 5",
            "product variant revision conflict: expected revision 4, current revision 0",
            "product variant revision conflict: expected revision 9223372036854775808, current revision 5",
        ] {
            assert!(!variant_revision_conflict_of(message), "{message}");
            let public = crate::map_product_public_error(
                &CommerceError::Validation(message.to_owned()),
                "test",
                "test",
            );
            assert_eq!(public.code, "PRODUCT_VALIDATION", "{message}");
        }
        assert!(variant_revision_conflict_of(
            "product variant revision conflict: expected revision 9223372036854775807, current revision 5"
        ));
    }

    #[test]
    fn exhausted_revision_counter_is_refused() {
        assert_eq!(next_revision(1).expect("next"), 2);
        assert!(next_revision(i32::MAX).is_err());
        assert!(next_revision(-1).is_err());
        assert!(next_revision(0).is_err());
    }

    #[test]
    fn patch_text_length_is_bounded_only_for_set_values() {
        assert!(validate_patch_text_length(&Patch::Keep, 3, "Vendor").is_ok());
        assert!(validate_patch_text_length(&Patch::Clear, 3, "Vendor").is_ok());
        assert!(validate_patch_text_length(&Patch::Set("abc".to_owned()), 3, "Vendor").is_ok());
        assert!(validate_patch_text_length(&Patch::Set("abcd".to_owned()), 3, "Vendor").is_err());
    }
}
