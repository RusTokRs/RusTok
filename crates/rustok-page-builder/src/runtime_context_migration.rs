use fly::{
    migrate_runtime_context, FlyResult, GrapesJsV1Codec, RuntimeContextContractSnapshot,
    RuntimeContextMigrationPolicy, RuntimeContextMigrationResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeContextMigrationRequest {
    pub previous_contract: RuntimeContextContractSnapshot,
    pub next_project_data: Value,
    #[serde(default)]
    pub context: Value,
    #[serde(default)]
    pub policy: RuntimeContextMigrationPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeContextMigrationResponse {
    pub migration: RuntimeContextMigrationResult,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PageBuilderRuntimeContextMigrator;

impl PageBuilderRuntimeContextMigrator {
    pub fn migrate(
        &self,
        request: PageBuilderRuntimeContextMigrationRequest,
    ) -> FlyResult<PageBuilderRuntimeContextMigrationResponse> {
        let next_document = GrapesJsV1Codec::decode_value(request.next_project_data)?;
        Ok(PageBuilderRuntimeContextMigrationResponse {
            migration: migrate_runtime_context(
                &request.previous_contract,
                &next_document,
                &request.context,
                request.policy,
            ),
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
    use fly::{GrapesJsV1Codec, RuntimeContextMigrationOperationKind};
    use serde_json::json;

    fn project(kind: &str, required: bool, default: Option<Value>) -> Value {
        let mut field = serde_json::Map::from_iter([
            ("id".to_string(), json!("count")),
            ("path".to_string(), json!("count")),
            ("kind".to_string(), json!(kind)),
            ("required".to_string(), json!(required)),
        ]);
        if let Some(default) = default {
            field.insert("default".to_string(), default);
        }
        json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": [Value::Object(field)]
        })
    }

    #[test]
    fn consumer_can_migrate_context_between_contracts() {
        let previous_project = project("string", false, None);
        let previous_document = GrapesJsV1Codec::decode_value(previous_project)
            .expect("previous document");
        let response = migrate_page_builder_runtime_context(
            PageBuilderRuntimeContextMigrationRequest {
                previous_contract: RuntimeContextContractSnapshot::from_document(
                    &previous_document,
                ),
                next_project_data: project("number", true, None),
                context: json!({ "count": "42" }),
                policy: RuntimeContextMigrationPolicy {
                    coerce_scalars: true,
                    ..RuntimeContextMigrationPolicy::default()
                },
            },
        )
        .expect("migration response");
        assert!(response.migration.accepted);
        assert_eq!(response.migration.migrated_context["count"], 42.0);
        assert!(response.migration.operations.iter().any(|operation| {
            operation.kind == RuntimeContextMigrationOperationKind::ScalarCoerced
        }));
    }

    #[test]
    fn migration_response_preserves_rejected_result() {
        let previous_project = project("string", false, None);
        let previous_document = GrapesJsV1Codec::decode_value(previous_project)
            .expect("previous document");
        let response = migrate_page_builder_runtime_context(
            PageBuilderRuntimeContextMigrationRequest {
                previous_contract: RuntimeContextContractSnapshot::from_document(
                    &previous_document,
                ),
                next_project_data: project("string", true, None),
                context: json!({}),
                policy: RuntimeContextMigrationPolicy::default(),
            },
        )
        .expect("migration response");
        assert!(!response.migration.accepted);
    }
}
