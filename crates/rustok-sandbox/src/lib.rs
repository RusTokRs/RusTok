//! Neutral sandbox execution contracts shared by Alloy and module artifacts.

/// Stable runtime ABI used by Rhai module artifacts and Alloy publication
/// smoke executions. The sandbox owns this cross-boundary identity.
pub const RHAI_SANDBOX_RUNTIME_ABI: &str = "rustok:module/runtime@1";
/// Stable OCI layer media type for immutable Rhai source artifacts.
pub const RHAI_SOURCE_MEDIA_TYPE: &str = "application/vnd.rustok.rhai.source.v1";

mod admission;
mod capability;
mod error;
mod executor;
mod harness;
mod policy;
mod rhai_binding;
mod rhai_scope;
mod rhai_workspace;
mod runtime;
mod types;

#[cfg(feature = "rhai")]
pub mod rhai;
#[cfg(feature = "wasm-component")]
pub mod wasm;

pub use admission::SandboxAdmissionLimits;
pub use capability::{
    CapabilityAuditOutcome, CapabilityAuditRecord, CapabilityBroker, CapabilityBrokerRouter,
    CapabilityCall, CapabilityCallContext, CapabilityGrant, CapabilityName, CapabilityObserver,
    CapabilityResponse, DataCapabilityConstraints, EventCapabilityConstraints,
    HttpCapabilityConstraints, McpCapabilityConstraints, McpToolGrant, ObjectCapabilityConstraints,
    SandboxHost, SecretReferenceCapabilityConstraints,
};
pub use error::{SandboxError, SandboxResult};
pub use executor::{ExecutorRegistry, SandboxExecutor};
pub use harness::{
    FixtureCapabilityBroker, LocalCapabilityFixture, LocalSandboxExpectation, LocalSandboxHarness,
    LocalSandboxScenario, LocalSandboxScenarioComparison, LocalSandboxScenarioOutcome,
    LocalSandboxScenarioResult,
};
pub use policy::{SandboxLimits, SandboxPolicy};
#[cfg(feature = "rhai")]
pub use rhai::{RhaiCapabilityBridge, RhaiStandardLibrary};
pub use rhai_binding::{
    RHAI_BINDING_VERSION, RhaiBindingError, RhaiBindingInput, RhaiBindingOutput,
};
pub use rhai_scope::{
    MAX_RHAI_SCOPE_BYTES, MAX_RHAI_SCOPE_CONSTANTS, MAX_RHAI_SCOPE_NAME_BYTES,
    MAX_RHAI_SCOPE_RECORDS, RhaiRecordInput, RhaiScopeError, RhaiScopeInput, RhaiScopeOutput,
};
pub use rhai_workspace::{
    MAX_RHAI_WORKSPACE_BYTES, MAX_RHAI_WORKSPACE_FILE_BYTES, MAX_RHAI_WORKSPACE_FILES,
    MAX_RHAI_WORKSPACE_IMPORT_DEPTH, MAX_RHAI_WORKSPACE_PATH_BYTES, RHAI_WORKSPACE_MEDIA_TYPE,
    RHAI_WORKSPACE_SCHEMA_VERSION, RhaiWorkspace, RhaiWorkspaceError, RhaiWorkspaceFile,
    RhaiWorkspaceFileKind,
};
pub use runtime::{ExecutionObserver, NoopExecutionObserver, SandboxRuntime};
pub use types::{
    ExecutionMetrics, ExecutionPhase, ExecutionRecord, ExecutionStatus, SandboxCancellation,
    SandboxContext, SandboxExecutorKind, SandboxExecutorPlacement, SandboxOutcome, SandboxPayload,
    SandboxRequest, SandboxSubject,
};
