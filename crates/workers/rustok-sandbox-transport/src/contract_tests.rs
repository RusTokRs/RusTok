use std::sync::Arc;

use async_trait::async_trait;
use rustok_sandbox::{
    SandboxError, SandboxExecutor, SandboxExecutorKind, SandboxHost, SandboxOutcome,
    SandboxRequest, SandboxResult,
};
use tonic::transport::Channel;

use crate::server::SandboxWorkerReadiness;
use crate::{
    GrpcRhaiExecutor, SANDBOX_WORKER_MAX_MESSAGE_SIZE, SANDBOX_WORKER_PROTOCOL_REVISION,
    SandboxWorkerGrpcService, SandboxWorkerIdentity,
};

struct DummyRhaiExecutor;

#[async_trait]
impl SandboxExecutor for DummyRhaiExecutor {
    fn kind(&self) -> SandboxExecutorKind {
        SandboxExecutorKind::Rhai
    }

    async fn execute(
        &self,
        request: &SandboxRequest,
        _host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        Err(SandboxError::InvalidRequest(format!(
            "dummy executor does not execute {}",
            request.payload.entrypoint
        )))
    }
}

struct DummyReadiness;

#[async_trait]
impl SandboxWorkerReadiness for DummyReadiness {
    async fn check_readiness(&self) -> Result<(), String> {
        Ok(())
    }

    async fn admit_limits(&self, _limits: &rustok_sandbox::SandboxLimits) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains("readme = \"README.md\""),
        "manifest must declare readme"
    );
    assert!(
        manifest.contains("repository.workspace = true"),
        "manifest must inherit repository from workspace"
    );
    assert!(
        manifest.contains("rust-version.workspace = true"),
        "manifest must inherit rust-version from workspace"
    );
    assert!(
        manifest.contains("[package.metadata.docs.rs]"),
        "manifest must configure docs.rs metadata"
    );
}

#[test]
fn crate_api_contract_specification_is_complete() {
    let api_doc = include_str!("../CRATE_API.md");
    assert!(
        api_doc.contains("# `rustok-sandbox-transport` Crate API & Architecture Contract"),
        "CRATE_API.md must contain the main title"
    );
    assert!(
        api_doc.contains("## 1. Architectural Purpose & Boundaries"),
        "CRATE_API.md must document purpose and boundaries"
    );
    assert!(
        api_doc.contains("## 2. Wire Protocol & Streaming State Machine"),
        "CRATE_API.md must document wire protocol"
    );
    assert!(
        api_doc.contains("## 3. Worker Identity & Deployment Readiness"),
        "CRATE_API.md must document worker identity"
    );
    assert!(
        api_doc.contains("## 4. Universal Logic Dimensions Verification"),
        "CRATE_API.md must document universal logic dimensions"
    );
}

#[tokio::test]
async fn debug_formatting_implementations() {
    let channel = Channel::from_static("http://127.0.0.1:50051").connect_lazy();
    let executor = GrpcRhaiExecutor::from_channel(channel);
    let debug_executor = format!("{executor:?}");
    assert!(debug_executor.contains("GrpcRhaiExecutor"));

    let service =
        SandboxWorkerGrpcService::new(Arc::new(DummyRhaiExecutor), Arc::new(DummyReadiness))
            .expect("worker service");
    let debug_service = format!("{service:?}");
    assert!(debug_service.contains("SandboxWorkerGrpcService"));
    assert!(debug_service.contains("Rhai"));
}

#[test]
fn protocol_revision_and_message_size_constants() {
    assert_eq!(SANDBOX_WORKER_PROTOCOL_REVISION, 1);
    assert_eq!(SANDBOX_WORKER_MAX_MESSAGE_SIZE, 72 * 1024 * 1024);
}

#[test]
fn worker_identity_contract() {
    let identity = SandboxWorkerIdentity {
        executor_binary_digest: "sha256:1111".to_string(),
        engine_build_digest: "sha256:2222".to_string(),
        worker_image_digest: "sha256:3333".to_string(),
        engine_config_digest: "sha256:4444".to_string(),
        target_cpu_contract: "x86_64-v3".to_string(),
        runtime_abi: "rustok:module/runtime@1".to_string(),
        isolation_runtime: "containerd".to_string(),
    };

    assert_eq!(identity.executor_binary_digest, "sha256:1111");
    assert_eq!(identity.engine_build_digest, "sha256:2222");
    assert_eq!(identity.worker_image_digest, "sha256:3333");
    assert_eq!(identity.engine_config_digest, "sha256:4444");
    assert_eq!(identity.target_cpu_contract, "x86_64-v3");
    assert_eq!(identity.runtime_abi, "rustok:module/runtime@1");
    assert_eq!(identity.isolation_runtime, "containerd");

    let clone = identity.clone();
    assert_eq!(clone, identity);

    let default = SandboxWorkerIdentity::default();
    assert!(default.executor_binary_digest.is_empty());
}
