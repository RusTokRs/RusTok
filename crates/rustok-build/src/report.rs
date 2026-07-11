//! Result contract for build execution.

use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct BuildExecutionReport {
    pub build_id: Uuid,
    pub status: String,
    pub cargo_command: String,
    pub admin_command: Option<String>,
    pub storefront_command: Option<String>,
    pub release_id: Option<String>,
    pub release_status: Option<String>,
}
