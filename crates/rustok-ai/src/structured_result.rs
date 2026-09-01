use std::{collections::BTreeMap, sync::Arc, time::Duration};

use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, Payload},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use rustok_api::{PortError, manifest_hash::hash_manifest};
use rustok_secrets::{ExposeSecret, SecretRef, SecretResolverRegistry};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::ai_structured_results;

const NONCE_BYTES: usize = 12;
const KEY_BYTES: usize = 32;
const MIN_RETENTION_SECONDS: u64 = 60;
const MAX_RETENTION_SECONDS: u64 = 7 * 24 * 60 * 60;
const RESULT_CLEANUP_BATCH_SIZE: u64 = 500;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AiStructuredResultKeyringConfig {
    pub active_key_id: String,
    pub retention_seconds: u64,
    pub keys: BTreeMap<String, SecretRef>,
}

impl AiStructuredResultKeyringConfig {
    pub fn validate(&self) -> Result<(), PortError> {
        validate_key_id(&self.active_key_id)?;
        if !(MIN_RETENTION_SECONDS..=MAX_RETENTION_SECONDS).contains(&self.retention_seconds) {
            return Err(PortError::validation(
                "ai.structured.result_retention_invalid",
                "structured result retention must be between 60 seconds and 7 days",
            ));
        }
        if self.keys.is_empty() || self.keys.len() > 16 {
            return Err(PortError::validation(
                "ai.structured.result_keyring_invalid",
                "structured result keyring must contain 1..=16 keys",
            ));
        }
        for (key_id, reference) in &self.keys {
            validate_key_id(key_id)?;
            if reference.resolver.trim().is_empty() || reference.key.trim().is_empty() {
                return Err(PortError::validation(
                    "ai.structured.result_key_reference_invalid",
                    "structured result key references must be complete",
                ));
            }
        }
        if !self.keys.contains_key(&self.active_key_id) {
            return Err(PortError::validation(
                "ai.structured.result_active_key_missing",
                "structured result active key is not present in the keyring",
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct StructuredResultKeyring {
    active_key_id: String,
    retention: Duration,
    keys: Arc<BTreeMap<String, SecretRef>>,
    secrets: SecretResolverRegistry,
    #[cfg(test)]
    test_keys: Arc<BTreeMap<String, [u8; KEY_BYTES]>>,
}

#[derive(Debug, Clone)]
pub(crate) struct SealedStructuredResult {
    pub tenant_id: Uuid,
    pub execution_id: Uuid,
    pub request_digest: String,
    pub output_digest: String,
    pub key_id: String,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub plaintext_bytes: i64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone)]
pub(crate) struct StructuredResultStore {
    database: DatabaseConnection,
    keyring: StructuredResultKeyring,
}

impl StructuredResultKeyring {
    pub(crate) fn new(
        config: AiStructuredResultKeyringConfig,
        secrets: SecretResolverRegistry,
    ) -> Result<Self, PortError> {
        config.validate()?;
        Ok(Self {
            active_key_id: config.active_key_id,
            retention: Duration::from_secs(config.retention_seconds),
            keys: Arc::new(config.keys),
            secrets,
            #[cfg(test)]
            test_keys: Arc::new(BTreeMap::new()),
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        active_key_id: &str,
        retention: Duration,
        keys: BTreeMap<String, [u8; KEY_BYTES]>,
    ) -> Self {
        Self {
            active_key_id: active_key_id.to_string(),
            retention,
            keys: Arc::new(BTreeMap::new()),
            secrets: SecretResolverRegistry::builder().build(),
            test_keys: Arc::new(keys),
        }
    }

    pub(crate) async fn seal(
        &self,
        tenant_id: Uuid,
        execution_id: Uuid,
        request_digest: &str,
        output: &Value,
    ) -> Result<SealedStructuredResult, PortError> {
        let plaintext = serde_json::to_vec(output).map_err(|_| result_invariant())?;
        if plaintext.is_empty() {
            return Err(result_invariant());
        }
        let plaintext_bytes = i64::try_from(plaintext.len()).map_err(|_| result_invariant())?;
        let output_digest = hash_manifest(output).map_err(|_| result_invariant())?;
        let key = self.resolve_key(tenant_id, &self.active_key_id).await?;
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| result_key_invalid())?;
        let mut nonce = [0_u8; NONCE_BYTES];
        let random_bytes = Uuid::new_v4();
        nonce.copy_from_slice(&random_bytes.as_bytes()[..NONCE_BYTES]);
        let aad = result_aad(
            tenant_id,
            execution_id,
            request_digest,
            &output_digest,
            &self.active_key_id,
        );
        let ciphertext = cipher
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: &plaintext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| result_invariant())?;
        let created_at = Utc::now();
        let retention =
            chrono::Duration::from_std(self.retention).map_err(|_| result_invariant())?;
        let expires_at = created_at
            .checked_add_signed(retention)
            .ok_or_else(result_invariant)?;
        Ok(SealedStructuredResult {
            tenant_id,
            execution_id,
            request_digest: request_digest.to_string(),
            output_digest,
            key_id: self.active_key_id.clone(),
            nonce: nonce.to_vec(),
            ciphertext,
            plaintext_bytes,
            created_at,
            expires_at,
        })
    }

    pub(crate) async fn prepare(&self, tenant_id: Uuid) -> Result<(), PortError> {
        self.resolve_key(tenant_id, &self.active_key_id).await?;
        Ok(())
    }

    async fn open(
        &self,
        tenant_id: Uuid,
        execution_id: Uuid,
        expected_request_digest: &str,
        max_output_bytes: i64,
        result: &ai_structured_results::Model,
        now: DateTime<Utc>,
    ) -> Result<Value, PortError> {
        if result.tenant_id != tenant_id
            || result.execution_id != execution_id
            || result.request_digest != expected_request_digest
        {
            return Err(result_invariant());
        }
        if result.expires_at.with_timezone(&Utc) <= now {
            return Err(PortError::conflict(
                "ai.structured.result_expired",
                "structured execution result is no longer available for replay",
            ));
        }
        if max_output_bytes <= 0
            || result.nonce.len() != NONCE_BYTES
            || result.plaintext_bytes <= 0
            || result.plaintext_bytes > max_output_bytes
            || result.ciphertext.len()
                != usize::try_from(result.plaintext_bytes)
                    .map_err(|_| result_invariant())?
                    .saturating_add(16)
        {
            return Err(result_invariant());
        }
        let key = self.resolve_key(tenant_id, &result.key_id).await?;
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| result_key_invalid())?;
        let aad = result_aad(
            tenant_id,
            execution_id,
            expected_request_digest,
            &result.output_digest,
            &result.key_id,
        );
        let nonce: &[u8; NONCE_BYTES] = result
            .nonce
            .as_slice()
            .try_into()
            .map_err(|_| result_invariant())?;
        let plaintext = cipher
            .decrypt(
                nonce.into(),
                Payload {
                    msg: &result.ciphertext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| result_invariant())?;
        if i64::try_from(plaintext.len()).map_err(|_| result_invariant())? != result.plaintext_bytes
        {
            return Err(result_invariant());
        }
        let output: Value = serde_json::from_slice(&plaintext).map_err(|_| result_invariant())?;
        let output_digest = hash_manifest(&output).map_err(|_| result_invariant())?;
        if output_digest != result.output_digest {
            return Err(result_invariant());
        }
        Ok(output)
    }

    async fn resolve_key(
        &self,
        tenant_id: Uuid,
        key_id: &str,
    ) -> Result<[u8; KEY_BYTES], PortError> {
        #[cfg(test)]
        if let Some(key) = self.test_keys.get(key_id) {
            return Ok(*key);
        }
        let reference = self.keys.get(key_id).ok_or_else(|| {
            PortError::unavailable(
                "ai.structured.result_key_unavailable",
                "structured result encryption key is unavailable",
            )
        })?;
        let secret = self
            .secrets
            .resolve_for_tenant(tenant_id, reference)
            .await
            .map_err(|_| result_key_unavailable())?;
        let decoded = STANDARD
            .decode(secret.expose_secret())
            .map_err(|_| result_key_invalid())?;
        decoded.try_into().map_err(|_| result_key_invalid())
    }
}

impl StructuredResultStore {
    pub(crate) fn new(database: DatabaseConnection, keyring: StructuredResultKeyring) -> Self {
        Self { database, keyring }
    }

    pub(crate) async fn seal(
        &self,
        tenant_id: Uuid,
        execution_id: Uuid,
        request_digest: &str,
        output: &Value,
    ) -> Result<SealedStructuredResult, PortError> {
        self.keyring
            .seal(tenant_id, execution_id, request_digest, output)
            .await
    }

    pub(crate) async fn prepare(&self, tenant_id: Uuid) -> Result<(), PortError> {
        self.keyring.prepare(tenant_id).await
    }

    pub(crate) async fn replay(
        &self,
        tenant_id: Uuid,
        execution_id: Uuid,
        request_digest: &str,
        max_output_bytes: i64,
    ) -> Result<Value, PortError> {
        let result = ai_structured_results::Entity::find()
            .filter(ai_structured_results::Column::TenantId.eq(tenant_id))
            .filter(ai_structured_results::Column::ExecutionId.eq(execution_id))
            .one(&self.database)
            .await
            .map_err(|_| result_unavailable())?
            .ok_or_else(result_unavailable)?;
        let output = self
            .keyring
            .open(
                tenant_id,
                execution_id,
                request_digest,
                max_output_bytes,
                &result,
                Utc::now(),
            )
            .await?;
        let replayed = ai_structured_results::Entity::update_many()
            .col_expr(
                ai_structured_results::Column::ReplayCount,
                sea_orm::sea_query::ExprTrait::add(
                    Expr::col(ai_structured_results::Column::ReplayCount),
                    1,
                ),
            )
            .col_expr(
                ai_structured_results::Column::LastReplayedAt,
                Expr::value(Some(Utc::now())),
            )
            .filter(ai_structured_results::Column::Id.eq(result.id))
            .filter(ai_structured_results::Column::TenantId.eq(tenant_id))
            .exec(&self.database)
            .await
            .map_err(|_| result_unavailable())?;
        if replayed.rows_affected != 1 {
            return Err(result_unavailable());
        }
        Ok(output)
    }
}

pub(crate) async fn delete_expired_results(
    database: &DatabaseConnection,
    now: DateTime<Utc>,
) -> Result<u64, PortError> {
    let expired_ids = ai_structured_results::Entity::find()
        .select_only()
        .column(ai_structured_results::Column::Id)
        .filter(ai_structured_results::Column::ExpiresAt.lte(now))
        .order_by_asc(ai_structured_results::Column::ExpiresAt)
        .order_by_asc(ai_structured_results::Column::Id)
        .limit(RESULT_CLEANUP_BATCH_SIZE)
        .into_tuple::<Uuid>()
        .all(database)
        .await
        .map_err(|_| result_unavailable())?;
    if expired_ids.is_empty() {
        return Ok(0);
    }
    ai_structured_results::Entity::delete_many()
        .filter(ai_structured_results::Column::Id.is_in(expired_ids))
        .filter(ai_structured_results::Column::ExpiresAt.lte(now))
        .exec(database)
        .await
        .map(|result| result.rows_affected)
        .map_err(|_| result_unavailable())
}

fn validate_key_id(key_id: &str) -> Result<(), PortError> {
    if key_id.is_empty()
        || key_id.len() > 64
        || !key_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(PortError::validation(
            "ai.structured.result_key_id_invalid",
            "structured result key IDs must use 1..=64 safe ASCII characters",
        ));
    }
    Ok(())
}

fn result_aad(
    tenant_id: Uuid,
    execution_id: Uuid,
    request_digest: &str,
    output_digest: &str,
    key_id: &str,
) -> String {
    format!(
        "rustok-ai-structured-result:v1:{tenant_id}:{execution_id}:{request_digest}:{output_digest}:{key_id}"
    )
}

fn result_unavailable() -> PortError {
    PortError::unavailable(
        "ai.structured.result_unavailable",
        "structured execution result is temporarily unavailable",
    )
}

fn result_key_unavailable() -> PortError {
    PortError::unavailable(
        "ai.structured.result_key_unavailable",
        "structured result encryption key is unavailable",
    )
}

fn result_key_invalid() -> PortError {
    PortError::invariant_violation(
        "ai.structured.result_key_invalid",
        "structured result encryption key is invalid",
    )
}

fn result_invariant() -> PortError {
    PortError::invariant_violation(
        "ai.structured.result_integrity_invalid",
        "structured execution result integrity validation failed",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyring() -> StructuredResultKeyring {
        StructuredResultKeyring::for_test(
            "v2",
            Duration::from_secs(300),
            BTreeMap::from([
                ("v1".to_string(), [1_u8; 32]),
                ("v2".to_string(), [2_u8; 32]),
            ]),
        )
    }

    #[tokio::test]
    async fn encrypted_result_round_trips_and_binds_identity() {
        let tenant_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();
        let request_digest = &"a".repeat(64);
        let output = serde_json::json!({"translated": "hello"});
        let sealed = keyring()
            .seal(tenant_id, execution_id, request_digest, &output)
            .await
            .unwrap();
        assert_ne!(sealed.ciphertext, serde_json::to_vec(&output).unwrap());
        assert_eq!(sealed.key_id, "v2");
        let model = ai_structured_results::Model {
            id: Uuid::new_v4(),
            tenant_id,
            execution_id,
            request_digest: sealed.request_digest,
            output_digest: sealed.output_digest,
            key_id: sealed.key_id,
            nonce: sealed.nonce,
            ciphertext: sealed.ciphertext,
            plaintext_bytes: sealed.plaintext_bytes,
            replay_count: 0,
            created_at: sealed.created_at.into(),
            expires_at: sealed.expires_at.into(),
            last_replayed_at: None,
        };
        assert_eq!(
            keyring()
                .open(
                    tenant_id,
                    execution_id,
                    request_digest,
                    1024,
                    &model,
                    Utc::now(),
                )
                .await
                .unwrap(),
            output
        );
        assert_eq!(
            keyring()
                .open(
                    Uuid::new_v4(),
                    execution_id,
                    request_digest,
                    1024,
                    &model,
                    Utc::now()
                )
                .await
                .unwrap_err()
                .code,
            "ai.structured.result_integrity_invalid"
        );
    }

    #[tokio::test]
    async fn retained_key_decrypts_after_rotation_and_expiry_fails_closed() {
        let tenant_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();
        let request_digest = &"b".repeat(64);
        let old = StructuredResultKeyring::for_test(
            "v1",
            Duration::from_secs(300),
            BTreeMap::from([("v1".to_string(), [1_u8; 32])]),
        );
        let sealed = old
            .seal(
                tenant_id,
                execution_id,
                request_digest,
                &serde_json::json!({"ok": true}),
            )
            .await
            .unwrap();
        let model = ai_structured_results::Model {
            id: Uuid::new_v4(),
            tenant_id,
            execution_id,
            request_digest: sealed.request_digest,
            output_digest: sealed.output_digest,
            key_id: sealed.key_id,
            nonce: sealed.nonce,
            ciphertext: sealed.ciphertext,
            plaintext_bytes: sealed.plaintext_bytes,
            replay_count: 0,
            created_at: sealed.created_at.into(),
            expires_at: sealed.expires_at.into(),
            last_replayed_at: None,
        };
        assert!(
            keyring()
                .open(
                    tenant_id,
                    execution_id,
                    request_digest,
                    1024,
                    &model,
                    Utc::now(),
                )
                .await
                .is_ok()
        );
        assert_eq!(
            keyring()
                .open(
                    tenant_id,
                    execution_id,
                    request_digest,
                    1024,
                    &model,
                    model.expires_at.with_timezone(&Utc)
                )
                .await
                .unwrap_err()
                .code,
            "ai.structured.result_expired"
        );
    }
}
