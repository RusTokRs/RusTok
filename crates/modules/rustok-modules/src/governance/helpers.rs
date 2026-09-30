//! Low-level database formatting, hashing, and JSON column helpers.

use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, QueryResult, Statement, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::*;
use crate::installation::ArtifactVerificationEvidence;

pub(crate) fn required_json_text(
    row: &QueryResult,
    column: &str,
) -> Result<serde_json::Value, ModuleGovernanceError> {
    optional_json_text(row, column)?.ok_or_else(|| {
        ModuleGovernanceError::Store(format!("registry column '{column}' must not be null"))
    })
}

pub(crate) fn optional_json_text(
    row: &QueryResult,
    column: &str,
) -> Result<Option<serde_json::Value>, ModuleGovernanceError> {
    row.try_get::<Option<String>>("", column)
        .map_err(store_error)?
        .map(|value| serde_json::from_str(&value).map_err(store_error))
        .transpose()
}

pub(crate) async fn governance_owner_principal_for_slug(
    transaction: &DatabaseTransaction,
    backend: DbBackend,
    slug: &str,
    lock: bool,
) -> Result<Option<serde_json::Value>, ModuleGovernanceError> {
    let row_lock = if lock && backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT CAST(owner_principal AS TEXT) AS owner_principal \
                 FROM registry_module_owners WHERE slug = {}{row_lock}",
                placeholder(backend, 1),
            ),
            vec![slug.to_string().into()],
        ))
        .await
        .map_err(store_error)?;
    row.as_ref()
        .map(|row| required_json_text(row, "owner_principal"))
        .transpose()
}

pub(crate) fn json_string_list(
    value: Option<serde_json::Value>,
) -> Result<Vec<String>, ModuleGovernanceError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value.as_array().ok_or_else(|| {
        ModuleGovernanceError::Store("registry message list must be an array".to_string())
    })?;
    Ok(values
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(ToString::to_string)
        .collect())
}

pub(crate) fn required_column<T>(row: &QueryResult, column: &str) -> Result<T, ModuleGovernanceError>
where
    T: sea_orm::TryGetable,
{
    row.try_get("", column).map_err(store_error)
}

pub(crate) async fn current_publish_request_revision(
    transaction: &DatabaseTransaction,
    backend: DbBackend,
    request_id: &str,
) -> Result<i64, ModuleGovernanceError> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT revision FROM registry_publish_requests WHERE id = {}",
                placeholder(backend, 1),
            ),
            vec![request_id.to_string().into()],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleGovernanceError::PublishRequestNotFound)
        .and_then(|row| required_column(&row, "revision"))
}

pub(crate) async fn publish_request_revision_conflict(
    transaction: &DatabaseTransaction,
    backend: DbBackend,
    request_id: &str,
    expected_revision: i64,
) -> Result<ModuleGovernanceError, ModuleGovernanceError> {
    Ok(ModuleGovernanceError::PublishRequestRevisionConflict {
        expected: expected_revision,
        current: current_publish_request_revision(transaction, backend, request_id).await?,
    })
}

pub(crate) fn optional_column<T>(row: &QueryResult, column: &str) -> Result<Option<T>, ModuleGovernanceError>
where
    T: sea_orm::TryGetable,
{
    row.try_get("", column).map_err(store_error)
}

pub(crate) fn required_timestamp(row: &QueryResult, column: &str) -> Result<String, ModuleGovernanceError> {
    row.try_get::<chrono::DateTime<chrono::Utc>>("", column)
        .map(|value| value.to_rfc3339())
        .map_err(store_error)
}

pub(crate) fn optional_timestamp(
    row: &QueryResult,
    column: &str,
) -> Result<Option<String>, ModuleGovernanceError> {
    row.try_get::<Option<chrono::DateTime<chrono::Utc>>>("", column)
        .map(|value| value.map(|value| value.to_rfc3339()))
        .map_err(store_error)
}

pub(crate) fn placeholder(backend: sea_orm::DbBackend, position: usize) -> String {
    if backend == sea_orm::DbBackend::Postgres {
        format!("${position}")
    } else {
        format!("?{position}")
    }
}

pub(crate) fn database_now(backend: sea_orm::DbBackend) -> &'static str {
    if backend == sea_orm::DbBackend::Postgres {
        "NOW()"
    } else {
        "datetime('now')"
    }
}

pub(crate) fn registry_uuid_value(value: Uuid, backend: sea_orm::DbBackend) -> Value {
    if backend == sea_orm::DbBackend::Postgres {
        Value::Uuid(Some(value))
    } else {
        value.to_string().into()
    }
}

pub(crate) fn store_error(error: impl std::fmt::Display) -> ModuleGovernanceError {
    ModuleGovernanceError::Store(error.to_string())
}

pub(crate) fn dedupe_validation_messages(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for value in values {
        let value = value.trim().to_string();
        if !value.is_empty() && seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

pub(crate) fn valid_marketplace_taxonomy(value: &serde_json::Value) -> bool {
    let Some(marketplace) = value.as_object() else {
        return false;
    };
    if marketplace
        .keys()
        .any(|key| !matches!(key.as_str(), "category" | "tags"))
    {
        return false;
    }
    let valid_token = |token: &str| {
        !token.is_empty()
            && token.chars().count() <= MAX_MARKETPLACE_TAXONOMY_CHARS
            && token.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '-' | '_')
            })
    };
    if marketplace
        .get("category")
        .is_some_and(|category| !category.is_null() && !category.as_str().is_some_and(valid_token))
    {
        return false;
    }
    let Some(tags) = marketplace
        .get("tags")
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };
    if tags.len() > MAX_MARKETPLACE_TAXONOMY_ITEMS {
        return false;
    }
    let mut unique = std::collections::HashSet::new();
    tags.iter().all(|tag| {
        tag.as_str()
            .is_some_and(|tag| valid_token(tag) && unique.insert(tag))
    })
}

pub(crate) fn publish_request_create_identity(
    command: &ModulePublishRequestCreateCommand,
) -> Result<(String, String), ModuleGovernanceError> {
    let mut identity = serde_json::to_value(command)
        .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    identity
        .as_object_mut()
        .ok_or_else(|| {
            ModuleGovernanceError::Store(
                "publish request command must serialize as an object".to_string(),
            )
        })?
        .remove("context");
    let command_digest = rustok_api::manifest_hash::hash_manifest(&identity)
        .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    Ok((format!("rpr_{command_digest}"), command_digest))
}

pub(crate) fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn registry_publish_artifact_storage_key(
    checksum_sha256: &str,
) -> Result<String, ModuleGovernanceError> {
    if !is_sha256_hex(checksum_sha256) {
        return Err(ModuleGovernanceError::InvalidPublishArtifactAttachCommand);
    }
    DigestObjectKey::sha256(
        REGISTRY_PUBLISH_ARTIFACT_NAMESPACE,
        ObjectScope::Platform,
        checksum_sha256,
    )
    .map(|key| key.to_string())
    .map_err(|error| ModuleGovernanceError::Store(error.to_string()))
}


pub(crate) fn valid_publication_translations(
    default_locale: &str,
    translations: &[(String, String, String)],
) -> bool {
    let Some(normalized_default_locale) = rustok_api::normalize_locale_tag(default_locale) else {
        return false;
    };
    if normalized_default_locale != default_locale {
        return false;
    }

    let mut has_default_translation = false;
    for (locale, name, description) in translations {
        let Ok(content) = ModuleMarketplaceContentProjection::try_new(name, description) else {
            return false;
        };
        if rustok_api::normalize_locale_tag(locale).as_deref() != Some(locale.as_str())
            || content.name.as_str() != name.as_str()
            || content.description.as_str() != description.as_str()
            || content.description.chars().count() < 20
        {
            return false;
        }
        has_default_translation |= locale == default_locale;
    }
    has_default_translation
}

pub(crate) fn receipt_subject_digest_sha256(
    receipt: &ModuleBuildPublicationReceipt,
) -> Result<&str, ModuleGovernanceError> {
    receipt_digest_sha256(&receipt.artifact.digest)
}


pub(crate) fn platform_build_artifact_identities_valid(
    component_digest: &str,
    receipt: &ModuleBuildPublicationReceipt,
) -> bool {
    receipt_digest_sha256(component_digest).is_ok()
        && receipt_digest_sha256(&receipt.artifact.digest).is_ok()
}

pub(crate) fn receipt_digest_sha256(digest: &str) -> Result<&str, ModuleGovernanceError> {
    digest
        .strip_prefix("sha256:")
        .filter(|digest| is_sha256_hex(digest))
        .ok_or(ModuleGovernanceError::InvalidBuildServiceAttestationCommand)
}

pub(crate) fn prefixed_sha256_digest(digest: &str) -> bool {
    digest.strip_prefix("sha256:").is_some_and(is_sha256_hex)
}

pub(crate) fn platform_admission_policy_revision(evidence: &ArtifactVerificationEvidence) -> String {
    format!(
        "trust:{};capability:{}",
        evidence.trust_policy_revision, evidence.capability_policy_revision
    )
}

pub(crate) fn platform_admission_evidence_reference(
    reference: &OciArtifactReference,
    evidence: &ArtifactVerificationEvidence,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok.module.platform-admission.v1\0");
    let mut evidence_references = evidence.evidence.clone();
    evidence_references.sort_by_key(|evidence| evidence.kind);
    for value in [
        reference.canonical(),
        evidence.payload_digest.clone(),
        evidence.media_type.clone(),
        evidence.signer_identity.clone(),
        platform_admission_policy_revision(evidence),
        evidence.signature_verified.to_string(),
        evidence.provenance_verified.to_string(),
        evidence.sbom_verified.to_string(),
        evidence.license_policy_verified.to_string(),
        evidence.vulnerability_policy_verified.to_string(),
        evidence.verified_at.to_rfc3339(),
    ] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    for evidence in evidence_references {
        for value in [
            evidence.kind.as_str(),
            evidence.reference.as_str(),
            evidence.digest.as_str(),
        ] {
            hasher.update((value.len() as u64).to_be_bytes());
            hasher.update(value.as_bytes());
        }
    }
    format!(
        "platform-admission://{}/evidence/{}",
        reference.canonical(),
        hex::encode(hasher.finalize())
    )
}

pub(crate) fn validate_publication_evidence_fields(
    request_id: &str,
    subject_digest_sha256: &str,
    evidence_reference: &str,
    issuer_identity: &str,
    policy_revision: &str,
    actor_principal: &serde_json::Value,
) -> Result<(), ModuleGovernanceError> {
    if request_id.trim().is_empty()
        || !is_sha256_hex(subject_digest_sha256)
        || evidence_reference.trim().is_empty()
        || evidence_reference.len() > MAX_PUBLICATION_EVIDENCE_REFERENCE_BYTES
        || issuer_identity.trim().is_empty()
        || issuer_identity.len() > MAX_PUBLICATION_EVIDENCE_IDENTITY_BYTES
        || policy_revision.trim().is_empty()
        || policy_revision.len() > MAX_PUBLICATION_EVIDENCE_POLICY_REVISION_BYTES
        || !actor_principal.is_object()
    {
        return Err(ModuleGovernanceError::InvalidPublicationEvidenceCommand);
    }
    Ok(())
}

pub(crate) fn publication_evidence_digest_sha256(command: &ModulePublicationEvidenceCommand) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok.module.publication-evidence\0");
    for value in [
        command.authority.as_str(),
        command.subject_digest_sha256.as_str(),
        command.evidence_reference.as_str(),
        command.issuer_identity.as_str(),
        command.policy_revision.as_str(),
    ] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    match command.signature_digest_sha256.as_deref() {
        Some(digest) => {
            hasher.update([1]);
            hasher.update((digest.len() as u64).to_be_bytes());
            hasher.update(digest.as_bytes());
        }
        None => hasher.update([0]),
    }
    hex::encode(hasher.finalize())
}

