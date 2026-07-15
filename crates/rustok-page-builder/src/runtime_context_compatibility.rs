use fly::{
    diff_runtime_context_contracts, FlyResult, GrapesJsV1Codec, RuntimeContextContractDiff,
    RuntimeContextContractSnapshot,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeContractSnapshotRequest {
    pub project_data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeContractSnapshotResponse {
    pub snapshot: RuntimeContextContractSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeContractDiffRequest {
    pub previous: RuntimeContextContractSnapshot,
    pub next: RuntimeContextContractSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeProjectContractDiffRequest {
    pub previous_project_data: Value,
    pub next_project_data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeContractDiffResponse {
    pub diff: RuntimeContextContractDiff,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PageBuilderRuntimeContractCompatibilityInspector;

impl PageBuilderRuntimeContractCompatibilityInspector {
    pub fn snapshot(
        &self,
        request: PageBuilderRuntimeContractSnapshotRequest,
    ) -> FlyResult<PageBuilderRuntimeContractSnapshotResponse> {
        let document = GrapesJsV1Codec::decode_value(request.project_data)?;
        Ok(PageBuilderRuntimeContractSnapshotResponse {
            snapshot: RuntimeContextContractSnapshot::from_document(&document),
        })
    }

    pub fn diff(
        &self,
        request: PageBuilderRuntimeContractDiffRequest,
    ) -> PageBuilderRuntimeContractDiffResponse {
        PageBuilderRuntimeContractDiffResponse {
            diff: diff_runtime_context_contracts(&request.previous, &request.next),
        }
    }

    pub fn diff_projects(
        &self,
        request: PageBuilderRuntimeProjectContractDiffRequest,
    ) -> FlyResult<PageBuilderRuntimeContractDiffResponse> {
        let previous = GrapesJsV1Codec::decode_value(request.previous_project_data)?;
        let next = GrapesJsV1Codec::decode_value(request.next_project_data)?;
        Ok(PageBuilderRuntimeContractDiffResponse {
            diff: diff_runtime_context_contracts(
                &RuntimeContextContractSnapshot::from_document(&previous),
                &RuntimeContextContractSnapshot::from_document(&next),
            ),
        })
    }
}

pub fn snapshot_page_builder_runtime_contract(
    request: PageBuilderRuntimeContractSnapshotRequest,
) -> FlyResult<PageBuilderRuntimeContractSnapshotResponse> {
    PageBuilderRuntimeContractCompatibilityInspector.snapshot(request)
}

pub fn diff_page_builder_runtime_contracts(
    request: PageBuilderRuntimeContractDiffRequest,
) -> PageBuilderRuntimeContractDiffResponse {
    PageBuilderRuntimeContractCompatibilityInspector.diff(request)
}

pub fn diff_page_builder_runtime_project_contracts(
    request: PageBuilderRuntimeProjectContractDiffRequest,
) -> FlyResult<PageBuilderRuntimeContractDiffResponse> {
    PageBuilderRuntimeContractCompatibilityInspector.diff_projects(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::{ContractChangeImpact, RuntimeContractCompatibility};
    use serde_json::json;

    fn project(required: bool, default: Option<&str>) -> Value {
        let mut field = serde_json::Map::from_iter([
            ("id".to_string(), json!("title")),
            ("path".to_string(), json!("page.title")),
            ("kind".to_string(), json!("string")),
            ("required".to_string(), json!(required)),
        ]);
        if let Some(default) = default {
            field.insert("default".to_string(), json!(default));
        }
        json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": [Value::Object(field)]
        })
    }

    #[test]
    fn snapshots_are_serializable_and_stable() {
        let response =
            snapshot_page_builder_runtime_contract(PageBuilderRuntimeContractSnapshotRequest {
                project_data: project(false, None),
            })
            .expect("snapshot");
        assert!(response.snapshot.is_valid());
        assert!(!response.snapshot.contract_hash.is_empty());
        let encoded = serde_json::to_value(&response).expect("serialize snapshot");
        let decoded: PageBuilderRuntimeContractSnapshotResponse =
            serde_json::from_value(encoded).expect("deserialize snapshot");
        assert_eq!(decoded, response);
    }

    #[test]
    fn project_diff_classifies_required_change() {
        let response = diff_page_builder_runtime_project_contracts(
            PageBuilderRuntimeProjectContractDiffRequest {
                previous_project_data: project(false, None),
                next_project_data: project(true, None),
            },
        )
        .expect("diff");
        assert_eq!(
            response.diff.compatibility,
            RuntimeContractCompatibility::Breaking
        );
        assert!(response
            .diff
            .changes
            .iter()
            .any(|change| change.impact() == ContractChangeImpact::Breaking));
    }

    #[test]
    fn snapshot_diff_accepts_external_baselines() {
        let previous =
            snapshot_page_builder_runtime_contract(PageBuilderRuntimeContractSnapshotRequest {
                project_data: project(false, None),
            })
            .expect("previous snapshot")
            .snapshot;
        let next =
            snapshot_page_builder_runtime_contract(PageBuilderRuntimeContractSnapshotRequest {
                project_data: project(true, Some("Welcome")),
            })
            .expect("next snapshot")
            .snapshot;
        let response = diff_page_builder_runtime_contracts(PageBuilderRuntimeContractDiffRequest {
            previous,
            next,
        });
        assert_eq!(
            response.diff.compatibility,
            RuntimeContractCompatibility::RequiresReview
        );
    }
}
