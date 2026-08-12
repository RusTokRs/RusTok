//! Signed fresh-target handoff into the static-distribution owner ledger.

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ControlPlaneInfrastructure, ModuleStaticDistributionBuildEvidence,
    ModuleStaticDistributionExecutorMode, ModuleStaticDistributionItem,
    ModuleStaticDistributionReleaseAdmission,
    data::{now_expression, placeholder, uuid_from_row, uuid_value},
    distribution::{
        advance_distribution_state, load_distribution_state,
        module_static_distribution_composition_digest,
    },
    distribution_release::{
        ModuleStaticDistributionReleaseError, advance_release_state, insert_admission,
        insert_release_from_parts, load_release_state, validate_admission,
    },
    promotion::{
        normalize_native_entry_type, valid_cargo_package, valid_cas_source_reference, valid_digest,
        valid_reference,
    },
};

pub const MODULE_STATIC_DISTRIBUTION_BOOTSTRAP_RECEIPT_CONTRACT: &str =
    "rustok.modules.static_distribution_bootstrap_receipt";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionBootstrapPreparation {
    pub composition_revision: u64,
    pub composition_digest: String,
    pub platform_source_reference: String,
    pub platform_source_digest: String,
    pub toolchain_digest: String,
    pub build_target: String,
    pub items: Vec<ModuleStaticDistributionItem>,
    pub evidence: ModuleStaticDistributionBuildEvidence,
    pub admission: ModuleStaticDistributionReleaseAdmission,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionBootstrapReceiptPayload {
    pub contract: String,
    pub preparation_id: Uuid,
    pub distribution_release_id: Uuid,
    pub host_composition_revision: String,
    pub host_composition_hash: String,
    pub preparation: ModuleStaticDistributionBootstrapPreparation,
    pub migration_plan_digest: String,
    pub data_contract_digest: String,
    pub signer_key_digest: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionBootstrapReceipt {
    pub payload: ModuleStaticDistributionBootstrapReceiptPayload,
    /// Standard base64 encoding of the Ed25519 signature over canonical compact
    /// JSON bytes of `payload`.
    pub signature: String,
}

#[derive(Debug)]
pub struct VerifiedModuleStaticDistributionBootstrapReceipt {
    receipt: ModuleStaticDistributionBootstrapReceipt,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionBootstrapImportCommand {
    pub receipt: ModuleStaticDistributionBootstrapReceipt,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionBootstrapImportReceipt {
    pub preparation_id: Uuid,
    pub distribution_release_id: Uuid,
    pub release_revision: u64,
    pub receipt_digest: String,
    pub created: bool,
}

#[derive(Clone)]
pub struct SeaOrmModuleStaticDistributionBootstrapService {
    db: DatabaseConnection,
    infrastructure: ControlPlaneInfrastructure,
    public_key_base64: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModuleStaticDistributionBootstrapReceiptError {
    #[error("static-distribution bootstrap receipt is invalid")]
    Invalid,
    #[error("static-distribution bootstrap public key is invalid")]
    InvalidPublicKey,
    #[error("static-distribution bootstrap signature was rejected")]
    SignatureRejected,
    #[error("static-distribution bootstrap receipt is not yet valid")]
    NotYetValid,
    #[error("static-distribution bootstrap receipt has expired")]
    Expired,
    #[error("static-distribution bootstrap receipt serialization failed")]
    Serialization,
}

impl ModuleStaticDistributionBootstrapReceipt {
    pub fn verify(
        &self,
        public_key_base64: &str,
        now: DateTime<Utc>,
    ) -> Result<(), ModuleStaticDistributionBootstrapReceiptError> {
        self.validate_content()?;
        if self.payload.issued_at > now {
            return Err(ModuleStaticDistributionBootstrapReceiptError::NotYetValid);
        }
        if self.payload.expires_at <= now {
            return Err(ModuleStaticDistributionBootstrapReceiptError::Expired);
        }
        let public_key = decode_fixed::<32>(public_key_base64)
            .map_err(|_| ModuleStaticDistributionBootstrapReceiptError::InvalidPublicKey)?;
        if sha256_digest(&public_key) != self.payload.signer_key_digest {
            return Err(ModuleStaticDistributionBootstrapReceiptError::InvalidPublicKey);
        }
        let verifying_key = VerifyingKey::from_bytes(&public_key)
            .map_err(|_| ModuleStaticDistributionBootstrapReceiptError::InvalidPublicKey)?;
        let signature = decode_fixed::<64>(&self.signature)
            .map_err(|_| ModuleStaticDistributionBootstrapReceiptError::Invalid)?;
        let signature = Signature::from_bytes(&signature);
        let payload = rustok_api::manifest_hash::canonical_json_bytes(&self.payload)
            .map_err(|_| ModuleStaticDistributionBootstrapReceiptError::Serialization)?;
        verifying_key
            .verify_strict(&payload, &signature)
            .map_err(|_| ModuleStaticDistributionBootstrapReceiptError::SignatureRejected)
    }

    pub fn verify_owned(
        self,
        public_key_base64: &str,
        now: DateTime<Utc>,
    ) -> Result<
        VerifiedModuleStaticDistributionBootstrapReceipt,
        ModuleStaticDistributionBootstrapReceiptError,
    > {
        self.verify(public_key_base64, now)?;
        Ok(VerifiedModuleStaticDistributionBootstrapReceipt { receipt: self })
    }

    pub fn digest(&self) -> Result<String, ModuleStaticDistributionBootstrapReceiptError> {
        let bytes = rustok_api::manifest_hash::canonical_json_bytes(self)
            .map_err(|_| ModuleStaticDistributionBootstrapReceiptError::Serialization)?;
        Ok(sha256_digest(&bytes))
    }

    pub(crate) fn validate_content(
        &self,
    ) -> Result<(), ModuleStaticDistributionBootstrapReceiptError> {
        let payload = &self.payload;
        let preparation = &payload.preparation;
        if payload.contract != MODULE_STATIC_DISTRIBUTION_BOOTSTRAP_RECEIPT_CONTRACT
            || payload.preparation_id.is_nil()
            || payload.distribution_release_id.is_nil()
            || payload.host_composition_revision.trim().is_empty()
            || payload.host_composition_revision.trim() != payload.host_composition_revision
            || payload.host_composition_revision.len() > 128
            || !valid_hex_digest(&payload.host_composition_hash)
            || preparation.composition_revision != 1
            || !valid_digest(&preparation.composition_digest)
            || !valid_cas_source_reference(
                &preparation.platform_source_reference,
                &preparation.platform_source_digest,
            )
            || !valid_digest(&preparation.toolchain_digest)
            || !valid_reference(&preparation.build_target)
            || preparation.build_target.trim() != preparation.build_target
            || preparation.build_target.len() > 128
            || preparation.items.len() > 256
            || !valid_digest(&payload.migration_plan_digest)
            || !valid_digest(&payload.data_contract_digest)
            || !valid_digest(&payload.signer_key_digest)
            || payload.expires_at <= payload.issued_at
            || self.signature.trim().is_empty()
            || self.signature.len() > 128
        {
            return Err(ModuleStaticDistributionBootstrapReceiptError::Invalid);
        }
        for item in &preparation.items {
            if item.promotion_id.is_nil()
                || item.promotion_revision == 0
                || !valid_reference(&item.release_id)
                || !crate::is_valid_static_module_slug(&item.module_slug)
                || Version::parse(&item.module_version).is_err()
                || !valid_cargo_package(&item.cargo_package)
                || normalize_native_entry_type(&item.entry_type).as_deref()
                    != Some(item.entry_type.as_str())
                || !valid_cas_source_reference(&item.source_reference, &item.source_digest)
                || !valid_digest(&item.dependency_lock_digest)
                || item.executor_mode != ModuleStaticDistributionExecutorMode::StaticNative
            {
                return Err(ModuleStaticDistributionBootstrapReceiptError::Invalid);
            }
        }
        if preparation
            .items
            .windows(2)
            .any(|pair| pair[0].module_slug >= pair[1].module_slug)
        {
            return Err(ModuleStaticDistributionBootstrapReceiptError::Invalid);
        }
        preparation
            .evidence
            .validate()
            .map_err(|_| ModuleStaticDistributionBootstrapReceiptError::Invalid)?;
        validate_admission(
            &preparation.admission,
            &preparation.admission.policy_revision,
        )
        .map_err(|_| ModuleStaticDistributionBootstrapReceiptError::Invalid)?;
        let composition_digest = module_static_distribution_composition_digest(
            &preparation.platform_source_reference,
            &preparation.platform_source_digest,
            &preparation.toolchain_digest,
            &preparation.build_target,
            &preparation.items,
        )
        .map_err(|_| ModuleStaticDistributionBootstrapReceiptError::Invalid)?;
        if composition_digest != preparation.composition_digest {
            return Err(ModuleStaticDistributionBootstrapReceiptError::Invalid);
        }
        Ok(())
    }
}

impl VerifiedModuleStaticDistributionBootstrapReceipt {
    pub fn payload(&self) -> &ModuleStaticDistributionBootstrapReceiptPayload {
        &self.receipt.payload
    }

    pub fn into_receipt(self) -> ModuleStaticDistributionBootstrapReceipt {
        self.receipt
    }
}

impl SeaOrmModuleStaticDistributionBootstrapService {
    pub(crate) fn with_infrastructure(
        db: DatabaseConnection,
        infrastructure: ControlPlaneInfrastructure,
        public_key_base64: String,
    ) -> Self {
        Self {
            db,
            infrastructure,
            public_key_base64,
        }
    }

    /// Imports exactly one platform-signed base preparation into an empty
    /// fresh-target owner ledger. The signature is revalidated here even when
    /// an installer host already verified it.
    pub async fn import(
        &self,
        command: ModuleStaticDistributionBootstrapImportCommand,
    ) -> Result<ModuleStaticDistributionBootstrapImportReceipt, ModuleStaticDistributionReleaseError>
    {
        if command.actor_id.is_nil()
            || command.idempotency_key.is_nil()
            || self.public_key_base64.trim().is_empty()
            || self.public_key_base64.trim() != self.public_key_base64
        {
            return Err(ModuleStaticDistributionReleaseError::InvalidCommand);
        }
        command
            .receipt
            .verify(&self.public_key_base64, self.infrastructure.now())
            .map_err(|_| ModuleStaticDistributionReleaseError::VerificationDenied)?;
        let receipt_digest = command
            .receipt
            .digest()
            .map_err(|_| ModuleStaticDistributionReleaseError::VerificationDenied)?;
        let request_digest = bootstrap_import_request_digest(
            &receipt_digest,
            &self.public_key_base64,
            command.actor_id,
        )?;
        if let Some(replay) = load_bootstrap_import_operation(
            &self.db,
            command.idempotency_key,
            &request_digest,
            command.actor_id,
            &command.receipt.payload,
            &receipt_digest,
        )
        .await?
        {
            return Ok(replay);
        }

        let payload = &command.receipt.payload;
        let transaction = self.db.begin().await.map_err(store_error)?;
        reserve_bootstrap_import_operation(
            &transaction,
            command.idempotency_key,
            &request_digest,
            command.actor_id,
        )
        .await?;
        let distribution_state = load_distribution_state(&transaction, true)
            .await
            .map_err(distribution_error)?;
        let release_state = load_release_state(&transaction, true).await?;
        if distribution_state.revision != 0
            || distribution_state.current_build_id.is_some()
            || release_state.revision != 0
            || release_state.active_release_id.is_some()
        {
            return Err(ModuleStaticDistributionReleaseError::BootstrapLedgerNotFresh);
        }

        insert_bootstrap_preparation(
            &transaction,
            payload,
            &receipt_digest,
            command.actor_id,
            self.infrastructure.now(),
        )
        .await?;
        insert_release_from_parts(
            &transaction,
            payload.distribution_release_id,
            None,
            1,
            command.actor_id,
            self.infrastructure.now(),
            payload.preparation_id,
            payload.preparation.composition_revision,
            &payload.preparation.composition_digest,
            &payload.preparation.evidence,
        )
        .await?;
        insert_admission(
            &transaction,
            self.infrastructure.new_id(),
            payload.distribution_release_id,
            &payload.preparation.admission,
            self.infrastructure.now(),
        )
        .await?;
        advance_distribution_state(
            &transaction,
            0,
            payload.preparation.composition_revision,
            payload.preparation_id,
        )
        .await
        .map_err(distribution_error)?;
        advance_release_state(&transaction, 0, 1, payload.distribution_release_id).await?;
        transaction.commit().await.map_err(store_error)?;
        Ok(ModuleStaticDistributionBootstrapImportReceipt {
            preparation_id: payload.preparation_id,
            distribution_release_id: payload.distribution_release_id,
            release_revision: 1,
            receipt_digest,
            created: true,
        })
    }
}

async fn insert_bootstrap_preparation(
    transaction: &DatabaseTransaction,
    payload: &ModuleStaticDistributionBootstrapReceiptPayload,
    receipt_digest: &str,
    actor_id: Uuid,
    completed_at: DateTime<Utc>,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let preparation = &payload.preparation;
    let role_artifacts_json = serde_json::to_string(&preparation.evidence.roles)
        .map_err(|error| ModuleStaticDistributionReleaseError::Store(error.to_string()))?;
    let backend = transaction.get_database_backend();
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_builds
                 (distribution_build_id, predecessor_build_id, composition_revision,
                  composition_digest, platform_source_reference, platform_source_digest,
                  toolchain_digest, build_target, preparation_source, bootstrap_receipt_digest,
                  status, requested_by, requested_at, attempt_count,
                  bundle_reference, bundle_root_digest, role_set_digest, role_artifacts_json,
                  sbom_reference, sbom_digest, provenance_reference, provenance_digest,
                  signature_reference, signature_digest, test_evidence_reference,
                  test_evidence_digest, completion_digest, completed_at)
                 VALUES ({}, NULL, {}, {}, {}, {}, {}, {}, 'signed_bootstrap', {},
                         'succeeded', {}, {}, 0, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                placeholder(backend, 9),
                placeholder(backend, 10),
                placeholder(backend, 11),
                placeholder(backend, 12),
                placeholder(backend, 13),
                placeholder(backend, 14),
                placeholder(backend, 15),
                placeholder(backend, 16),
                placeholder(backend, 17),
                placeholder(backend, 18),
                placeholder(backend, 19),
                placeholder(backend, 20),
                placeholder(backend, 21),
                placeholder(backend, 22),
                placeholder(backend, 23),
                placeholder(backend, 24),
            ),
            vec![
                uuid_value(payload.preparation_id, backend),
                revision_value(preparation.composition_revision)?,
                preparation.composition_digest.clone().into(),
                preparation.platform_source_reference.clone().into(),
                preparation.platform_source_digest.clone().into(),
                preparation.toolchain_digest.clone().into(),
                preparation.build_target.clone().into(),
                receipt_digest.to_owned().into(),
                uuid_value(actor_id, backend),
                datetime_value(completed_at, backend),
                preparation.evidence.bundle_reference.clone().into(),
                preparation.evidence.bundle_root_digest.clone().into(),
                preparation.evidence.role_set_digest.clone().into(),
                role_artifacts_json.into(),
                preparation.evidence.sbom_reference.clone().into(),
                preparation.evidence.sbom_digest.clone().into(),
                preparation.evidence.provenance_reference.clone().into(),
                preparation.evidence.provenance_digest.clone().into(),
                preparation.evidence.signature_reference.clone().into(),
                preparation.evidence.signature_digest.clone().into(),
                preparation.evidence.test_evidence_reference.clone().into(),
                preparation.evidence.test_evidence_digest.clone().into(),
                receipt_digest.to_owned().into(),
                datetime_value(completed_at, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    for (ordinal, item) in preparation.items.iter().enumerate() {
        insert_bootstrap_item(transaction, payload.preparation_id, ordinal, item).await?;
    }
    Ok(())
}

async fn insert_bootstrap_item(
    transaction: &DatabaseTransaction,
    preparation_id: Uuid,
    ordinal: usize,
    item: &ModuleStaticDistributionItem,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_items
                 (distribution_build_id, ordinal, promotion_id, promotion_revision, release_id,
                  module_slug, module_version, cargo_package, entry_type, source_reference,
                  source_digest, dependency_lock_digest, executor_mode)
                 VALUES ({})",
                (1..=13)
                    .map(|index| placeholder(backend, index))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            vec![
                uuid_value(preparation_id, backend),
                i64::try_from(ordinal)
                    .map_err(|_| ModuleStaticDistributionReleaseError::InvalidCommand)?
                    .into(),
                uuid_value(item.promotion_id, backend),
                revision_value(item.promotion_revision)?,
                item.release_id.clone().into(),
                item.module_slug.clone().into(),
                item.module_version.clone().into(),
                item.cargo_package.clone().into(),
                item.entry_type.clone().into(),
                item.source_reference.clone().into(),
                item.source_digest.clone().into(),
                item.dependency_lock_digest.clone().into(),
                item.executor_mode.as_str().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    Ok(())
}

fn bootstrap_import_request_digest(
    receipt_digest: &str,
    public_key_base64: &str,
    actor_id: Uuid,
) -> Result<String, ModuleStaticDistributionReleaseError> {
    #[derive(Serialize)]
    struct Request<'a> {
        receipt_digest: &'a str,
        public_key_base64: &'a str,
        actor_id: Uuid,
    }
    rustok_api::manifest_hash::hash_manifest(&Request {
        receipt_digest,
        public_key_base64,
        actor_id,
    })
    .map(|digest| format!("sha256:{digest}"))
    .map_err(store_error)
}

async fn load_bootstrap_import_operation<C: ConnectionTrait>(
    connection: &C,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
    payload: &ModuleStaticDistributionBootstrapReceiptPayload,
    receipt_digest: &str,
) -> Result<
    Option<ModuleStaticDistributionBootstrapImportReceipt>,
    ModuleStaticDistributionReleaseError,
> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation_kind, request_digest, actor_id
                 FROM module_static_distribution_release_idempotency_keys
                 WHERE idempotency_key = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(idempotency_key, backend)],
        ))
        .await
        .map_err(store_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let operation_kind: String = row.try_get("", "operation_kind").map_err(store_error)?;
    let stored_digest: String = row.try_get("", "request_digest").map_err(store_error)?;
    let stored_actor = uuid_from_row(&row, "actor_id", backend).map_err(store_error)?;
    if operation_kind != "bootstrap_import"
        || stored_digest != request_digest
        || stored_actor != actor_id
    {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    let release_state = load_release_state(connection, false).await?;
    let distribution_state = load_distribution_state(connection, false)
        .await
        .map_err(distribution_error)?;
    if release_state.revision != 1
        || release_state.active_release_id != Some(payload.distribution_release_id)
        || distribution_state.revision != payload.preparation.composition_revision
        || distribution_state.current_build_id != Some(payload.preparation_id)
    {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    Ok(Some(ModuleStaticDistributionBootstrapImportReceipt {
        preparation_id: payload.preparation_id,
        distribution_release_id: payload.distribution_release_id,
        release_revision: 1,
        receipt_digest: receipt_digest.to_owned(),
        created: false,
    }))
}

async fn reserve_bootstrap_import_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    let inserted = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_release_idempotency_keys
                 (idempotency_key, operation_kind, request_digest, actor_id, created_at)
                 VALUES ({}, 'bootstrap_import', {}, {}, {})
                 ON CONFLICT (idempotency_key) DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
            ),
            vec![
                uuid_value(idempotency_key, backend),
                request_digest.to_owned().into(),
                uuid_value(actor_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if inserted.rows_affected() != 1 {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    Ok(())
}

fn revision_value(value: u64) -> Result<sea_orm::Value, ModuleStaticDistributionReleaseError> {
    i64::try_from(value)
        .map(Into::into)
        .map_err(|_| ModuleStaticDistributionReleaseError::RevisionOverflow)
}

fn datetime_value(value: DateTime<Utc>, backend: DbBackend) -> sea_orm::Value {
    match backend {
        DbBackend::Postgres => sea_orm::Value::ChronoDateTimeUtc(Some(Box::new(value))),
        _ => value.to_rfc3339().into(),
    }
}

fn distribution_error(
    error: crate::ModuleStaticDistributionError,
) -> ModuleStaticDistributionReleaseError {
    ModuleStaticDistributionReleaseError::Store(error.to_string())
}

fn store_error(error: impl std::fmt::Display) -> ModuleStaticDistributionReleaseError {
    ModuleStaticDistributionReleaseError::Store(error.to_string())
}

fn decode_fixed<const N: usize>(value: &str) -> Result<[u8; N], ()> {
    STANDARD
        .decode(value.trim())
        .map_err(|_| ())?
        .try_into()
        .map_err(|_| ())
}

fn valid_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}
