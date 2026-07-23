//! Framework-neutral snapshots for platform composition builds and releases.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlatformBuildSnapshot {
    pub id: String,
    pub status: String,
    pub stage: String,
    pub progress: i32,
    pub profile: String,
    pub manifest_ref: String,
    pub manifest_hash: String,
    #[serde(default)]
    pub manifest_revision: i64,
    pub modules_delta: String,
    #[serde(default)]
    pub build_command: Option<String>,
    #[serde(default)]
    pub build_features: Vec<String>,
    #[serde(default)]
    pub build_target: Option<String>,
    #[serde(default)]
    pub build_profile: Option<String>,
    pub requested_by: String,
    pub reason: Option<String>,
    pub release_id: Option<String>,
    pub logs_url: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlatformReleaseSnapshot {
    pub id: String,
    pub build_id: String,
    pub status: String,
    pub environment: String,
    #[serde(default)]
    pub container_image: Option<String>,
    #[serde(default)]
    pub server_artifact_url: Option<String>,
    #[serde(default)]
    pub admin_artifact_url: Option<String>,
    #[serde(default)]
    pub storefront_artifact_url: Option<String>,
    pub manifest_hash: String,
    #[serde(default)]
    pub manifest_revision: i64,
    #[serde(default)]
    pub modules: Vec<String>,
    pub previous_release_id: Option<String>,
    pub deployed_at: Option<String>,
    pub rolled_back_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{PlatformBuildSnapshot, PlatformReleaseSnapshot};

    #[test]
    fn browser_build_snapshot_accepts_the_public_graphql_subset() {
        let snapshot: PlatformBuildSnapshot = serde_json::from_value(json!({
            "id": "build-1",
            "status": "SUCCESS",
            "stage": "COMPLETE",
            "progress": 100,
            "profile": "HEADLESS_API",
            "manifestRef": "platform_state:1",
            "manifestHash": "manifest",
            "manifestRevision": 1,
            "modulesDelta": "search",
            "requestedBy": "operator",
            "reason": null,
            "releaseId": "release-1",
            "logsUrl": null,
            "errorMessage": null,
            "startedAt": null,
            "finishedAt": null,
            "createdAt": "2026-07-23T00:00:00Z",
            "updatedAt": "2026-07-23T00:00:01Z"
        }))
        .expect("public GraphQL build subset must deserialize");

        assert!(snapshot.build_features.is_empty());
        assert!(snapshot.build_command.is_none());
    }

    #[test]
    fn browser_release_snapshot_accepts_the_public_graphql_subset() {
        let snapshot: PlatformReleaseSnapshot = serde_json::from_value(json!({
            "id": "release-1",
            "buildId": "build-1",
            "status": "ACTIVE",
            "environment": "production",
            "manifestHash": "manifest",
            "manifestRevision": 1,
            "modules": ["search"],
            "previousReleaseId": null,
            "deployedAt": null,
            "rolledBackAt": null,
            "createdAt": "2026-07-23T00:00:00Z",
            "updatedAt": "2026-07-23T00:00:01Z"
        }))
        .expect("public GraphQL release subset must deserialize");

        assert!(snapshot.container_image.is_none());
        assert!(snapshot.server_artifact_url.is_none());
    }
}
