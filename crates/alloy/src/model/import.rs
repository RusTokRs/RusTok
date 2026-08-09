use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use rustok_modules::{ArtifactPayloadKind, ArtifactRelease, ArtifactReleaseRef};

use super::{RhaiWorkspace, Script, ScriptTrigger};

const MAX_IMPORT_NAME_BYTES: usize = 255;
const MAX_IMPORT_ACTOR_BYTES: usize = 255;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlloyPublishedReleaseImportCommand {
    pub tenant_id: Uuid,
    pub release: ArtifactReleaseRef,
    pub draft_name: String,
    pub actor_id: String,
    pub idempotency_key: Uuid,
}

impl AlloyPublishedReleaseImportCommand {
    pub fn validate(&self) -> Result<(), AlloyImportError> {
        self.release
            .validate()
            .map_err(|_| AlloyImportError::InvalidCommand)?;
        if self.tenant_id.is_nil()
            || self.idempotency_key.is_nil()
            || !bounded_value(&self.draft_name, MAX_IMPORT_NAME_BYTES)
            || !bounded_value(&self.actor_id, MAX_IMPORT_ACTOR_BYTES)
        {
            return Err(AlloyImportError::InvalidCommand);
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct AlloyPublishedRhaiSource {
    pub release: ArtifactRelease,
    pub workspace: RhaiWorkspace,
}

impl AlloyPublishedRhaiSource {
    pub fn validate_for(&self, expected: &ArtifactReleaseRef) -> Result<String, AlloyImportError> {
        self.release
            .descriptor
            .validate()
            .map_err(|_| AlloyImportError::IneligibleRelease)?;
        if self.release.descriptor.release_ref() != *expected
            || self.release.descriptor.payload_kind != ArtifactPayloadKind::Rhai
            || self.release.descriptor.runtime_abi != rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI
        {
            return Err(AlloyImportError::IneligibleRelease);
        }
        self.workspace
            .validate_rhai_workspace()
            .map_err(|_| AlloyImportError::InvalidSource)?;
        let source_digest = self
            .workspace
            .digest()
            .map_err(|_| AlloyImportError::InvalidSource)?;
        if self.release.lineage.source_digest != source_digest
            || self.release.descriptor.artifact_digest != source_digest
        {
            return Err(AlloyImportError::InvalidSource);
        }
        Ok(source_digest)
    }
}

#[derive(Clone, Debug)]
pub struct AlloyImportedDraftCommand {
    pub script: Script,
    pub idempotency_key: Uuid,
    pub request_digest: String,
}

impl AlloyImportedDraftCommand {
    pub fn from_source(
        command: &AlloyPublishedReleaseImportCommand,
        source: &AlloyPublishedRhaiSource,
        source_digest: &str,
    ) -> Result<Self, AlloyImportError> {
        let mut script = Script::new(
            command.draft_name.clone(),
            source.workspace.clone(),
            ScriptTrigger::Manual,
        );
        script.tenant_id = command.tenant_id;
        script.author_id = Some(command.actor_id.clone());
        script.parent_release = Some(command.release.clone());

        let request_digest = import_request_digest(command, source_digest)?;
        Ok(Self {
            script,
            idempotency_key: command.idempotency_key,
            request_digest,
        })
    }

    pub fn validate(&self) -> Result<(), AlloyImportError> {
        if self.idempotency_key.is_nil()
            || self.script.tenant_id.is_nil()
            || self.script.version != 1
            || self.script.status != super::ScriptStatus::Draft
            || !matches!(&self.script.trigger, ScriptTrigger::Manual)
            || self.script.run_as_system
            || !self.script.permissions.is_empty()
            || self.script.error_count != 0
            || self.script.last_error_at.is_some()
            || !bounded_value(&self.script.name, MAX_IMPORT_NAME_BYTES)
            || !self
                .script
                .author_id
                .as_deref()
                .is_some_and(|actor| bounded_value(actor, MAX_IMPORT_ACTOR_BYTES))
            || !canonical_sha256(&self.request_digest)
        {
            return Err(AlloyImportError::InvalidCommand);
        }
        self.script
            .workspace
            .validate_rhai_workspace()
            .map_err(|_| AlloyImportError::InvalidSource)?;
        self.script
            .parent_release
            .as_ref()
            .ok_or(AlloyImportError::InvalidCommand)?
            .validate()
            .map_err(|_| AlloyImportError::InvalidCommand)
    }
}

#[derive(Clone, Debug)]
pub struct AlloyImportedDraftResult {
    pub script: Script,
    pub created: bool,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AlloyImportError {
    #[error("Alloy published release import command is invalid")]
    InvalidCommand,
    #[error("published release is not eligible for Alloy Rhai import")]
    IneligibleRelease,
    #[error("published release source does not match its immutable lineage")]
    InvalidSource,
    #[error("published release source could not be loaded: {0}")]
    SourceUnavailable(String),
    #[error("Alloy published release import idempotency key was reused")]
    IdempotencyConflict,
    #[error("an Alloy draft with the requested tenant-scoped name already exists")]
    DraftNameConflict,
    #[error("Alloy published release import storage failed: {0}")]
    Storage(String),
}

fn import_request_digest(
    command: &AlloyPublishedReleaseImportCommand,
    source_digest: &str,
) -> Result<String, AlloyImportError> {
    let encoded = serde_json::to_vec(&serde_json::json!({
        "tenant_id": command.tenant_id,
        "release": command.release,
        "draft_name": command.draft_name,
        "actor_id": command.actor_id,
        "source_digest": source_digest,
    }))
    .map_err(|error| AlloyImportError::Storage(error.to_string()))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(encoded))))
}

fn bounded_value(value: &str, max_bytes: usize) -> bool {
    value == value.trim()
        && !value.is_empty()
        && value.len() <= max_bytes
        && !value.chars().any(char::is_control)
}

fn canonical_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
        && value[7..].bytes().all(|byte| !byte.is_ascii_uppercase())
}
