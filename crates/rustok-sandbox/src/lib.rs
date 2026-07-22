//! Neutral sandbox execution contracts shared by Alloy and module artifacts.

/// Stable runtime ABI used by Rhai module artifacts and Alloy publication
/// smoke executions. The sandbox owns this cross-boundary identity.
pub const RHAI_SANDBOX_RUNTIME_ABI: &str = "rustok:module/runtime@1";

mod admission;
mod capability;
mod error;
mod executor;
mod harness;
mod policy;
mod rhai_binding;
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
pub use harness::{FixtureCapabilityBroker, LocalSandboxHarness};
pub use policy::{SandboxLimits, SandboxPolicy};
#[cfg(feature = "rhai")]
pub use rhai::RhaiCapabilityBridge;
pub use rhai_binding::{
    RHAI_BINDING_VERSION, RhaiBindingError, RhaiBindingInput, RhaiBindingOutput,
};
pub use runtime::{ExecutionObserver, NoopExecutionObserver, SandboxRuntime};
pub use types::{
    ExecutionMetrics, ExecutionPhase, ExecutionRecord, ExecutionStatus, SandboxCancellation,
    SandboxContext, SandboxExecutorKind, SandboxOutcome, SandboxPayload, SandboxRequest,
    SandboxSubject,
};
