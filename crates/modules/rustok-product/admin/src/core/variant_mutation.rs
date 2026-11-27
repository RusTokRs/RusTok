/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Durable caller recovery for existing Product Variant updates and deletions.
//!
//! The journal freezes the exact sparse update payload or delete revision before transport.
//! Unknown outcomes can only replay that immutable request with its original caller key.

use crate::core::write_journal::{
    JournalState, ProductWriteScope, RecoveryQueryKey, WriteJournal, WriteJournalError, valid_uuid,
};
use crate::model::VariantUpdateDraft;
use rustok_product_ports::MAX_VARIANT_PRICE_ROWS;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

const MAX_VARIANT_UPDATE_REQUEST_BYTES: usize = 256 * 1024;

pub(crate) fn valid_revision(value: &str) -> bool {
    value.len() <= 20
        && value
            .parse::<u64>()
            .is_ok_and(|revision| revision > 0 && revision.to_string() == value)
}

fn valid_update_request(request: &str) -> bool {
    if request.is_empty() || request.len() > MAX_VARIANT_UPDATE_REQUEST_BYTES {
        return false;
    }
    let Ok(draft) = serde_json::from_str::<VariantUpdateDraft>(request) else {
        return false;
    };
    if serde_json::to_string(&draft).ok().as_deref() != Some(request) {
        return false;
    }
    if !draft.has_changes()
        || draft
            .expected_revision
            .as_deref()
            .is_none_or(|revision| !valid_revision(revision))
        || crate::core::validate_editor_variant_draft(&draft).is_err()
        || draft.inventory_quantity.is_some() != draft.expected_inventory_quantity.is_some()
        || draft
            .inventory_quantity
            .is_some_and(|quantity| quantity < 0)
        || draft
            .expected_inventory_quantity
            .is_some_and(|quantity| quantity < 0)
        || draft.inventory_policy.as_deref().is_some_and(|policy| {
            !policy.eq_ignore_ascii_case("deny") && !policy.eq_ignore_ascii_case("continue")
        })
        || draft.prices.len() > MAX_VARIANT_PRICE_ROWS
        || (!draft.prices_changed && !draft.prices.is_empty())
        || (draft.prices_changed
            && draft.prices.iter().any(|price| {
                price.currency_code.len() != 3
                    || !price
                        .currency_code
                        .bytes()
                        .all(|byte| byte.is_ascii_alphabetic())
            }))
        || draft.axis_values.len() > 32
    {
        return false;
    }

    let mut attributes = HashSet::new();
    draft.axis_values.iter().all(|value| {
        valid_uuid(&value.attribute_id)
            && valid_uuid(&value.option_id)
            && attributes.insert(value.attribute_id.as_str())
    })
}

fn valid_refusal_code(code: &str) -> bool {
    matches!(
        code,
        "duplicate_sku" | "conflict" | "validation" | "not_found" | "inventory_blocked"
    )
}

/// Only closed, deterministic owner decisions settle a pending caller request.
/// Invariant failures and unavailable provider state deliberately remain unknown.
pub(crate) fn normalized_refusal(code: &str) -> Option<&'static str> {
    match code {
        "DUPLICATE_SKU" | "product.duplicate_sku" => Some("duplicate_sku"),
        "PRODUCT_NOT_FOUND"
        | "PRODUCT_VARIANT_NOT_FOUND"
        | "PRODUCT_DEPENDENCY_NOT_FOUND"
        | "product.product_not_found"
        | "product.variant_not_found"
        | "product.dependency_not_found" => Some("not_found"),
        "PRODUCT_VALIDATION"
        | "PRODUCT_INVENTORY_DELETION_INVALID"
        | "PRODUCT_INVENTORY_DELETION_LIMIT"
        | "product.validation" => Some("validation"),
        "PRODUCT_CONFLICT"
        | "PRODUCT_REVISION_CONFLICT"
        | "CANNOT_DELETE_ONLY_VARIANT"
        | "PRODUCT_VARIANT_AXIS_POLICY_CONFLICT"
        | "product.conflict" => Some("conflict"),
        "PRODUCT_INVENTORY_DELETION_HISTORY_PRESENT"
        | "PRODUCT_INVENTORY_DELETION_STOCK_PRESENT"
        | "PRODUCT_INVENTORY_DELETION_CHANGED"
        | "PRODUCT_INVENTORY_QUANTITY_LOCATION_UNAVAILABLE"
        | "PRODUCT_INVENTORY_QUANTITY_INSUFFICIENT"
        | "PRODUCT_INVENTORY_QUANTITY_CHANGED"
        | "PRODUCT_INVENTORY_QUANTITY_HISTORY_PRESENT"
        | "product.inventory_quantity_location_unavailable"
        | "product.inventory_quantity_insufficient"
        | "product.inventory_quantity_changed"
        | "product.inventory_quantity_history_present" => Some("inventory_blocked"),
        "PRODUCT_INVENTORY_QUANTITY_INVALID"
        | "PRODUCT_INVENTORY_QUANTITY_LIMIT"
        | "product.inventory_quantity_invalid"
        | "product.inventory_quantity_limit" => Some("validation"),
        _ => None,
    }
}

pub(crate) fn refusal_key(code: &str) -> &'static str {
    match code {
        "duplicate_sku" => "product-variantMutation-duplicateSku",
        "not_found" => "product-variantMutation-notFound",
        "conflict" => "product-variantMutation-conflict",
        "inventory_blocked" => "product-variantMutation-inventoryBlocked",
        _ => "product-variantMutation-invalid",
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum VariantMutationPhase {
    Ready,
    PendingUpdate {
        key: String,
        variant_id: String,
        request: String,
    },
    ConfirmedUpdate {
        key: String,
        variant_id: String,
        request: String,
        revision: String,
    },
    RejectedUpdate {
        key: String,
        variant_id: String,
        request: String,
        code: String,
    },
    PendingDelete {
        key: String,
        variant_id: String,
        expected_revision: String,
    },
    ConfirmedDelete {
        key: String,
        variant_id: String,
        expected_revision: String,
    },
    RejectedDelete {
        key: String,
        variant_id: String,
        expected_revision: String,
        code: String,
    },
}

impl VariantMutationPhase {
    pub fn is_pending(&self) -> bool {
        matches!(
            self,
            Self::PendingUpdate { .. } | Self::PendingDelete { .. }
        )
    }

    pub fn pending_update(&self) -> Option<(String, String, VariantUpdateDraft)> {
        let Self::PendingUpdate {
            key,
            variant_id,
            request,
        } = self
        else {
            return None;
        };
        Some((
            key.clone(),
            variant_id.clone(),
            serde_json::from_str(request).ok()?,
        ))
    }

    pub fn pending_delete(&self) -> Option<(String, String, String)> {
        let Self::PendingDelete {
            key,
            variant_id,
            expected_revision,
        } = self
        else {
            return None;
        };
        Some((key.clone(), variant_id.clone(), expected_revision.clone()))
    }
}

impl JournalState for VariantMutationPhase {
    type Scope = ProductWriteScope;
    const STORAGE_PREFIX: &'static str = "rustok.product.variant_mutation_attempt";
    const DOCUMENT_QUERY: RecoveryQueryKey = RecoveryQueryKey::VariantMutation;

    fn ready() -> Self {
        Self::Ready
    }

    fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::ConfirmedUpdate { .. }
                | Self::RejectedUpdate { .. }
                | Self::ConfirmedDelete { .. }
                | Self::RejectedDelete { .. }
        )
    }

    fn valid(&self) -> bool {
        match self {
            Self::Ready => true,
            Self::PendingUpdate {
                key,
                variant_id,
                request,
            }
            | Self::ConfirmedUpdate {
                key,
                variant_id,
                request,
                ..
            }
            | Self::RejectedUpdate {
                key,
                variant_id,
                request,
                ..
            } => {
                valid_uuid(key)
                    && valid_uuid(variant_id)
                    && valid_update_request(request)
                    && match self {
                        Self::ConfirmedUpdate { revision, .. } => valid_revision(revision),
                        Self::RejectedUpdate { code, .. } => valid_refusal_code(code),
                        _ => true,
                    }
            }
            Self::PendingDelete {
                key,
                variant_id,
                expected_revision,
            }
            | Self::ConfirmedDelete {
                key,
                variant_id,
                expected_revision,
            }
            | Self::RejectedDelete {
                key,
                variant_id,
                expected_revision,
                ..
            } => {
                valid_uuid(key)
                    && valid_uuid(variant_id)
                    && valid_revision(expected_revision)
                    && match self {
                        Self::RejectedDelete { code, .. } => valid_refusal_code(code),
                        _ => true,
                    }
            }
        }
    }

    fn can_follow(&self, next: &Self) -> bool {
        match (self, next) {
            (Self::Ready, Self::PendingUpdate { .. } | Self::PendingDelete { .. }) => true,
            (
                Self::PendingUpdate {
                    key: old_key,
                    variant_id: old_variant,
                    request: old_request,
                },
                Self::ConfirmedUpdate {
                    key,
                    variant_id,
                    request,
                    ..
                }
                | Self::RejectedUpdate {
                    key,
                    variant_id,
                    request,
                    ..
                },
            ) => old_key == key && old_variant == variant_id && old_request == request,
            (
                Self::PendingDelete {
                    key: old_key,
                    variant_id: old_variant,
                    expected_revision: old_revision,
                },
                Self::ConfirmedDelete {
                    key,
                    variant_id,
                    expected_revision,
                }
                | Self::RejectedDelete {
                    key,
                    variant_id,
                    expected_revision,
                    ..
                },
            ) => old_key == key && old_variant == variant_id && old_revision == expected_revision,
            _ => false,
        }
    }
}

pub(crate) type VariantMutationRecord = WriteJournal<VariantMutationPhase>;

impl WriteJournal<VariantMutationPhase> {
    pub fn begin_update(
        &self,
        variant_id: String,
        draft: VariantUpdateDraft,
    ) -> Result<Self, WriteJournalError> {
        let request = serde_json::to_string(&draft).map_err(|_| WriteJournalError::Invalid)?;
        if !self.state.is_ready() || !valid_uuid(&variant_id) || !valid_update_request(&request) {
            return Err(WriteJournalError::Changed);
        }
        let mut next = self.clone();
        next.state = VariantMutationPhase::PendingUpdate {
            key: Uuid::new_v4().to_string(),
            variant_id,
            request,
        };
        next.encode()?;
        Ok(next)
    }

    pub fn begin_delete(
        &self,
        variant_id: String,
        expected_revision: String,
    ) -> Result<Self, WriteJournalError> {
        if !self.state.is_ready() || !valid_uuid(&variant_id) || !valid_revision(&expected_revision)
        {
            return Err(WriteJournalError::Changed);
        }
        let mut next = self.clone();
        next.state = VariantMutationPhase::PendingDelete {
            key: Uuid::new_v4().to_string(),
            variant_id,
            expected_revision,
        };
        next.encode()?;
        Ok(next)
    }

    pub fn confirm_update(&self, revision: String) -> Result<Self, WriteJournalError> {
        if !valid_revision(&revision) {
            return Err(WriteJournalError::Invalid);
        }
        let VariantMutationPhase::PendingUpdate {
            key,
            variant_id,
            request,
        } = &self.state
        else {
            return Err(WriteJournalError::Changed);
        };
        let mut next = self.clone();
        next.state = VariantMutationPhase::ConfirmedUpdate {
            key: key.clone(),
            variant_id: variant_id.clone(),
            request: request.clone(),
            revision,
        };
        next.encode()?;
        Ok(next)
    }

    pub fn confirm_delete(&self) -> Result<Self, WriteJournalError> {
        let VariantMutationPhase::PendingDelete {
            key,
            variant_id,
            expected_revision,
        } = &self.state
        else {
            return Err(WriteJournalError::Changed);
        };
        let mut next = self.clone();
        next.state = VariantMutationPhase::ConfirmedDelete {
            key: key.clone(),
            variant_id: variant_id.clone(),
            expected_revision: expected_revision.clone(),
        };
        next.encode()?;
        Ok(next)
    }

    pub fn reject(&self, code: String) -> Result<Self, WriteJournalError> {
        if !valid_refusal_code(&code) {
            return Err(WriteJournalError::Invalid);
        }
        let mut next = self.clone();
        next.state = match &self.state {
            VariantMutationPhase::PendingUpdate {
                key,
                variant_id,
                request,
            } => VariantMutationPhase::RejectedUpdate {
                key: key.clone(),
                variant_id: variant_id.clone(),
                request: request.clone(),
                code,
            },
            VariantMutationPhase::PendingDelete {
                key,
                variant_id,
                expected_revision,
            } => VariantMutationPhase::RejectedDelete {
                key: key.clone(),
                variant_id: variant_id.clone(),
                expected_revision: expected_revision.clone(),
                code,
            },
            _ => return Err(WriteJournalError::Changed),
        };
        next.encode()?;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::Patch;

    fn id(value: u128) -> String {
        Uuid::from_u128(value).to_string()
    }

    fn scope() -> ProductWriteScope {
        ProductWriteScope {
            tenant_id: id(1),
            actor_id: id(2),
            product_id: id(3),
            locale: "en".into(),
        }
    }

    fn draft() -> VariantUpdateDraft {
        VariantUpdateDraft {
            sku: Patch::Set("SKU-2".into()),
            expected_revision: Some("7".into()),
            ..Default::default()
        }
    }

    #[test]
    fn update_journal_round_trips_the_exact_request_and_caller_key() {
        let ready = WriteJournal::<VariantMutationPhase>::new(scope()).unwrap();
        let pending = ready.begin_update(id(4), draft()).unwrap();
        let (key, variant_id, recovered) = pending.state.pending_update().unwrap();

        assert_eq!(variant_id, id(4));
        assert_eq!(
            serde_json::to_string(&recovered).unwrap(),
            match &pending.state {
                VariantMutationPhase::PendingUpdate { request, .. } => request.clone(),
                _ => unreachable!(),
            }
        );
        assert_eq!(
            WriteJournal::<VariantMutationPhase>::decode(&pending.encode().unwrap(), &scope())
                .unwrap(),
            pending
        );
        assert!(valid_uuid(&key));
    }

    #[test]
    fn update_terminal_evidence_must_match_the_pending_key_variant_and_body() {
        let ready = WriteJournal::<VariantMutationPhase>::new(scope()).unwrap();
        let pending = ready.begin_update(id(4), draft()).unwrap();
        let confirmed = pending.confirm_update("8".into()).unwrap();

        assert!(pending.can_transition_to(&confirmed));
        let mut tampered = confirmed.clone();
        if let VariantMutationPhase::ConfirmedUpdate { request, .. } = &mut tampered.state {
            request.push(' ');
        }
        assert!(!pending.can_transition_to(&tampered));
        assert!(!tampered.state.valid());
    }

    #[test]
    fn delete_journal_freezes_the_expected_variant_revision() {
        let ready = WriteJournal::<VariantMutationPhase>::new(scope()).unwrap();
        let pending = ready.begin_delete(id(4), "7".into()).unwrap();
        let (_, variant_id, revision) = pending.state.pending_delete().unwrap();
        let confirmed = pending.confirm_delete().unwrap();

        assert_eq!(variant_id, id(4));
        assert_eq!(revision, "7");
        assert!(pending.can_transition_to(&confirmed));
    }

    #[test]
    fn invalid_or_nonterminal_requests_cannot_be_admitted_or_settled() {
        let ready = WriteJournal::<VariantMutationPhase>::new(scope()).unwrap();
        assert!(
            ready
                .begin_update(id(4), VariantUpdateDraft::default())
                .is_err()
        );
        assert!(ready.begin_delete(id(4), "0".into()).is_err());

        let pending = ready.begin_update(id(4), draft()).unwrap();
        assert!(pending.confirm_update("0".into()).is_err());
        assert!(pending.reject("provider_unavailable".into()).is_err());
    }

    #[test]
    fn inventory_quantity_refusals_match_the_owner_terminal_classes() {
        for (code, expected) in [
            ("PRODUCT_INVENTORY_QUANTITY_INVALID", Some("validation")),
            ("product.inventory_quantity_invalid", Some("validation")),
            ("PRODUCT_INVENTORY_QUANTITY_LIMIT", Some("validation")),
            ("product.inventory_quantity_limit", Some("validation")),
            (
                "PRODUCT_INVENTORY_QUANTITY_LOCATION_UNAVAILABLE",
                Some("inventory_blocked"),
            ),
            (
                "product.inventory_quantity_location_unavailable",
                Some("inventory_blocked"),
            ),
            (
                "PRODUCT_INVENTORY_QUANTITY_INSUFFICIENT",
                Some("inventory_blocked"),
            ),
            (
                "product.inventory_quantity_insufficient",
                Some("inventory_blocked"),
            ),
            (
                "PRODUCT_INVENTORY_QUANTITY_CHANGED",
                Some("inventory_blocked"),
            ),
            (
                "product.inventory_quantity_changed",
                Some("inventory_blocked"),
            ),
            (
                "PRODUCT_INVENTORY_QUANTITY_HISTORY_PRESENT",
                Some("inventory_blocked"),
            ),
            (
                "product.inventory_quantity_history_present",
                Some("inventory_blocked"),
            ),
        ] {
            assert_eq!(normalized_refusal(code), expected, "{code}");
        }
    }

    #[test]
    fn invariant_and_unknown_owner_codes_never_settle_the_request() {
        for code in [
            "PRODUCT_INVENTORY_DELETION_STATE_UNVERIFIED",
            "PRODUCT_INVENTORY_QUANTITY_IDENTITY_UNAVAILABLE",
            "PRODUCT_INVENTORY_QUANTITY_STATE_UNVERIFIED",
            "PRODUCT_INVENTORY_QUANTITY_UNREPRESENTABLE",
            "product.inventory_quantity_unrepresentable",
            "PRODUCT_INVENTORY_QUANTITY_ISOLATION_UNSUPPORTED",
            "PRODUCT_INVENTORY_QUANTITY_EVENT_UNREPRESENTABLE",
            "PRODUCT_INVENTORY_QUANTITY_PUBLICATION_UNVERIFIED",
            "PRODUCT_TEMPORARILY_UNAVAILABLE",
        ] {
            assert_eq!(normalized_refusal(code), None, "{code}");
        }
    }
}
