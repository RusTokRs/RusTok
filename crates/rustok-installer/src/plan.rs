use serde::{Deserialize, Serialize};

use crate::secrets::{SecretMode, SecretValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallEnvironment {
    Local,
    Demo,
    Test,
    Production,
}

impl InstallEnvironment {
    pub fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallProfile {
    DevLocal,
    Monolith,
    HybridAdmin,
    HeadlessNext,
    HeadlessLeptos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseEngine {
    Postgres,
    Sqlite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub engine: DatabaseEngine,
    pub url: SecretValue,
    #[serde(default)]
    pub create_if_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantBootstrap {
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminBootstrap {
    pub email: String,
    pub password: SecretValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeedProfile {
    None,
    Minimal,
    Dev,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModuleSelection {
    #[serde(default)]
    pub enable: Vec<String>,
    #[serde(default)]
    pub disable: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub environment: InstallEnvironment,
    pub profile: InstallProfile,
    pub database: DatabaseConfig,
    pub tenant: TenantBootstrap,
    pub admin: AdminBootstrap,
    pub modules: ModuleSelection,
    pub seed_profile: SeedProfile,
    pub secrets_mode: SecretMode,
}

impl InstallPlan {
    pub fn production_minimal(
        database_url: SecretValue,
        tenant: TenantBootstrap,
        admin: AdminBootstrap,
    ) -> Self {
        Self {
            environment: InstallEnvironment::Production,
            profile: InstallProfile::Monolith,
            database: DatabaseConfig {
                engine: DatabaseEngine::Postgres,
                url: database_url,
                create_if_missing: false,
            },
            tenant,
            admin,
            modules: ModuleSelection::default(),
            seed_profile: SeedProfile::Minimal,
            secrets_mode: SecretMode::ExternalSecret,
        }
    }
}
