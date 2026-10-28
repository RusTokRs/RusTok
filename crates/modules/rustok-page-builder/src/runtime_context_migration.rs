use fly::{
    migrate_runtime_context, FlyResult, GrapesJsCodec, RuntimeContextMigration,
    RuntimeContextMigrationPolicy,
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
