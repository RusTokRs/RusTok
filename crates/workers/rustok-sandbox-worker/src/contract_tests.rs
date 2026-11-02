use std::sync::Arc;

use async_trait::async_trait;
use rustok_sandbox::{
    SandboxExecutor, SandboxExecutorKind, SandboxHost, SandboxOutcome, SandboxRequest,
    SandboxResult,
};
use rustok_sandbox_transport::SandboxWorkerReadiness;

use crate::{
    HardenedRuntime, ObservedRhaiExecutor, ObservedWorkerReadiness, WorkerMemoryObserver,
    is_sha256_digest,
};

#[derive(Debug)]
struct DummyExecutor;

#[async_trait]
impl SandboxExecutor for DummyExecutor {
    fn kind(&self) -> SandboxExecutorKind {
        SandboxExecutorKind::Rhai
    }

    async fn execute(
        &self,
        request: &SandboxRequest,
        _host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        Err(rustok_sandbox::SandboxError::InvalidRequest(format!(
            "dummy executor does not execute {}",
            request.payload.entrypoint
        )))
    }
}

#[derive(Debug)]
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

struct StaticProbe(u64);

impl crate::MemoryProbe for StaticProbe {
    fn current_bytes(&self) -> Result<u64, String> {
        Ok(self.0)
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
        api_doc.contains("# `rustok-sandbox-worker` Crate API & Architecture Contract"),
        "CRATE_API.md must contain the main title"
    );
    assert!(
        api_doc.contains("## 1. Architectural Purpose & Boundaries"),
        "CRATE_API.md must document purpose and boundaries"
    );
    assert!(
        api_doc.contains("## 2. Hardened Runtime Isolation & Attestation"),
        "CRATE_API.md must document runtime isolation"
    );
    assert!(
        api_doc.contains("## 3. Cgroup v2 Memory Observation & Peak Sampling"),
        "CRATE_API.md must document memory observation"
    );
    assert!(
        api_doc.contains("## 4. Readiness Probing & Pool Composition"),
        "CRATE_API.md must document readiness probing"
    );
    assert!(
        api_doc.contains("## 5. Universal Logic Dimensions Verification"),
        "CRATE_API.md must document universal logic dimensions"
    );
}

#[test]
fn debug_formatting_implementations() {
    let memory = WorkerMemoryObserver::with_probe(Arc::new(StaticProbe(1024)));
    let debug_memory = format!("{memory:?}");
    assert!(debug_memory.contains("WorkerMemoryObserver"));

    let executor = ObservedRhaiExecutor::new(DummyExecutor, memory.clone());
    let debug_executor = format!("{executor:?}");
    assert!(debug_executor.contains("ObservedRhaiExecutor"));
    assert!(debug_executor.contains("DummyExecutor"));

    let readiness = ObservedWorkerReadiness::new(DummyReadiness, memory);
    let debug_readiness = format!("{readiness:?}");
    assert!(debug_readiness.contains("ObservedWorkerReadiness"));
    assert!(debug_readiness.contains("DummyReadiness"));
}

#[test]
fn hardened_runtime_parser_invariants() {
    assert_eq!(
        HardenedRuntime::parse("gvisor").unwrap(),
        HardenedRuntime::Gvisor
    );
    assert_eq!(
        HardenedRuntime::parse("GVISOR").unwrap(),
        HardenedRuntime::Gvisor
    );
    assert_eq!(
        HardenedRuntime::parse("  kata  ").unwrap(),
        HardenedRuntime::Kata
    );
    assert!(HardenedRuntime::parse("docker").is_err());
    assert!(HardenedRuntime::parse("").is_err());
}

#[test]
fn sha256_digest_validator_invariants() {
    let valid = format!("sha256:{}", "f".repeat(64));
    assert!(is_sha256_digest(&valid));

    assert!(!is_sha256_digest(""));
    assert!(!is_sha256_digest("sha256:"));
    assert!(!is_sha256_digest(&format!("sha256:{}", "f".repeat(63))));
    assert!(!is_sha256_digest(&format!("sha256:{}", "f".repeat(65))));
    assert!(!is_sha256_digest(&format!("sha256:{}g", "f".repeat(63))));
    assert!(!is_sha256_digest(&format!("md5:{}", "f".repeat(32))));
}
