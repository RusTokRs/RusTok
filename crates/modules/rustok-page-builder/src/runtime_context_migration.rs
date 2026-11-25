use fly::{
    FlyResult, GrapesJsCodec, RuntimeContextMigration, RuntimeContextMigrationPolicy,
    migrate_runtime_context,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeContextMigrationRequest {
    /// Document the supplied runtime context was authored against.
    pub source_project_data: Value,
    /// Candidate document the context must be migrated to.
    pub target_project_data: Value,
    #[serde(default)]
    pub context: Value,
    #[serde(default)]
    pub policy: RuntimeContextMigrationPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeContextMigrationResponse {
    pub migration: RuntimeContextMigration,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PageBuilderRuntimeContextMigrator;

impl PageBuilderRuntimeContextMigrator {
    pub fn migrate(
        &self,
        request: PageBuilderRuntimeContextMigrationRequest,
    ) -> FlyResult<PageBuilderRuntimeContextMigrationResponse> {
        let source = GrapesJsCodec::decode_value(request.source_project_data)?;
        let target = GrapesJsCodec::decode_value(request.target_project_data)?;
        Ok(PageBuilderRuntimeContextMigrationResponse {
            migration: migrate_runtime_context(&source, &target, &request.context, request.policy),
        })
    }
}

pub fn migrate_page_builder_runtime_context(
    request: PageBuilderRuntimeContextMigrationRequest,
) -> FlyResult<PageBuilderRuntimeContextMigrationResponse> {
    PageBuilderRuntimeContextMigrator.migrate(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn adapter_preserves_payload_rejection_without_materialization() {
        let project_data = json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": [{
                "id": "quantity", "path": "quantity", "kind": "number", "required": true
            }]
        });
        let response = migrate_page_builder_runtime_context(PageBuilderRuntimeContextMigrationRequest {
            source_project_data: project_data.clone(),
            target_project_data: project_data,
            context: json!({ "quantity": "not a number" }),
            policy: RuntimeContextMigrationPolicy {
                apply_target_defaults: false,
                ..Default::default()
            },
        })
        .expect("migration report");
        assert!(!response.migration.is_accepted());
        assert_eq!(response.migration.type_mismatches, 1);
    }
}
