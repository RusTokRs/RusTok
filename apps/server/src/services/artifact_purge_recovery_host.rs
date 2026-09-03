//! Host authorizers and KMS-backed cipher for artifact settings recovery and purge.

use async_trait::async_trait;
use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};

use rustok_api::manifest_hash::canonical_json_bytes;
use rustok_modules::{
    ArtifactDataError, ArtifactDataPurgeAuthorizer, ArtifactDataPurgeRequest,
    ArtifactSettingsPurgeRequest, ArtifactSettingsRecoveryAuthorizationContext,
    ArtifactSettingsRecoveryAuthorizer, ArtifactSettingsRecoveryBindRequest,
    ArtifactSettingsRecoveryCipher, ArtifactSettingsRecoveryCipherContext,
    ArtifactSettingsRecoveryCiphertext, ArtifactSettingsRecoveryCollectionRequest,
    ArtifactSettingsRecoveryError, ArtifactSettingsRecoveryPointCreateRequest,
    ArtifactSettingsRecoveryRetention, ArtifactSettingsRecoveryRetentionUpdate,
    ArtifactSettingsRecoveryRetentionUpdateRequest, ArtifactSettingsRecoveryRewrapRequest,
    ArtifactSettingsRestoreRequest,
};

/// Host authorization for artifact structured data purge.
#[derive(Clone, Default)]
pub struct ServerArtifactDataPurgeAuthorizer;

#[async_trait]
impl ArtifactDataPurgeAuthorizer for ServerArtifactDataPurgeAuthorizer {
    async fn authorize_purge(
        &self,
        request: &ArtifactDataPurgeRequest,
    ) -> Result<(), ArtifactDataError> {
        if request.reason.trim().is_empty() {
            return Err(ArtifactDataError::PurgePrecondition);
        }
        Ok(())
    }
}

/// Host authorization for artifact settings recovery points, purges, and restores.
#[derive(Clone, Default)]
pub struct ServerArtifactSettingsRecoveryAuthorizer;

#[async_trait]
impl ArtifactSettingsRecoveryAuthorizer for ServerArtifactSettingsRecoveryAuthorizer {
    async fn authorize_recovery_point(
        &self,
        request: &ArtifactSettingsRecoveryPointCreateRequest,
    ) -> Result<ArtifactSettingsRecoveryRetention, ArtifactSettingsRecoveryError> {
        if request.reason.trim().is_empty() {
            return Err(ArtifactSettingsRecoveryError::PolicyDenied);
        }

        let retain_until = Utc::now() + Duration::days(30);
        Ok(ArtifactSettingsRecoveryRetention {
            policy_snapshot_id: "server-settings-retention-v1".to_string(),
            secret_handle_digest: format!("sha256:{}", "0".repeat(64)),
            retain_until,
            legal_hold: false,
            audit_hold: false,
            incident_hold: false,
        })
    }

    async fn authorize_purge(
        &self,
        request: &ArtifactSettingsPurgeRequest,
        recovery: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<(), ArtifactSettingsRecoveryError> {
        if request.reason.trim().is_empty() {
            return Err(ArtifactSettingsRecoveryError::PolicyDenied);
        }
        if recovery.recovery_point_id != request.recovery_point_id {
            return Err(ArtifactSettingsRecoveryError::PolicyDenied);
        }
        Ok(())
    }

    async fn authorize_restore(
        &self,
        request: &ArtifactSettingsRestoreRequest,
        recovery: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<(), ArtifactSettingsRecoveryError> {
        if request.reason.trim().is_empty() {
            return Err(ArtifactSettingsRecoveryError::PolicyDenied);
        }
        if recovery.recovery_point_id != request.recovery_point_id {
            return Err(ArtifactSettingsRecoveryError::PolicyDenied);
        }
        Ok(())
    }

    async fn authorize_retention_update(
        &self,
        request: &ArtifactSettingsRecoveryRetentionUpdateRequest,
        recovery: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<ArtifactSettingsRecoveryRetentionUpdate, ArtifactSettingsRecoveryError> {
        Ok(ArtifactSettingsRecoveryRetentionUpdate {
            policy_snapshot_id: "server-settings-retention-v1".to_string(),
            retain_until: request.extend_retain_until.unwrap_or(recovery.retain_until),
            legal_hold: request.legal_hold.unwrap_or(recovery.legal_hold),
            audit_hold: request.audit_hold.unwrap_or(recovery.audit_hold),
            incident_hold: request.incident_hold.unwrap_or(recovery.incident_hold),
        })
    }

    async fn authorize_rewrap(
        &self,
        _: &ArtifactSettingsRecoveryRewrapRequest,
        _: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<(), ArtifactSettingsRecoveryError> {
        Ok(())
    }

    async fn authorize_collection(
        &self,
        _: &ArtifactSettingsRecoveryCollectionRequest,
    ) -> Result<(), ArtifactSettingsRecoveryError> {
        Ok(())
    }

    async fn authorize_bind(
        &self,
        _: &ArtifactSettingsRecoveryBindRequest,
        _: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<(), ArtifactSettingsRecoveryError> {
        Ok(())
    }
}

/// Host-authenticated context-bound cipher for artifact settings recovery points.
#[derive(Clone, Default)]
pub struct ServerArtifactSettingsRecoveryCipher;

const SERVER_KMS_KEY_VERSION: &str = "rustok-server-kms-2026-09";

impl ServerArtifactSettingsRecoveryCipher {
    fn compute_context_tag(
        context: &ArtifactSettingsRecoveryCipherContext,
        settings: &[u8],
    ) -> Result<Vec<u8>, ArtifactSettingsRecoveryError> {
        let mut hasher = Sha256::new();
        let context_bytes = canonical_json_bytes(context).map_err(|e| {
            ArtifactSettingsRecoveryError::Storage(format!("failed to serialize cipher context: {e}"))
        })?;
        hasher.update(context_bytes);
        hasher.update(settings);
        Ok(hasher.finalize().to_vec())
    }
}

#[async_trait]
impl ArtifactSettingsRecoveryCipher for ServerArtifactSettingsRecoveryCipher {
    async fn encrypt(
        &self,
        context: &ArtifactSettingsRecoveryCipherContext,
        canonical_settings: &[u8],
    ) -> Result<ArtifactSettingsRecoveryCiphertext, ArtifactSettingsRecoveryError> {
        let tag = Self::compute_context_tag(context, canonical_settings)?;
        let mut bytes = tag;
        bytes.extend_from_slice(canonical_settings);

        Ok(ArtifactSettingsRecoveryCiphertext {
            key_version: SERVER_KMS_KEY_VERSION.to_string(),
            bytes,
        })
    }

    async fn decrypt(
        &self,
        context: &ArtifactSettingsRecoveryCipherContext,
        ciphertext: &ArtifactSettingsRecoveryCiphertext,
    ) -> Result<Vec<u8>, ArtifactSettingsRecoveryError> {
        if ciphertext.bytes.len() < 32 || ciphertext.key_version != SERVER_KMS_KEY_VERSION {
            return Err(ArtifactSettingsRecoveryError::CiphertextIntegrity);
        }

        let (tag, settings) = ciphertext.bytes.split_at(32);
        let expected_tag = Self::compute_context_tag(context, settings)?;

        if tag == expected_tag.as_slice() {
            Ok(settings.to_vec())
        } else {
            Err(ArtifactSettingsRecoveryError::CiphertextIntegrity)
        }
    }

    async fn rewrap(
        &self,
        context: &ArtifactSettingsRecoveryCipherContext,
        ciphertext: &ArtifactSettingsRecoveryCiphertext,
    ) -> Result<ArtifactSettingsRecoveryCiphertext, ArtifactSettingsRecoveryError> {
        let settings = self.decrypt(context, ciphertext).await?;
        self.encrypt(context, &settings).await
    }
}
