use std::sync::Arc;
use rustok_modules::{
    ModuleBuildProtocolError, ModuleBuildRequest, ModuleBuildResult, ModuleBuildWorker,
    ModuleBuildWorkerReadiness, ModuleStaticDistributionCompletionOutcome,
    ModuleStaticDistributionExecutor, ModuleStaticDistributionExecutorError,
    ModuleStaticDistributionExecutorReadiness, ModuleStaticDistributionWorkItem,
};
use rustok_worker_transport::WorkerAdmission;
use tonic::Code;

use crate::distribution_server::{executor_status, StaticDistributionGrpcService};
use crate::module_build_proto::{
    ExecuteBuildRequest as MbExecuteBuildRequest,
    ExecuteBuildResponse as MbExecuteBuildResponse,
    ReadinessRequest as MbReadinessRequest,
    ReadinessResponse as MbReadinessResponse,
};
use crate::server::ModuleBuildGrpcService;
use crate::static_distribution_proto::{
    ExecuteBuildRequest as SdExecuteBuildRequest,
    ExecuteBuildResponse as SdExecuteBuildResponse,
    ReadinessRequest as SdReadinessRequest,
    ReadinessResponse as SdReadinessResponse,
};

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest_bytes = include_bytes!("../Cargo.toml");
    let manifest_text = std::str::from_utf8(manifest_bytes).expect("Cargo.toml must be valid UTF-8");

    assert!(manifest_text.contains(r#"name = "rustok-module-build-transport""#));
    assert!(manifest_text.contains(r#"readme = "README.md""#));
    assert!(manifest_text.contains(r#"repository.workspace = true"#));
    assert!(manifest_text.contains(r#"[package.metadata.docs.rs]"#));
}

#[test]
fn crate_api_contract_specification_is_complete() {
    let api_bytes = include_bytes!("../CRATE_API.md");
    let api_text = std::str::from_utf8(api_bytes).expect("CRATE_API.md must be valid UTF-8");

    assert!(api_text.contains("## Role"));
    assert!(api_text.contains("## Public surface"));
    assert!(api_text.contains("## Wire Protocol & Framing"));
    assert!(api_text.contains("## Minimum Contract Set"));
    assert!(api_text.contains("### Input DTOs/Commands"));
    assert!(api_text.contains("### Domain Invariants"));
    assert!(api_text.contains("### Events / Outbox Side Effects"));
    assert!(api_text.contains("### Errors / Failure Codes"));
    assert!(api_text.contains("## Verification & Test Coverage"));
}

#[test]
fn module_build_proto_structures_invariants() {
    let req = MbExecuteBuildRequest {
        module_build_request_json: b"{\"test\": 1}".to_vec(),
    };
    assert_eq!(req.module_build_request_json, b"{\"test\": 1}");

    let resp = MbExecuteBuildResponse {
        module_build_result_json: b"{\"success\": true}".to_vec(),
    };
    assert_eq!(resp.module_build_result_json, b"{\"success\": true}");

    let r_req = MbReadinessRequest {};
    let _ = format!("{r_req:?}");

    let r_resp = MbReadinessResponse { ready: true };
    assert!(r_resp.ready);
}

#[test]
fn static_distribution_proto_structures_invariants() {
    let req = SdExecuteBuildRequest {
        work_item_json: b"{\"item\": 42}".to_vec(),
    };
    assert_eq!(req.work_item_json, b"{\"item\": 42}");

    let resp = SdExecuteBuildResponse {
        outcome_json: b"{\"outcome\": \"ok\"}".to_vec(),
    };
    assert_eq!(resp.outcome_json, b"{\"outcome\": \"ok\"}");

    let r_req = SdReadinessRequest {};
    let _ = format!("{r_req:?}");

    let r_resp = SdReadinessResponse { ready: false };
    assert!(!r_resp.ready);
}

#[test]
fn static_distribution_error_status_mapping() {
    let transport_err = ModuleStaticDistributionExecutorError::Transport("connection reset".to_string());
    let status = executor_status(transport_err);
    assert_eq!(status.code(), Code::Unavailable);
    assert_eq!(status.message(), "connection reset");

    let rejected_err = ModuleStaticDistributionExecutorError::Rejected("policy violation".to_string());
    let status = executor_status(rejected_err);
    assert_eq!(status.code(), Code::FailedPrecondition);
    assert_eq!(status.message(), "policy violation");
}

struct DummyWorker;

#[async_trait::async_trait]
impl ModuleBuildWorker for DummyWorker {
    async fn execute_build(&self, _request: ModuleBuildRequest) -> Result<ModuleBuildResult, ModuleBuildProtocolError> {
        Err(ModuleBuildProtocolError::Transport("dummy".to_string()))
    }
}

impl ModuleBuildWorkerReadiness for DummyWorker {
    fn is_ready(&self) -> bool {
        true
    }
}

struct DummyExecutor;

#[async_trait::async_trait]
impl ModuleStaticDistributionExecutor for DummyExecutor {
    async fn execute(&self, _work_item: ModuleStaticDistributionWorkItem) -> Result<ModuleStaticDistributionCompletionOutcome, ModuleStaticDistributionExecutorError> {
        Err(ModuleStaticDistributionExecutorError::Rejected("dummy".to_string()))
    }
}

impl ModuleStaticDistributionExecutorReadiness for DummyExecutor {
    fn is_ready(&self) -> bool {
        true
    }
}

#[test]
fn service_adapters_debug_formatting() {
    let admission = WorkerAdmission::new(1, std::time::Duration::from_secs(1)).expect("valid admission");
    let service = ModuleBuildGrpcService::new(Arc::new(DummyWorker), admission);
    let debug = format!("{service:?}");
    assert!(debug.contains("ModuleBuildGrpcService"));

    let dist_service = StaticDistributionGrpcService::new(Arc::new(DummyExecutor));
    let dist_debug = format!("{dist_service:?}");
    assert!(dist_debug.contains("StaticDistributionGrpcService"));
}
