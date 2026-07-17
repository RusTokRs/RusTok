//! Host-composed handles shared by artifact durable-work adapters.
//!
//! The module owns its queues, but a host owns both the sandbox-backed
//! executor and tenant enumeration. Keeping these handles explicit prevents a
//! durable worker from bypassing tenant RLS or constructing a fallback runtime.

use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::ArtifactBindingExecutor;

/// Enumerates tenants that a host has made eligible for artifact delivery.
///
/// Implementations must return only concrete tenant identities; adapters
/// validate that no nil identity reaches a tenant-RLS queue.
#[async_trait]
pub trait ArtifactDeliveryTenantSource: Send + Sync {
    async fn tenant_ids(&self) -> Result<Vec<Uuid>, String>;
}

/// The host-owned sandbox-backed dispatcher for immutable artifact bindings.
pub type SharedArtifactBindingExecutor = Arc<dyn ArtifactBindingExecutor>;

/// The host-owned tenant enumerator for durable artifact delivery queues.
pub type SharedArtifactDeliveryTenantSource = Arc<dyn ArtifactDeliveryTenantSource>;
