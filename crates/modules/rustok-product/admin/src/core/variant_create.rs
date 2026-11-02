/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Standalone SKU request preparation and immutable recovery, independent of mounted controls
//! or a current Product read. Only transport adapters may supply terminal owner evidence.

use super::initial_catalog;
use super::write_journal::{
    JournalState, RecoveryQueryKey, WriteJournal, WriteJournalError, valid_uuid,
};
use crate::model::{ProductDetail, VariantCreateOutcome, VariantDraft, VariantPriceDraft};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

pub(crate) const MAX_PRICE_ROWS: usize = 64;

pub(crate) fn empty_draft(currency: &str) -> VariantDraft {
    VariantDraft {
        prices: vec![VariantPriceDraft {
            currency_code: currency.to_string(),
            amount: String::new(),
            compare_at_amount: None,
        }],
        inventory_quantity: Some(0),
        inventory_policy: Some("deny".into()),
        ..Default::default()
    }
}

pub(crate) fn can_create(product: &ProductDetail) -> bool {
    product.variants.len() < 256
        && (!product.variant_axes.is_empty() || product.variants.is_empty())
}

/// Normalize exactly once, before Pending is stored. Replay never uses this function or newer
/// axis metadata. Blank editor-only price rows mean no price; an orphan compare-at refuses.
pub(crate) fn prepare(
    mut draft: VariantDraft,
    quantity: &str,
    product: &ProductDetail,
) -> Result<VariantDraft, String> {
    if !can_create(product) {
        return Err("This Product cannot accept another SKU in this editor".into());
    }
    draft.inventory_quantity = Some(initial_catalog::parse_inventory_quantity(quantity)?);
    if draft.prices.len() > MAX_PRICE_ROWS {
        return Err("Review the supplied price rows".into());
    }
    for row in &mut draft.prices {
        row.compare_at_amount = row.compare_at_amount.take().and_then(super::text_or_none);
        if row.amount.trim().is_empty() && row.compare_at_amount.is_some() {
            return Err("A compare-at price requires an amount".into());
        }
    }
    draft.prices.retain(|row| !row.amount.trim().is_empty());
    draft.shipping_profile_slug = draft.shipping_profile_slug.and_then(super::text_or_none);
    let draft = initial_catalog::normalize_standalone_variant(draft, &product.variant_axes)?;
    if !valid_input(&draft) {
        return Err("Review the full SKU request before creating it".into());
    }
    Ok(draft)
}

fn optional_short(value: &Option<String>) -> bool {
    value
        .as_ref()
        .is_none_or(|text| text.chars().count() <= 100)
}

fn valid_amount(value: &str) -> bool {
    value.len() <= 64
        && value.trim() == value
        && initial_catalog::validate_amount(value).is_ok()
        && rust_decimal::Decimal::from_str_exact(value)
            .is_ok_and(|amount| amount.normalize().to_string() == value)
}

pub(crate) fn valid_input(input: &VariantDraft) -> bool {
    let mut attributes = HashSet::new();
    optional_short(&input.sku)
        && optional_short(&input.barcode)
        && optional_short(&input.shipping_profile_slug)
        && input.axis_values.len() <= 32
        && input.axis_values.iter().all(|value| {
            valid_uuid(&value.attribute_id)
                && valid_uuid(&value.option_id)
                && attributes.insert(value.attribute_id.as_str())
        })
        && input
            .inventory_quantity
            .is_some_and(|quantity| quantity >= 0)
        && matches!(input.inventory_policy.as_deref(), Some("deny" | "continue"))
        && input.prices.len() <= MAX_PRICE_ROWS
        && input.prices.iter().all(|price| {
            price.currency_code.len() == 3
                && price
                    .currency_code
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase())
                && valid_amount(&price.amount)
                && price
                    .compare_at_amount
                    .as_ref()
                    .is_none_or(|amount| valid_amount(amount))
        })
}

/// Both transports normalize only explicit durable owner codes, never arbitrary error text.
pub(crate) fn normalized_refusal(code: &str) -> Option<&'static str> {
    match code {
        "DUPLICATE_SKU" | "product.duplicate_sku" => Some("duplicate_sku"),
        "PRODUCT_CONFLICT" | "product.conflict" => Some("conflict"),
        "PRODUCT_VALIDATION" | "product.validation" | "product.schema_validation" => {
            Some("validation")
        }
        "PRODUCT_VARIANT_AXIS_POLICY_CONFLICT" | "product.variant_axis_policy_conflict" => {
            Some("policy_conflict")
        }
        "PRODUCT_NOT_FOUND"
        | "PRODUCT_DEPENDENCY_NOT_FOUND"
        | "PRODUCT_VARIANT_NOT_FOUND"
        | "product.product_not_found"
        | "product.dependency_not_found"
        | "product.variant_not_found" => Some("not_found"),
        _ => None,
    }
}

pub(crate) fn refusal_key(code: &str) -> &'static str {
    match code {
        "duplicate_sku" => "product-variantCreate-duplicateSku",
        "not_found" => "product-variantCreate-notFound",
        "conflict" | "policy_conflict" => "product-variantCreate-conflict",
        _ => "product-variantCreate-invalid",
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum VariantCreatePhase {
    Ready,
    Pending {
        key: String,
        input: VariantDraft,
    },
    Confirmed {
        key: String,
        input: VariantDraft,
        variant_id: String,
    },
    Rejected {
        key: String,
        input: VariantDraft,
        code: String,
    },
}

impl VariantCreatePhase {
    pub fn input(&self) -> Option<&VariantDraft> {
        match self {
            Self::Ready => None,
            Self::Pending { input, .. }
            | Self::Confirmed { input, .. }
            | Self::Rejected { input, .. } => Some(input),
        }
    }
}

impl JournalState for VariantCreatePhase {
    type Scope = super::write_journal::ProductWriteScope;
    const STORAGE_PREFIX: &'static str = "rustok.product.variant_create_attempt";
    const DOCUMENT_QUERY: RecoveryQueryKey = RecoveryQueryKey::VariantCreate;

    fn ready() -> Self {
        Self::Ready
    }
    fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
    fn is_terminal(&self) -> bool {
        matches!(self, Self::Confirmed { .. } | Self::Rejected { .. })
    }
    fn valid(&self) -> bool {
        match self {
            Self::Ready => true,
            Self::Pending { key, input } => valid_uuid(key) && valid_input(input),
            Self::Confirmed {
                key,
                input,
                variant_id,
            } => valid_uuid(key) && valid_input(input) && valid_uuid(variant_id),
            Self::Rejected { key, input, code } => {
                valid_uuid(key)
                    && valid_input(input)
                    && matches!(
                        code.as_str(),
                        "duplicate_sku"
                            | "conflict"
                            | "validation"
                            | "policy_conflict"
                            | "not_found"
                    )
            }
        }
    }
    fn can_follow(&self, next: &Self) -> bool {
        match (self, next) {
            (Self::Ready, Self::Pending { .. }) => true,
            (
                Self::Pending {
                    key: old,
                    input: saved,
                },
                Self::Confirmed { key, input, .. },
            )
            | (
                Self::Pending {
                    key: old,
                    input: saved,
                },
                Self::Rejected { key, input, .. },
            ) => old == key && saved == input,
            _ => false,
        }
    }
}

pub(crate) type VariantCreateRecord = WriteJournal<VariantCreatePhase>;

impl WriteJournal<VariantCreatePhase> {
    pub fn begin(&self, input: VariantDraft, locale: String) -> Result<Self, WriteJournalError> {
        if !self.state.is_ready() || !valid_input(&input) {
            return Err(WriteJournalError::Changed);
        }
        let mut next = self.clone();
        next.scope.locale = locale;
        next.state = VariantCreatePhase::Pending {
            key: Uuid::new_v4().to_string(),
            input,
        };
        next.encode()?;
        Ok(next)
    }

    pub fn settle(&self, outcome: VariantCreateOutcome) -> Result<Self, WriteJournalError> {
        let VariantCreatePhase::Pending { key, input } = &self.state else {
            return Err(WriteJournalError::Changed);
        };
        let mut next = self.clone();
        next.state = match outcome {
            VariantCreateOutcome::Confirmed { variant_id } => VariantCreatePhase::Confirmed {
                key: key.clone(),
                input: input.clone(),
                variant_id,
            },
            VariantCreateOutcome::Rejected { code } => VariantCreatePhase::Rejected {
                key: key.clone(),
                input: input.clone(),
                code,
            },
            VariantCreateOutcome::Unknown => return Err(WriteJournalError::Changed),
        };
        next.encode()?;
        Ok(next)
    }
}

/// Owner-local form-field projection; DOM bindings do not normalize or republish on mount.
#[derive(Clone, Copy)]
pub(crate) enum CreateField {
    Sku,
    Barcode,
    ShippingProfile,
    Currency(usize),
    Amount(usize),
    CompareAt(usize),
}
impl CreateField {
    pub fn message_key(self) -> &'static str {
        match self {
            Self::Sku => "product-variantCreate-sku",
            Self::Barcode => "product-variantCreate-barcode",
            Self::ShippingProfile => "product-variantCreate-shippingProfile",
            Self::Currency(_) => "product-variantCreate-currency",
            Self::Amount(_) => "product-variantCreate-amount",
            Self::CompareAt(_) => "product-variantCreate-compareAt",
        }
    }
    pub fn value(self, draft: &VariantDraft) -> String {
        match self {
            Self::Sku => draft.sku.clone().unwrap_or_default(),
            Self::Barcode => draft.barcode.clone().unwrap_or_default(),
            Self::ShippingProfile => draft.shipping_profile_slug.clone().unwrap_or_default(),
            Self::Currency(index) => draft
                .prices
                .get(index)
                .map(|row| row.currency_code.clone())
                .unwrap_or_default(),
            Self::Amount(index) => draft
                .prices
                .get(index)
                .map(|row| row.amount.clone())
                .unwrap_or_default(),
            Self::CompareAt(index) => draft
                .prices
                .get(index)
                .and_then(|row| row.compare_at_amount.clone())
                .unwrap_or_default(),
        }
    }
    pub fn set(self, draft: &mut VariantDraft, value: String) {
        match self {
            Self::Sku => draft.sku = Some(value),
            Self::Barcode => draft.barcode = Some(value),
            Self::ShippingProfile => draft.shipping_profile_slug = Some(value),
            Self::Currency(index) => {
                if let Some(row) = draft.prices.get_mut(index) {
                    row.currency_code = value;
                }
            }
            Self::Amount(index) => {
                if let Some(row) = draft.prices.get_mut(index) {
                    row.amount = value;
                }
            }
            Self::CompareAt(index) => {
                if let Some(row) = draft.prices.get_mut(index) {
                    row.compare_at_amount = Some(value);
                }
            }
        }
    }
}

pub(crate) fn assignment_ids(product: Option<&ProductDetail>, draft: &VariantDraft) -> Vec<String> {
    let mut ids: Vec<String> = product
        .map(|product| {
            product
                .variant_axes
                .iter()
                .map(|axis| axis.attribute_id.clone())
                .collect()
        })
        .unwrap_or_default();
    for value in &draft.axis_values {
        if !ids.contains(&value.attribute_id) {
            ids.push(value.attribute_id.clone());
        }
    }
    ids
}

pub(crate) fn assignment_choices(
    product: Option<&ProductDetail>,
    draft: &VariantDraft,
    attribute_id: &str,
) -> (String, Vec<super::variant_axes::AxisOption>) {
    let axis = product.and_then(|product| {
        product
            .variant_axes
            .iter()
            .find(|axis| axis.attribute_id == attribute_id)
    });
    let label = axis
        .map(|axis| axis.name.clone())
        .unwrap_or_else(|| attribute_id.to_string());
    let mut options: Vec<super::variant_axes::AxisOption> = axis
        .map(|axis| {
            axis.allowed_values
                .iter()
                .map(|option| super::variant_axes::AxisOption {
                    id: option.option_id.clone(),
                    label: option.value.clone(),
                    available: true,
                })
                .collect()
        })
        .unwrap_or_default();
    for value in &draft.axis_values {
        if value.attribute_id == attribute_id
            && !options.iter().any(|option| option.id == value.option_id)
        {
            options.push(super::variant_axes::AxisOption {
                id: value.option_id.clone(),
                label: value.option_id.clone(),
                available: false,
            });
        }
    }
    (label, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::write_journal::ProductWriteScope;
    use crate::model::VariantAxisValueDraft;

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
    fn input() -> VariantDraft {
        VariantDraft {
            sku: Some("FULL-REQUEST".into()),
            barcode: Some("1234".into()),
            shipping_profile_slug: Some("standard".into()),
            axis_values: vec![VariantAxisValueDraft {
                attribute_id: id(4),
                option_id: id(5),
            }],
            prices: vec![VariantPriceDraft {
                currency_code: "USD".into(),
                amount: "12.000001".into(),
                compare_at_amount: Some("14".into()),
            }],
            inventory_quantity: Some(17),
            inventory_policy: Some("continue".into()),
        }
    }

    #[test]
    fn frozen_prices_must_be_exactly_representable_and_within_the_editor_bound() {
        for amount in [
            "0.00000000000000000000000000001",
            "79228162514264337593543950336",
            "1.0",
            " 1",
            "-1",
        ] {
            let mut draft = input();
            draft.prices[0].amount = amount.into();
            assert!(!valid_input(&draft));
        }
        let mut draft = input();
        draft.prices[0].amount = "0".into();
        assert!(valid_input(&draft));
        draft.prices = vec![draft.prices[0].clone(); MAX_PRICE_ROWS + 1];
        assert!(!valid_input(&draft));
        assert!(initial_catalog::validate_amount("0.00000000000000000000000000001").is_err());
    }

    #[test]
    fn metadata_absence_retains_unavailable_tuple_ids_in_the_control_projection() {
        let draft = input();
        assert_eq!(assignment_ids(None, &draft), vec![id(4)]);
        let (_, choices) = assignment_choices(None, &draft, &id(4));
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].id, id(5));
        assert!(!choices[0].available);
        assert_eq!(draft, input());
    }

    #[test]
    fn reload_retains_every_field_key_and_original_locale_without_a_product_read() {
        let ready = VariantCreateRecord::new(scope()).unwrap();
        let pending = ready.begin(input(), "en".into()).unwrap();
        let mut changed_locale = scope();
        changed_locale.locale = "ru".into();
        let restored =
            VariantCreateRecord::decode(&pending.encode().unwrap(), &changed_locale).unwrap();
        assert_eq!(restored, pending);
        assert_eq!(restored.state.input(), Some(&input()));
        assert_eq!(restored.scope.locale, "en");
        assert!(restored.begin(input(), "ru".into()).is_err());
        assert!(restored.fresh("ru".into()).is_err());
    }

    #[test]
    fn unknown_cannot_settle_or_retarget_a_pending_request() {
        let ready = VariantCreateRecord::new(scope()).unwrap();
        let pending = ready.begin(input(), "en".into()).unwrap();
        assert!(pending.settle(VariantCreateOutcome::Unknown).is_err());
        for field in 0..7 {
            let mut changed = pending.clone();
            let VariantCreatePhase::Pending { key, input } = &mut changed.state else {
                unreachable!()
            };
            match field {
                0 => *key = id(99),
                1 => input.axis_values[0].option_id = id(6),
                2 => input.prices[0].amount = "99".into(),
                3 => input.inventory_quantity = Some(99),
                4 => input.barcode = None,
                5 => input.shipping_profile_slug = None,
                _ => input.inventory_policy = Some("deny".into()),
            }
            assert!(!pending.can_restore(&changed));
            assert!(!pending.can_transition_to(&changed));
        }
    }

    #[test]
    fn terminal_evidence_preserves_request_and_requires_explicit_fresh_intent() {
        let pending = VariantCreateRecord::new(scope())
            .unwrap()
            .begin(input(), "en".into())
            .unwrap();
        assert!(
            pending
                .settle(VariantCreateOutcome::Confirmed {
                    variant_id: "bad".into()
                })
                .is_err()
        );
        assert!(
            pending
                .settle(VariantCreateOutcome::Rejected {
                    code: "forbidden".into()
                })
                .is_err()
        );
        for outcome in [
            VariantCreateOutcome::Confirmed { variant_id: id(8) },
            VariantCreateOutcome::Rejected {
                code: "duplicate_sku".into(),
            },
        ] {
            let settled = pending.settle(outcome).unwrap();
            assert!(pending.can_restore(&settled));
            assert_eq!(settled.state.input(), Some(&input()));
            let fresh = settled.fresh("ru".into()).unwrap();
            assert!(settled.can_transition_to(&fresh));
            assert!(!settled.can_restore(&fresh));
            assert_eq!(settled.document_intent, fresh.document_intent);
            assert_ne!(settled.intent, fresh.intent);
            assert_eq!(fresh.scope.locale, "ru");
            let mut invalid_generation = fresh;
            invalid_generation.intent = uuid::Uuid::nil();
            assert!(!settled.can_transition_to(&invalid_generation));
        }
    }

    #[test]
    fn foreign_corrupt_or_unknown_fields_never_become_a_fresh_request() {
        let pending = VariantCreateRecord::new(scope())
            .unwrap()
            .begin(input(), "en".into())
            .unwrap();
        let raw = pending.encode().unwrap();
        for changed in [
            ProductWriteScope {
                actor_id: id(9),
                ..scope()
            },
            ProductWriteScope {
                tenant_id: id(9),
                ..scope()
            },
            ProductWriteScope {
                product_id: id(9),
                ..scope()
            },
        ] {
            assert_eq!(
                VariantCreateRecord::decode(&raw, &changed),
                Err(WriteJournalError::Changed)
            );
        }
        let mut projection: serde_json::Value = serde_json::from_str(&raw).unwrap();
        projection["state"]["input"]["weight"] = serde_json::json!("2");
        assert!(VariantCreateRecord::decode(&projection.to_string(), &scope()).is_err());
        assert!(VariantCreateRecord::decode("{}", &scope()).is_err());
        assert!(
            VariantCreateRecord::decode(
                &" ".repeat(super::super::write_journal::MAX_JOURNAL_BYTES + 1),
                &scope()
            )
            .is_err()
        );
    }

    #[test]
    fn admission_key_conflicts_and_unrecognized_codes_are_not_terminal_evidence() {
        for code in [
            "FORBIDDEN",
            "UNAUTHORIZED",
            "PRODUCT_OPERATION_FAILED",
            "product.idempotency_conflict",
            "product.revision_required",
            "network",
        ] {
            assert_eq!(normalized_refusal(code), None);
        }
        assert_eq!(normalized_refusal("DUPLICATE_SKU"), Some("duplicate_sku"));
        assert_eq!(
            normalized_refusal("product.duplicate_sku"),
            Some("duplicate_sku")
        );
        assert_eq!(
            normalized_refusal("product.schema_validation"),
            Some("validation")
        );
    }
}
