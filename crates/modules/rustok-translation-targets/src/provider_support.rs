//! Reusable owner-side helpers for exact-locale Translation target providers.
//!
//! The helpers deliberately contain no persistence, authorization, or owner
//! error mapping. Domain modules keep those responsibilities while sharing the
//! contract-level CAS, field-hash, and receipt mechanics.

use std::collections::BTreeMap;

use rustok_api::PortError;
use sha2::{Digest, Sha256};

use crate::{
    FieldKey, OpaqueRevision, ReadTranslationResourceRequest, TranslationApplicationReceipt,
    TranslationPatchIssue, TranslationPatchIssueSeverity, TranslationPatchRequest,
    TranslationPatchValidation, TranslationResourceLifecycle, TranslationResourceSnapshot,
};

/// Returns the stable SHA-256 source hash used by translation patch CAS.
pub fn field_hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

/// Builds a read request with the exact source and target locales from a patch.
pub fn read_request_from_patch(
    request: &TranslationPatchRequest,
) -> ReadTranslationResourceRequest {
    ReadTranslationResourceRequest {
        identity: request.identity.clone(),
        source_locale: request.source_locale.clone(),
        target_locale: request.target_locale.clone(),
    }
}

/// Verifies optimistic revisions and source field hashes against a live snapshot.
pub fn validate_patch_against_snapshot(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> TranslationPatchValidation {
    let mut issues = Vec::new();
    if request.expected_resource_revision != snapshot.summary.resource_revision {
        issues.push(conflict_issue(None, "resource_revision_conflict"));
    }
    if request.expected_source_revision != snapshot.source_revision {
        issues.push(conflict_issue(None, "source_revision_conflict"));
    }
    if request.expected_target_revision != snapshot.target_revision {
        issues.push(conflict_issue(None, "target_revision_conflict"));
    }

    let fields = snapshot
        .fields
        .iter()
        .map(|field| (field.descriptor.key.as_str(), field))
        .collect::<BTreeMap<_, _>>();
    for patch in &request.fields {
        match fields.get(patch.key.as_str()) {
            Some(field) if field.source_hash != patch.expected_source_hash => {
                issues.push(conflict_issue(
                    Some(patch.key.clone()),
                    "source_hash_conflict",
                ));
            }
            Some(_) => {}
            None => issues.push(TranslationPatchIssue {
                field: Some(patch.key.clone()),
                severity: TranslationPatchIssueSeverity::Error,
                code: "field_not_supported".to_string(),
                message: "field is not exposed by this translation target".to_string(),
            }),
        }
    }

    TranslationPatchValidation {
        accepted: issues.is_empty(),
        issues,
    }
}

/// Converts structured patch validation evidence into the canonical port error.
pub fn validation_to_port_error(validation: &TranslationPatchValidation) -> PortError {
    let conflict = validation
        .issues
        .iter()
        .any(|issue| issue.code.ends_with("_conflict"));
    let codes = validation
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>()
        .join(",");
    if conflict {
        PortError::conflict(
            "translation.target_patch_conflict",
            format!("translation patch conflicts with live state: {codes}"),
        )
    } else {
        PortError::validation(
            "translation.target_patch_invalid",
            format!("translation patch is invalid: {codes}"),
        )
    }
}

/// Merges a sparse patch over the current exact target values, keyed by field.
pub fn merged_patch_values(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> BTreeMap<String, Option<String>> {
    let mut values = snapshot
        .fields
        .iter()
        .map(|field| {
            (
                field.descriptor.key.as_str().to_string(),
                field.exact_target_value.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for patch in &request.fields {
        if let Some(existing) = values.get_mut(patch.key.as_str()) {
            *existing = Some(patch.value.clone());
        }
    }
    values
}

/// Requires a non-empty target value for a required field.
pub fn required_target_value(value: Option<String>, field: &str) -> Result<String, PortError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            PortError::validation(
                "translation.target_required_field_missing",
                format!("translation patch must contain a non-empty {field}"),
            )
        })
}

/// Treats blank optional target text as absent.
pub fn normalize_optional_target_value(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

/// Parses a persisted positive integer revision from the opaque target contract.
pub fn parse_positive_revision(
    revision: &OpaqueRevision,
    field: &'static str,
) -> Result<i64, PortError> {
    revision
        .as_str()
        .parse::<i64>()
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| {
            PortError::validation(
                "translation.target_revision_invalid",
                format!("{field} must be a positive owner revision"),
            )
        })
}

/// Converts a persisted positive integer revision into the opaque target contract.
pub fn opaque_positive_revision(
    value: i64,
    field: &'static str,
) -> Result<OpaqueRevision, PortError> {
    if value <= 0 {
        return Err(PortError::invariant_violation(
            "translation.target_revision_invalid",
            format!("persisted owner {field} must be positive"),
        ));
    }
    OpaqueRevision::new(value.to_string()).map_err(|error| {
        PortError::invariant_violation("translation.target_revision_invalid", error.to_string())
    })
}

/// Decodes a durable owner-operation receipt for idempotent replay.
pub fn decode_application_receipt(
    value: serde_json::Value,
) -> Result<TranslationApplicationReceipt, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_corrupt", error.to_string())
    })
}

/// Parses a persisted owner lifecycle value into the contract lifecycle.
pub fn parse_resource_lifecycle(value: &str) -> Result<TranslationResourceLifecycle, PortError> {
    match value {
        "active" => Ok(TranslationResourceLifecycle::Active),
        "archived" => Ok(TranslationResourceLifecycle::Archived),
        "deleted" => Ok(TranslationResourceLifecycle::Deleted),
        "unavailable" => Ok(TranslationResourceLifecycle::Unavailable),
        _ => Err(PortError::invariant_violation(
            "translation.target_change_lifecycle_invalid",
            "persisted owner translation lifecycle is invalid",
        )),
    }
}

/// Converts a contract-validation failure into the canonical port error.
pub fn contract_validation_error(message: String) -> PortError {
    PortError::validation("translation.target_contract_invalid", message)
}

fn conflict_issue(field: Option<FieldKey>, code: &str) -> TranslationPatchIssue {
    TranslationPatchIssue {
        field,
        severity: TranslationPatchIssueSeverity::Error,
        code: code.to_string(),
        message: "live translation state no longer matches the proposal".to_string(),
    }
}
