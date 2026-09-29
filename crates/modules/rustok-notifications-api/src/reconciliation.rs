use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{HostRuntimeContext, PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxReconciliationInspectRequest {
    pub recipient_id: Uuid,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxReconciliationInspectPage {
    pub scanned: u16,
    pub unavailable: u16,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[async_trait]
pub trait NotificationInboxReconciliationInspectPort: Send + Sync {
    async fn inspect_page(
        &self,
        context: PortContext,
        request: NotificationInboxReconciliationInspectRequest,
    ) -> Result<NotificationInboxReconciliationInspectPage, PortError>;
}

/// Deferred construction seam for the Notifications reconciliation inspection port.
///
/// Registration happens before DB-backed owner services exist. The executable host supplies the
/// immutable runtime context later, after all module runtime capabilities have been materialized.
pub trait NotificationInboxReconciliationInspectPortFactory: Send + Sync {
    fn build(
        &self,
        host: &HostRuntimeContext,
    ) -> Result<Arc<dyn NotificationInboxReconciliationInspectPort>, PortError>;
}
