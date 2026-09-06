use async_trait::async_trait;
use rustok_modules::{
    ArtifactBlobStore, ArtifactPayloadKind, ModuleArtifactNodeAssignmentWorkItem,
    ModuleInstallationError, StorageArtifactBlobStore,
};
use rustok_runtime::{
    InstanceLayout, ModulePayloadCacheError, materialize_module_payload, record_prepared_module,
};
use rustok_sandbox::{RHAI_SANDBOX_RUNTIME_ABI, rhai::RhaiExecutor, wasm::WasmComponentExecutor};
use rustok_sandbox_transport::GrpcRhaiExecutor;

use crate::{ArtifactNodeMaterializationError, ArtifactNodeMaterializer, NodeArtifactPreparation};

/// Production local materializer. It reads only the platform CAS and the
/// prepared instance root. It does not read a database, select a release, or
/// obtain capabilities, tenants, secrets, or product infrastructure.
pub struct StorageArtifactNodeMaterializer {
    blobs: StorageArtifactBlobStore,
    layout: InstanceLayout,
    rhai_worker: GrpcRhaiExecutor,
    rhai: RhaiExecutor,
    wasm: WasmComponentExecutor,
}

impl StorageArtifactNodeMaterializer {
    pub fn new(
        blobs: StorageArtifactBlobStore,
        layout: InstanceLayout,
        rhai_worker: GrpcRhaiExecutor,
    ) -> Self {
        Self {
            blobs,
            layout,
            rhai_worker,
            rhai: RhaiExecutor::new(),
            wasm: WasmComponentExecutor::new(),
        }
    }

    async fn prepare_local(
        &self,
        work: &ModuleArtifactNodeAssignmentWorkItem,
    ) -> Result<NodeArtifactPreparation, ArtifactNodeMaterializationError> {
        let assignment = &work.assignment;
        validate_assignment_contract(
            assignment.payload_kind,
            &assignment.payload_media_type,
            &assignment.executor_abi,
        )?;
        let bytes = self
            .blobs
            .get_verified(&assignment.payload_digest)
            .await
            .map_err(classify_blob_error)?;
        materialize_module_payload(&self.layout, &assignment.payload_digest, &bytes)
            .map_err(classify_cache_error)?;
        let runtime_fingerprint = match assignment.payload_kind {
            ArtifactPayloadKind::Rhai => {
                self.rhai
                    .prepare_artifact_payload(
                        &assignment.executor_abi,
                        &assignment.payload_media_type,
                        &assignment.payload_digest,
                        &bytes,
                    )
                    .map_err(|_| {
                        ArtifactNodeMaterializationError::terminal(
                            "rhai_payload_invalid",
                            "the owner-assigned Rhai payload cannot be prepared",
                        )
                    })?
                    .runtime_fingerprint
            }
            ArtifactPayloadKind::WasmComponent => {
                self.wasm
                    .prepare_component(&assignment.payload_digest, &assignment.executor_abi, &bytes)
                    .map_err(|_| {
                        ArtifactNodeMaterializationError::terminal(
                            "wasm_component_invalid",
                            "the owner-assigned Wasm Component cannot be prepared",
                        )
                    })?
                    .runtime_fingerprint
            }
            ArtifactPayloadKind::StaticPromoted => {
                return Err(ArtifactNodeMaterializationError::terminal(
                    "static_promotion_assignment_invalid",
                    "static promotion artifacts cannot be assigned to the dynamic node agent",
                ));
            }
            ArtifactPayloadKind::Sidecar => {
                return Err(ArtifactNodeMaterializationError::terminal(
                    "sidecar_assignment_unsupported",
                    "the node agent has no sidecar runtime implementation",
                ));
            }
        };
        record_prepared_module(
            &self.layout,
            &runtime_fingerprint,
            &assignment.payload_digest,
        )
        .map_err(classify_cache_error)?;
        Ok(NodeArtifactPreparation {
            runtime_fingerprint,
        })
    }
}

fn validate_assignment_contract(
    payload_kind: ArtifactPayloadKind,
    payload_media_type: &str,
    executor_abi: &str,
) -> Result<(), ArtifactNodeMaterializationError> {
    if !payload_kind.supports_media_type(payload_media_type) {
        return Err(ArtifactNodeMaterializationError::terminal(
            "payload_media_type_invalid",
            "the owner-assigned payload media type is invalid for its runtime kind",
        ));
    }
    match payload_kind {
        ArtifactPayloadKind::Rhai if executor_abi != RHAI_SANDBOX_RUNTIME_ABI => {
            Err(ArtifactNodeMaterializationError::terminal(
                "rhai_runtime_abi_unsupported",
                "the owner-assigned Rhai runtime ABI is not available on this node",
            ))
        }
        ArtifactPayloadKind::StaticPromoted => Err(ArtifactNodeMaterializationError::terminal(
            "static_promotion_assignment_invalid",
            "static promotion artifacts cannot be assigned to the dynamic node agent",
        )),
        ArtifactPayloadKind::Sidecar => Err(ArtifactNodeMaterializationError::terminal(
            "sidecar_assignment_unsupported",
            "the node agent has no sidecar runtime implementation",
        )),
        ArtifactPayloadKind::Rhai | ArtifactPayloadKind::WasmComponent => Ok(()),
    }
}

#[async_trait]
impl ArtifactNodeMaterializer for StorageArtifactNodeMaterializer {
    async fn prepare(
        &self,
        work: &ModuleArtifactNodeAssignmentWorkItem,
    ) -> Result<NodeArtifactPreparation, ArtifactNodeMaterializationError> {
        self.prepare_local(work).await
    }

    async fn verify_ready(
        &self,
        work: &ModuleArtifactNodeAssignmentWorkItem,
    ) -> Result<NodeArtifactPreparation, ArtifactNodeMaterializationError> {
        let preparation = self.prepare_local(work).await?;
        if work.assignment.payload_kind == ArtifactPayloadKind::Rhai {
            self.rhai_worker.check_readiness().await.map_err(|_| {
                ArtifactNodeMaterializationError::retryable(
                    "the isolated Rhai sandbox worker is not ready",
                )
            })?;
        }
        Ok(preparation)
    }
}

fn classify_blob_error(error: ModuleInstallationError) -> ArtifactNodeMaterializationError {
    match error {
        ModuleInstallationError::BlobNotFound(_) => ArtifactNodeMaterializationError::terminal(
            "cas_payload_unavailable",
            "the owner-assigned admitted payload is absent from durable CAS",
        ),
        ModuleInstallationError::PayloadDigestMismatch { .. } => {
            ArtifactNodeMaterializationError::terminal(
                "cas_payload_integrity_failed",
                "durable CAS returned bytes that do not match the admitted payload digest",
            )
        }
        _ => ArtifactNodeMaterializationError::retryable(
            "durable artifact CAS is temporarily unavailable",
        ),
    }
}

fn classify_cache_error(error: ModulePayloadCacheError) -> ArtifactNodeMaterializationError {
    match error {
        ModulePayloadCacheError::DigestMismatch { .. } => {
            ArtifactNodeMaterializationError::terminal(
                "local_payload_integrity_failed",
                "the node-local payload cache could not preserve the admitted digest",
            )
        }
        ModulePayloadCacheError::UnsafeCacheEntry(_) => ArtifactNodeMaterializationError::terminal(
            "local_payload_cache_unsafe",
            "the node-local payload cache contains an unsafe filesystem entry",
        ),
        ModulePayloadCacheError::InvalidRequest | ModulePayloadCacheError::Layout(_) => {
            ArtifactNodeMaterializationError::terminal(
                "local_payload_cache_invalid",
                "the canonical node-local payload cache cannot represent this assignment",
            )
        }
        ModulePayloadCacheError::InvalidPreparationMarker(_) => {
            ArtifactNodeMaterializationError::terminal(
                "local_preparation_marker_invalid",
                "the node-local preparation marker is not safe to reuse",
            )
        }
        ModulePayloadCacheError::Io { .. } | ModulePayloadCacheError::Serialization { .. } => {
            ArtifactNodeMaterializationError::retryable(
                "the node-local payload cache is temporarily unavailable",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use rustok_modules::{
        ArtifactPayloadKind, MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE,
        MODULE_ARTIFACT_STATIC_PROMOTION_MEDIA_TYPE,
    };
    use rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI;

    use super::{ArtifactNodeMaterializationError, validate_assignment_contract};

    #[test]
    fn assignment_contract_rejects_wrong_media_before_cas_access() {
        let error = validate_assignment_contract(
            ArtifactPayloadKind::WasmComponent,
            MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE,
            "rustok:module/runtime@1",
        )
        .expect_err("media type must match payload kind");
        assert!(matches!(
            error,
            ArtifactNodeMaterializationError::Terminal { ref code, .. }
                if code == "payload_media_type_invalid"
        ));
    }

    #[test]
    fn assignment_contract_rejects_unsupported_runtime_kinds_and_abi() {
        let rhai_error = validate_assignment_contract(
            ArtifactPayloadKind::Rhai,
            MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE,
            "rustok:module/runtime@unsupported",
        )
        .expect_err("unknown Rhai ABI");
        assert!(matches!(
            rhai_error,
            ArtifactNodeMaterializationError::Terminal { ref code, .. }
                if code == "rhai_runtime_abi_unsupported"
        ));
        assert!(
            validate_assignment_contract(
                ArtifactPayloadKind::StaticPromoted,
                MODULE_ARTIFACT_STATIC_PROMOTION_MEDIA_TYPE,
                RHAI_SANDBOX_RUNTIME_ABI,
            )
            .is_err()
        );
    }
}
