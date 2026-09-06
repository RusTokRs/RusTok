//! Framework-neutral snapshots for platform composition builds and releases.

use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

macro_rules! platform_code {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(format!("unknown {} code: {value}", stringify!($name))),
                }
            }
        }
    };
}

platform_code!(PlatformBuildStatus {
    Queued => "QUEUED",
    Running => "RUNNING",
    Success => "SUCCESS",
    Failed => "FAILED",
    Cancelled => "CANCELLED",
});

platform_code!(PlatformBuildStage {
    Pending => "PENDING",
    Checkout => "CHECKOUT",
    Build => "BUILD",
    Test => "TEST",
    Deploy => "DEPLOY",
    Complete => "COMPLETE",
});

platform_code!(PlatformDeploymentProfile {
    Monolith => "MONOLITH",
    ServerWithAdmin => "SERVER_WITH_ADMIN",
    ServerWithStorefront => "SERVER_WITH_STOREFRONT",
    HeadlessApi => "HEADLESS_API",
    Worker => "WORKER",
    Registry => "REGISTRY",
});

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlatformBuildSnapshot {
    pub id: String,
    pub status: PlatformBuildStatus,
    pub stage: PlatformBuildStage,
    pub progress: i32,
    pub profile: PlatformDeploymentProfile,
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
    pub logs_url: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::PlatformBuildSnapshot;

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
}
