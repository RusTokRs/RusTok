//! Release persistence model.

use chrono::{DateTime, Datelike, Timelike, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

const MAX_RELEASE_ENVIRONMENT_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum ReleaseStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "deploying")]
    Deploying,
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "rolled_back")]
    RolledBack,
    #[sea_orm(string_value = "failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "releases")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub status: ReleaseStatus,
    pub build_id: Uuid,
    pub environment: String,
    pub container_image: Option<String>,
    pub server_artifact_url: Option<String>,
    pub admin_artifact_url: Option<String>,
    pub storefront_artifact_url: Option<String>,
    pub manifest_hash: String,
    pub manifest_revision: i64,
    pub manifest_snapshot: Json,
    pub modules: Json,
    pub previous_release_id: Option<String>,
    pub deployed_at: Option<DateTime<Utc>>,
    pub rolled_back_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn generate_id() -> String {
        let now = Utc::now();
        format!(
            "rel_{:04}{:02}{:02}_{:02}{:02}{:02}",
            now.year(),
            now.month(),
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        )
    }

    pub fn new(
        build_id: Uuid,
        environment: String,
        manifest_hash: String,
        manifest_revision: i64,
        manifest_snapshot: Json,
        modules: Vec<String>,
    ) -> Self {
        Self {
            id: Self::generate_id(),
            status: ReleaseStatus::Pending,
            build_id,
            environment: normalize_release_environment(&environment),
            container_image: None,
            server_artifact_url: None,
            admin_artifact_url: None,
            storefront_artifact_url: None,
            manifest_hash,
            manifest_revision,
            manifest_snapshot,
            modules: serde_json::to_value(modules).expect("Vec<String> is always valid JSON"),
            previous_release_id: None,
            deployed_at: None,
            rolled_back_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    pub fn mark_deployed(&mut self) {
        self.status = ReleaseStatus::Active;
        self.deployed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    pub fn mark_rolled_back(&mut self) {
        self.status = ReleaseStatus::RolledBack;
        self.rolled_back_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }
}

/// Release environment values are persisted into image tags, rollout metadata
/// and historically into shell command templates. Keep the domain value a
/// bounded deployment identifier so transport input can never introduce shell
/// syntax or path separators at a later execution boundary.
pub fn normalize_release_environment(value: &str) -> String {
    let normalized = value
        .trim()
        .chars()
        .take(MAX_RELEASE_ENVIRONMENT_LEN)
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '.' | '-' => character,
            _ => '-',
        })
        .collect::<String>()
        .trim_matches(['.', '-'])
        .to_string();

    if normalized.is_empty() {
        "default".to_string()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_release_environment;

    #[test]
    fn release_environment_cannot_inject_shell_syntax() {
        assert_eq!(
            normalize_release_environment("prod; curl internal | sh"),
            "prod--curl-internal---sh"
        );
        assert_eq!(normalize_release_environment("../../prod"), "prod");
        assert_eq!(normalize_release_environment("   "), "default");
    }
}