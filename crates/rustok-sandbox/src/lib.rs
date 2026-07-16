//! Neutral sandbox execution contracts shared by Alloy and module artifacts.

mod admission;
mod capability;
mod error;
mod executor;
mod policy;
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
    HttpCapabilityConstraints, McpCapabilityConstraints, McpToolGrant, SandboxHost,
    SecretReferenceCapabilityConstraints,
};
pub use error::{SandboxError, SandboxResult};
pub use executor::{ExecutorRegistry, SandboxExecutor};
pub use policy::{SandboxLimits, SandboxPolicy};
pub use runtime::{ExecutionObserver, NoopExecutionObserver, SandboxRuntime};
pub use types::{
    ExecutionMetrics, ExecutionPhase, ExecutionRecord, ExecutionStatus, SandboxCancellation,
    SandboxContext, SandboxExecutorKind, SandboxOutcome, SandboxPayload, SandboxRequest,
    SandboxSubject,
};
