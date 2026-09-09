//! Browser-safe projections for static module lifecycle state and recovery.
//!
//! The module owner deliberately omits internal override and trace evidence
//! from this contract. GraphQL and native adapters expose this same bounded
//! recovery surface to operator clients.

use serde::{Deserialize, Serialize};

/// Current explicit static-module lifecycle state in one tenant.
///
/// This is a bounded operator projection: the underlying lifecycle aggregate,
/// command receipt, and hook evidence remain owner-private.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct StaticTenantModuleView {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub enabled: bool,
    /// Canonical JSON encoding of the owner-normalized settings object.
    pub settings: String,
    pub revision: i64,
}

/// Recovery facts for one tenant-scoped failed static-module operation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModuleOperationRecoveryPlanView {
    #[serde(rename = "operationId")]
    pub operation_id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    #[serde(rename = "requestedEnabled")]
    pub requested_enabled: bool,
    #[serde(rename = "previousEffectiveEnabled")]
    pub previous_effective_enabled: bool,
    pub status: String,
    pub issue: String,
    pub retryable: bool,
    #[serde(rename = "recommendedAction")]
    pub recommended_action: String,
    #[serde(rename = "correlationId")]
    pub correlation_id: Option<String>,
    #[serde(rename = "requestedBy")]
    pub requested_by: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{ModuleOperationRecoveryPlanView, StaticTenantModuleView};

    #[test]
    fn static_lifecycle_view_uses_the_graphql_field_contract() {
        let view = StaticTenantModuleView {
            module_slug: "forum".to_string(),
            enabled: true,
            settings: r#"{\"visibility\":\"public\"}"#.to_string(),
            revision: 4,
        };

        let encoded = serde_json::to_value(view).expect("static lifecycle view serializes");
        assert_eq!(encoded["moduleSlug"], "forum");
        assert_eq!(encoded["settings"], r#"{\"visibility\":\"public\"}"#);
        assert_eq!(encoded["revision"], 4);
    }

    #[test]
    fn recovery_view_uses_the_graphql_field_contract() {
        let plan = ModuleOperationRecoveryPlanView {
            operation_id: "11111111-1111-1111-1111-111111111111".to_string(),
            tenant_id: "22222222-2222-2222-2222-222222222222".to_string(),
            module_slug: "forum".to_string(),
            requested_enabled: true,
            previous_effective_enabled: false,
            status: "failed".to_string(),
            issue: "post_hook_failed".to_string(),
            retryable: true,
            recommended_action: "retry_post_hook".to_string(),
            correlation_id: Some("correlation".to_string()),
            requested_by: Some("operator".to_string()),
            error_message: Some("hook failed".to_string()),
        };

        let encoded = serde_json::to_value(plan).expect("recovery view serializes");
        assert_eq!(
            encoded["operationId"],
            "11111111-1111-1111-1111-111111111111"
        );
        assert_eq!(encoded["recommendedAction"], "retry_post_hook");
        assert!(encoded.get("traceId").is_none());
    }
}
