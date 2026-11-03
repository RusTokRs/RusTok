/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Pure existing-SKU reconfiguration drafts and recoverable caller state. Storage and transport
//! live in adapters; only a canonical owner preview authorizes an application.

use crate::core::write_journal::{
    valid_uuid, JournalState, RecoveryQueryKey, WriteJournal, WriteJournalError,
};
use crate::model::{
    ProductDetail, ProductEffectiveForm, SetVariantAxesDraft, VariantAxesPreview,
    VariantAxesPreviewVariant, VariantAxisDraft, VariantAxisReassignmentDraft,
    VariantAxisValueDraft,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) const AXIS_PREVIEW_PAGE_SIZE: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AxisOption {
    pub id: String,
    pub label: String,
    pub available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AxisRow {
    pub attribute_id: String,
    pub code: String,
    pub label: String,
    pub dictionary_limited: bool,
    pub allowed_option_ids: Vec<String>,
    pub options: Vec<AxisOption>,
}

pub(crate) fn seed_axes(
    product: &ProductDetail,
    form: Option<&ProductEffectiveForm>,
) -> Vec<AxisRow> {
    let mut rows = product
        .variant_axes
        .iter()
        .map(|axis| AxisRow {
            attribute_id: axis.attribute_id.clone(),
            code: axis.code.clone(),
            label: axis.name.clone(),
            dictionary_limited: false,
            allowed_option_ids: axis
                .allowed_values
                .iter()
                .map(|value| value.option_id.clone())
                .collect(),
            options: axis
                .allowed_values
                .iter()
                .map(|value| AxisOption {
                    id: value.option_id.clone(),
                    label: if value.value.is_empty() {
                        value.option_id.clone()
                    } else {
                        value.value.clone()
                    },
                    available: false,
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    refresh_choices(&mut rows, form);
    rows
}

/// Refresh localized affordances without reseeding/reordering/pruning an operator proposal.
pub(crate) fn refresh_choices(rows: &mut [AxisRow], form: Option<&ProductEffectiveForm>) {
    for row in rows {
        let attribute = form.and_then(|form| {
            form.attributes
                .iter()
                .find(|attribute| attribute.attribute_id == row.attribute_id)
        });
        row.dictionary_limited = attribute.is_some_and(|attribute| attribute.options.len() > 256);
        let mut options = attribute
            .map(|attribute| {
                attribute
                    .options
                    .iter()
                    .filter(|option| {
                        !row.dictionary_limited || row.allowed_option_ids.contains(&option.id)
                    })
                    .map(|option| AxisOption {
                        id: option.id.clone(),
                        label: option.label.clone(),
                        available: super::is_eligible_axis(attribute),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for id in &row.allowed_option_ids {
            if !options.iter().any(|option| option.id == *id) {
                options.push(AxisOption {
                    id: id.clone(),
                    label: row
                        .options
                        .iter()
                        .find(|option| option.id == *id)
                        .map(|option| option.label.clone())
                        .unwrap_or_else(|| id.clone()),
                    available: false,
                });
            }
        }
        if let Some(attribute) = attribute {
            row.label = attribute.label.clone();
            row.code = attribute.code.clone();
        }
        row.options = options;
    }
}

pub(crate) fn axis_candidate(form: &ProductEffectiveForm, attribute_id: &str) -> Option<AxisRow> {
    let attribute = form
        .attributes
        .iter()
        .find(|attribute| attribute.attribute_id == attribute_id)?;
    if !super::is_eligible_axis(attribute)
        || attribute.options.is_empty()
        || attribute.options.len() > 256
    {
        return None;
    }
    Some(AxisRow {
        attribute_id: attribute.attribute_id.clone(),
        code: attribute.code.clone(),
        label: attribute.label.clone(),
        dictionary_limited: false,
        allowed_option_ids: attribute
            .options
            .iter()
            .map(|option| option.id.clone())
            .collect(),
        options: attribute
            .options
            .iter()
            .map(|option| AxisOption {
                id: option.id.clone(),
                label: option.label.clone(),
                available: true,
            })
            .collect(),
    })
}

pub(crate) fn proposal(
    rows: &[AxisRow],
    reassignments: &[VariantAxisReassignmentDraft],
) -> SetVariantAxesDraft {
    SetVariantAxesDraft {
        axes: rows
            .iter()
            .enumerate()
            .map(|(index, row)| VariantAxisDraft {
                attribute_id: row.attribute_id.clone(),
                position: Some(index as i32 * 100),
                allowed_option_ids: row.allowed_option_ids.clone(),
            })
            .collect(),
        expected_state: None,
        reassignments: reassignments.to_vec(),
    }
}

/// Full tuples are retained on axis-draft changes, including now-unexpected attributes. Only an
/// explicit reset/reassignment may change that operator intent; preview reports the conflicts.
pub(crate) fn reassign_value(
    reassignments: &mut Vec<VariantAxisReassignmentDraft>,
    source: &VariantAxesPreviewVariant,
    attribute_id: &str,
    option_id: &str,
) {
    let mut values = reassignments
        .iter()
        .find(|row| row.variant_id == source.variant_id)
        .map(|row| row.axis_values.clone())
        .unwrap_or_else(|| source.proposed_values.clone());
    values.retain(|value| value.attribute_id != attribute_id);
    if !option_id.is_empty() {
        values.push(VariantAxisValueDraft {
            attribute_id: attribute_id.to_owned(),
            option_id: option_id.to_owned(),
        });
    }
    reassignments.retain(|row| row.variant_id != source.variant_id);
    reassignments.push(VariantAxisReassignmentDraft {
        variant_id: source.variant_id.clone(),
        axis_values: values,
    });
    reassignments.sort_by(|left, right| left.variant_id.cmp(&right.variant_id));
}

pub(crate) fn target_values(
    reassignments: &[VariantAxisReassignmentDraft],
    source: &VariantAxesPreviewVariant,
) -> Vec<VariantAxisValueDraft> {
    reassignments
        .iter()
        .find(|row| row.variant_id == source.variant_id)
        .map(|row| row.axis_values.clone())
        .unwrap_or_else(|| source.proposed_values.clone())
}

pub(crate) fn move_axis(rows: &mut [AxisRow], attribute_id: &str, delta: i32) {
    if let Some(index) = rows.iter().position(|row| row.attribute_id == attribute_id) {
        let target = index as i32 + delta;
        if target >= 0 && (target as usize) < rows.len() {
            rows.swap(index, target as usize);
        }
    }
}

pub(crate) fn toggle_allowed(rows: &mut [AxisRow], attribute_id: &str, option_id: &str) {
    if let Some(row) = rows.iter_mut().find(|row| row.attribute_id == attribute_id) {
        if row.allowed_option_ids.iter().any(|id| id == option_id) {
            row.allowed_option_ids.retain(|id| id != option_id);
        } else if row
            .options
            .iter()
            .any(|option| option.id == option_id && option.available)
        {
            row.allowed_option_ids.push(option_id.to_owned());
        }
    }
}

pub(crate) fn tuple_summary(
    values: &[VariantAxisValueDraft],
    rows: &[AxisRow],
    form: Option<&ProductEffectiveForm>,
) -> String {
    values
        .iter()
        .map(|value| {
            let axis = rows
                .iter()
                .find(|row| row.attribute_id == value.attribute_id);
            let attribute = form.and_then(|form| {
                form.attributes
                    .iter()
                    .find(|attribute| attribute.attribute_id == value.attribute_id)
            });
            let name = axis
                .map(|axis| axis.label.as_str())
                .or_else(|| attribute.map(|attribute| attribute.label.as_str()))
                .unwrap_or(value.attribute_id.as_str());
            let option = axis
                .and_then(|axis| {
                    axis.options
                        .iter()
                        .find(|option| option.id == value.option_id)
                        .map(|option| option.label.as_str())
                })
                .or_else(|| {
                    attribute.and_then(|attribute| {
                        attribute
                            .options
                            .iter()
                            .find(|option| option.id == value.option_id)
                            .map(|option| option.label.as_str())
                    })
                })
                .unwrap_or(value.option_id.as_str());
            format!("{name}: {option}")
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

pub(crate) fn conflict_key(kind: &str) -> &'static str {
    match kind {
        "missing_assignment" => "product-axes-conflict-missingAssignment",
        "duplicate_assignment" => "product-axes-conflict-duplicateAssignment",
        "unexpected_assignment" => "product-axes-conflict-unexpectedAssignment",
        "value_not_allowed" => "product-axes-conflict-valueNotAllowed",
        "duplicate_combination" => "product-axes-conflict-duplicateCombination",
        "multiple_default_variants" => "product-axes-conflict-multipleDefaultVariants",
        _ => "product-axes-conflict-unsupported",
    }
}

/// Only these closed owner codes can settle a rejection. Never parse arbitrary error messages,
/// or treat auth/4xx/idempotency admission conflicts as a decision about a prior uncertain write.
pub(crate) fn normalized_refusal(code: &str) -> Option<&'static str> {
    match code {
        "PRODUCT_VARIANT_AXES_PREVIEW_REQUIRED" | "product.variant_axes_preview_required" => {
            Some("preview_required")
        }
        "PRODUCT_VARIANT_AXES_STALE"
        | "product.variant_axes_stale"
        | "PRODUCT_REVISION_CONFLICT"
        | "product.revision_conflict" => Some("stale"),
        "PRODUCT_VARIANT_AXES_CONFLICT"
        | "product.variant_axes_conflict"
        | "PRODUCT_CONFLICT"
        | "product.conflict" => Some("conflict"),
        "PRODUCT_VARIANT_AXES_LIMIT_EXCEEDED" | "product.variant_axes_limit_exceeded" => {
            Some("limit")
        }
        "PRODUCT_VALIDATION" | "product.validation" | "product.schema_validation" => {
            Some("validation")
        }
        "PRODUCT_NOT_FOUND"
        | "PRODUCT_DEPENDENCY_NOT_FOUND"
        | "product.dependency_not_found"
        | "PRODUCT_VARIANT_NOT_FOUND"
        | "product.product_not_found"
        | "product.variant_not_found" => Some("not_found"),
        "PRODUCT_VARIANT_AXIS_POLICY_CONFLICT" | "product.variant_axis_policy_conflict" => {
            Some("policy_conflict")
        }
        "NO_VARIANTS" | "product.no_variants" => Some("no_variants"),
        _ => None,
    }
}

pub(crate) fn refusal_key(code: &str) -> &'static str {
    match code {
        "preview_required" | "stale" => "product-axes-stale",
        "conflict" | "no_variants" => "product-axes-conflicts",
        "limit" => "product-axes-bounds",
        _ => "product-axes-failed",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AxisReview {
    pub proposal: SetVariantAxesDraft,
    pub preview: VariantAxesPreview,
}

impl AxisReview {
    pub fn is_current(&self, proposal: &SetVariantAxesDraft) -> bool {
        self.proposal == *proposal
    }
    pub fn application(&self, proposal: &SetVariantAxesDraft) -> Option<SetVariantAxesDraft> {
        if !self.is_current(proposal) || !self.preview.can_apply || !self.preview.has_changes {
            return None;
        }
        let mut input = self.proposal.clone();
        input.expected_state = Some(self.preview.state_token.clone());
        Some(input)
    }
}

pub(crate) fn valid_preview(product_id: &str, preview: &VariantAxesPreview) -> bool {
    preview.product_id == product_id
        && preview.product_revision >= 0
        && preview.variant_count > 0
        && preview.variant_count <= 256
        && preview.variants.len() == preview.variant_count as usize
        && preview
            .variants
            .iter()
            .map(|row| row.variant_id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len()
            == preview.variants.len()
        && preview.variants.iter().all(|row| {
            valid_uuid(&row.variant_id)
                && row.current_values.len() <= 32768
                && row.proposed_values.len() <= 32768
                && row
                    .current_values
                    .iter()
                    .chain(&row.proposed_values)
                    .all(|value| valid_uuid(&value.attribute_id) && valid_uuid(&value.option_id))
        })
        && valid_token(&preview.state_token)
        && preview.conflicts.len() <= 16
        && preview.conflicts.iter().all(|conflict| {
            conflict.variant_ids.len() <= 8
                && conflict.variant_ids.iter().all(|id| valid_uuid(id))
                && conflict.attribute_id.as_deref().is_none_or(valid_uuid)
                && conflict.option_id.as_deref().is_none_or(valid_uuid)
                && conflict_key(&conflict.kind) != "product-axes-conflict-unsupported"
        })
        && (!preview.can_apply || (preview.conflicts.is_empty() && !preview.has_more_conflicts))
}

fn valid_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
fn valid_input(input: &SetVariantAxesDraft) -> bool {
    input.expected_state.as_deref().is_some_and(valid_token)
        && input.axes.len() <= 32
        && input.reassignments.len() <= 256
        && input.axes.iter().all(|axis| {
            valid_uuid(&axis.attribute_id)
                && axis.position.is_none_or(|position| position >= 0)
                && !axis.allowed_option_ids.is_empty()
                && axis.allowed_option_ids.len() <= 256
                && axis.allowed_option_ids.iter().all(|id| valid_uuid(id))
        })
        && input.reassignments.iter().all(|row| {
            valid_uuid(&row.variant_id)
                && row.axis_values.len() <= 32
                && row
                    .axis_values
                    .iter()
                    .all(|value| valid_uuid(&value.attribute_id) && valid_uuid(&value.option_id))
        })
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum AxisJournalPhase {
    Ready,
    Pending {
        key: String,
        input: SetVariantAxesDraft,
    },
    Confirmed {
        key: String,
    },
    Rejected {
        key: String,
        code: String,
    },
}

pub(crate) type AxisJournalRecord = WriteJournal<AxisJournalPhase>;

impl JournalState for AxisJournalPhase {
    type Scope = super::write_journal::ProductWriteScope;
    const STORAGE_PREFIX: &'static str = "rustok.product.axis_attempt";
    const DOCUMENT_QUERY: RecoveryQueryKey = RecoveryQueryKey::Axes;

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
            Self::Confirmed { key } => valid_uuid(key),
            Self::Rejected { key, code } => {
                valid_uuid(key)
                    && matches!(
                        code.as_str(),
                        "validation"
                            | "policy_conflict"
                            | "conflict"
                            | "not_found"
                            | "preview_required"
                            | "stale"
                            | "limit"
                            | "no_variants"
                    )
            }
        }
    }
    fn can_follow(&self, next: &Self) -> bool {
        match (self, next) {
            (Self::Ready, Self::Pending { .. }) => true,
            (Self::Pending { key: old, .. }, Self::Confirmed { key })
            | (Self::Pending { key: old, .. }, Self::Rejected { key, .. }) => old == key,
            _ => false,
        }
    }
}

impl WriteJournal<AxisJournalPhase> {
    pub fn begin(&self, input: SetVariantAxesDraft) -> Result<Self, WriteJournalError> {
        if !matches!(self.state, AxisJournalPhase::Ready) || !valid_input(&input) {
            return Err(WriteJournalError::Changed);
        }
        let mut record = self.clone();
        record.state = AxisJournalPhase::Pending {
            key: Uuid::new_v4().to_string(),
            input,
        };
        Ok(record)
    }
    pub fn settle(&self, refusal: Option<&str>) -> Result<Self, WriteJournalError> {
        let AxisJournalPhase::Pending { key, .. } = &self.state else {
            return Err(WriteJournalError::Changed);
        };
        let mut record = self.clone();
        record.state = match refusal {
            None => AxisJournalPhase::Confirmed { key: key.clone() },
            Some(code) => AxisJournalPhase::Rejected {
                key: key.clone(),
                code: code.to_owned(),
            },
        };
        record.encode()?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::write_journal::ProductWriteScope;
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
    fn input() -> SetVariantAxesDraft {
        SetVariantAxesDraft {
            axes: vec![VariantAxisDraft {
                attribute_id: id(4),
                position: Some(0),
                allowed_option_ids: vec![id(5), id(6)],
            }],
            expected_state: Some("a".repeat(64)),
            reassignments: vec![VariantAxisReassignmentDraft {
                variant_id: id(7),
                axis_values: vec![VariantAxisValueDraft {
                    attribute_id: id(4),
                    option_id: id(5),
                }],
            }],
        }
    }

    #[test]
    fn existing_serialized_axes_envelope_keeps_its_shape_and_frozen_request() {
        let raw = serde_json::json!({
            "scope": { "tenant_id": id(1), "actor_id": id(2), "product_id": id(3), "locale": "en" },
            "document_intent": id(11), "intent": id(12),
            "state": { "phase": "pending", "key": id(13), "input": {
                "axes": [{ "attributeId": id(4), "position": 0, "allowedOptionIds": [id(5), id(6)] }],
                "expectedState": "a".repeat(64),
                "reassignments": [{ "variantId": id(7), "axisValues": [{ "attributeId": id(4), "optionId": id(5) }] }]
            }}
        });
        let decoded = AxisJournalRecord::decode(&raw.to_string(), &scope()).unwrap();
        assert_eq!(serde_json::to_value(&decoded).unwrap(), raw);
        assert_eq!(
            decoded.scope.storage_key::<AxisJournalPhase>(),
            format!("rustok.product.axis_attempt:{}:{}:{}", id(1), id(2), id(3))
        );
        assert!(
            matches!(decoded.state, AxisJournalPhase::Pending { input: saved, .. } if saved == input())
        );
    }

    #[test]
    fn metadata_refresh_preserves_the_dirty_proposal_and_unavailable_selected_ids() {
        let mut rows = vec![AxisRow {
            attribute_id: id(4),
            code: "color".into(),
            label: "Saved color".into(),
            dictionary_limited: false,
            allowed_option_ids: vec![id(6), id(5)],
            options: vec![AxisOption {
                id: id(5),
                label: "Saved red".into(),
                available: true,
            }],
        }];
        let before = proposal(&rows, &[]);
        refresh_choices(&mut rows, None);
        assert_eq!(proposal(&rows, &[]), before);
        assert_eq!(rows[0].options.len(), 2);
        assert!(rows[0].options.iter().all(|option| !option.available));
        assert_eq!(
            normalized_refusal("product.schema_validation"),
            Some("validation")
        );
        assert_eq!(
            normalized_refusal("PRODUCT_DEPENDENCY_NOT_FOUND"),
            Some("not_found")
        );
    }

    #[test]
    fn pending_restore_keeps_exact_key_input_and_original_locale() {
        let ready = AxisJournalRecord::new(scope()).unwrap();
        let pending = ready.begin(input()).unwrap();
        let mut changed_locale = scope();
        changed_locale.locale = "ru".into();
        let restored =
            AxisJournalRecord::decode(&pending.encode().unwrap(), &changed_locale).unwrap();
        assert_eq!(restored, pending);
        assert_eq!(restored.scope.locale, "en");
        assert!(restored.begin(input()).is_err());
        assert!(restored.fresh("ru".into()).is_err());
        let mut changed_actor = scope();
        changed_actor.actor_id = id(9);
        assert_eq!(
            AxisJournalRecord::decode(&pending.encode().unwrap(), &changed_actor),
            Err(WriteJournalError::Changed)
        );
    }

    #[test]
    fn only_owner_terminal_outcome_allows_next_generation_without_changing_document_marker() {
        let ready = AxisJournalRecord::new(scope()).unwrap();
        let pending = ready.begin(input()).unwrap();
        let terminal = pending.settle(None).unwrap();
        let fresh = terminal.fresh("ru".into()).unwrap();
        assert!(ready.can_transition_to(&pending));
        assert!(pending.can_transition_to(&terminal));
        assert!(terminal.can_transition_to(&fresh));
        assert_eq!(terminal.document_intent, fresh.document_intent);
        assert_ne!(terminal.intent, fresh.intent);
        assert!(!pending.can_transition_to(&fresh));
        assert!(!terminal.can_transition_to(&pending));
        assert!(
            !terminal.can_restore(&fresh),
            "a new generation must not unlock a confirmed old Product document"
        );
        assert_eq!(
            terminal.fresh_from_live(&fresh, "en".into()).unwrap(),
            fresh
        );
        assert!(pending.settle(Some("arbitrary_backend_text")).is_err());
    }

    #[test]
    fn restore_may_finish_a_failed_storage_readback_but_not_replace_an_uncertain_request() {
        let ready = AxisJournalRecord::new(scope()).unwrap();
        let pending = ready.begin(input()).unwrap();
        let terminal = pending.settle(Some("stale")).unwrap();
        assert!(ready.can_restore(&pending));
        assert!(pending.can_restore(&terminal));
        let other_pending = ready.begin(input()).unwrap();
        assert!(!pending.can_restore(&other_pending));
        assert!(!terminal.can_restore(&pending));
        let mut other_document = pending.clone();
        other_document.document_intent = Uuid::new_v4();
        assert!(!ready.can_restore(&other_document));
    }

    #[test]
    fn journal_rejects_unknown_fields_unbounded_records_and_malformed_payloads() {
        let ready = AxisJournalRecord::new(scope()).unwrap();
        let pending = ready.begin(input()).unwrap();
        let mut raw = serde_json::to_value(&pending).unwrap();
        raw["secret"] = serde_json::json!("not a journal field");
        assert_eq!(
            AxisJournalRecord::decode(&raw.to_string(), &scope()),
            Err(WriteJournalError::Invalid)
        );
        assert_eq!(
            AxisJournalRecord::decode(
                &"x".repeat(crate::core::write_journal::MAX_JOURNAL_BYTES + 1),
                &scope()
            ),
            Err(WriteJournalError::Invalid)
        );
        let mut bad = input();
        bad.expected_state = None;
        assert!(ready.begin(bad).is_err());
        let mut bad = input();
        bad.axes[0].allowed_option_ids.clear();
        assert!(ready.begin(bad).is_err());
        let mut bad = input();
        bad.axes[0].attribute_id = Uuid::nil().to_string();
        assert!(ready.begin(bad).is_err());
    }

    #[test]
    fn full_tuple_edits_retain_unexpected_assignments_until_an_explicit_reset() {
        let source = VariantAxesPreviewVariant {
            variant_id: id(7),
            sku: None,
            current_values: vec![],
            proposed_values: vec![
                VariantAxisValueDraft {
                    attribute_id: id(4),
                    option_id: id(5),
                },
                VariantAxisValueDraft {
                    attribute_id: id(8),
                    option_id: id(9),
                },
            ],
            changes: true,
        };
        let mut overrides = Vec::new();
        reassign_value(&mut overrides, &source, &id(4), &id(6));
        assert_eq!(overrides[0].axis_values.len(), 2);
        assert!(overrides[0]
            .axis_values
            .iter()
            .any(|value| value.attribute_id == id(8)));
        reassign_value(&mut overrides, &source, &id(4), "");
        assert_eq!(overrides[0].axis_values.len(), 1);
        assert_eq!(normalized_refusal("UNAUTHORIZED"), None);
        assert_eq!(
            normalized_refusal("PRODUCT_VARIANT_AXES_STALE"),
            Some("stale")
        );
    }
}
