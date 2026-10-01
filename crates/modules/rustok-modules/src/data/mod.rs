//! Artifact data infrastructure, storage brokers, and sandboxed capability adapters.
#![allow(unused_imports)]

pub(crate) use async_trait::async_trait;
pub(crate) use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
pub(crate) use bytes::Bytes;
pub(crate) use object_store::{ObjectStoreExt, path::Path};
pub(crate) use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use serde_json::{Value, json};
pub(crate) use sha2::{Digest, Sha256};
pub(crate) use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
pub(crate) use thiserror::Error;
pub(crate) use uuid::Uuid;

pub(crate) use rustok_events::DomainEvent;
pub(crate) use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityResponse, ExecutionPhase,
    SandboxError, SandboxResult, SandboxSubject,
};
pub(crate) use rustok_storage::{ObjectKey, ObjectScope, ObjectZone, StorageRuntime};

pub(crate) use crate::{
    ArtifactBindingDispatch, ArtifactBindingExecutor, ArtifactCapabilityBrokerResolver,
    ArtifactCapabilityExecution, ArtifactDataIndexField, ArtifactDataIndexValueType,
    ArtifactInstallationTarget, ArtifactMigrationCheckpointRequest, ArtifactReleaseRef,
    ControlPlaneInfrastructure, InstalledModuleArtifact, ModuleCommandContext,
    ModuleInstallationScope, ModuleRuntimeBinding, ModuleRuntimeBindingKind,
    artifact_schema::{ArtifactSchemaValidationError, ArtifactSchemaValidatorCache},
    resolve_granted_artifact_capability,
};

pub mod broker;
pub mod capabilities;
pub mod constants;
pub mod error;
pub mod export;
pub mod gc;
pub mod helpers;
pub mod object_capabilities;
pub mod objects;
pub mod objects_persistence;
pub mod purge;
pub mod purge_targets;
pub mod structured_persistence;
pub mod traits;
pub mod types;
pub mod upgrade;
pub mod upload;
pub mod upload_sessions;
pub mod validation;

pub use broker::*;
pub(crate) use capabilities::escape_like_prefix;
pub use capabilities::*;
pub(crate) use constants::*;
pub use error::*;
pub use export::*;
pub use gc::*;
pub(crate) use helpers::*;
pub use object_capabilities::*;
pub use objects::*;
pub(crate) use objects_persistence::*;
pub use purge::*;
pub(crate) use purge_targets::*;
pub(crate) use structured_persistence::*;
pub use traits::*;
pub use types::*;
pub use upgrade::*;
pub use upload::*;
pub(crate) use upload_sessions::*;
pub use validation::*;

#[cfg(test)]
mod tests;
