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

    pub fn parse_cli_value(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "demo" => Ok(Self::Demo),
            "test" => Ok(Self::Test),
            "production" | "prod" => Ok(Self::Production),
            _ => Err(format!("unknown install environment `{value}`")),
        }
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

impl InstallProfile {
    pub fn parse_cli_value(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dev_local" | "dev-local" | "dev" => Ok(Self::DevLocal),
            "monolith" => Ok(Self::Monolith),
            "hybrid_admin" | "hybrid-admin" => Ok(Self::HybridAdmin),
            "headless_next" | "headless-next" => Ok(Self::HeadlessNext),
            "headless_leptos" | "headless-leptos" => Ok(Self::HeadlessLeptos),
            _ => Err(format!("unknown install profile `{value}`")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseEngine {
    Postgres,
    Sqlite,
}

impl DatabaseEngine {
    pub fn parse_cli_value(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "postgres" | "postgresql" => Ok(Self::Postgres),
            "sqlite" => Ok(Self::Sqlite),
            _ => Err(format!("unknown database engine `{value}`")),
        }
    }
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

impl SeedProfile {
    /// Parses the stable CLI/environment representation of a seed profile.
    pub fn parse_cli_value(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "minimal" => Ok(Self::Minimal),
            "dev" => Ok(Self::Dev),
            _ => Err(format!("unknown seed profile `{value}`")),
        }
    }

    /// Returns the module slugs enabled by the canonical seed profile.
    ///
    /// Host installers may add or remove modules through their explicit plan
    /// selection, but must not reimplement profile policy locally.
    pub fn default_enabled_modules(self) -> Vec<String> {
        match self {
            Self::Dev => ["content", "commerce", "pages", "blog", "forum", "index"]
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            Self::Minimal | Self::None => Vec::new(),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::SeedProfile;

    #[test]
    fn development_seed_profile_enables_the_canonical_module_set() {
        assert_eq!(
            SeedProfile::Dev.default_enabled_modules(),
            vec![
                "content".to_string(),
                "commerce".to_string(),
                "pages".to_string(),
                "blog".to_string(),
                "forum".to_string(),
                "index".to_string(),
            ]
        );
        assert!(SeedProfile::Minimal.default_enabled_modules().is_empty());
        assert!(SeedProfile::None.default_enabled_modules().is_empty());
    }

    #[test]
    fn seed_profile_parsing_is_normalized_and_rejects_unknown_values() {
        assert_eq!(SeedProfile::parse_cli_value(" DEV "), Ok(SeedProfile::Dev));
        assert_eq!(
            SeedProfile::parse_cli_value("unknown").unwrap_err(),
            "unknown seed profile `unknown`"
        );
    }

    #[test]
    fn installation_contract_parsers_accept_documented_aliases() {
        assert_eq!(
            InstallEnvironment::parse_cli_value("prod"),
            Ok(InstallEnvironment::Production)
        );
        assert_eq!(
            InstallProfile::parse_cli_value("headless-next"),
            Ok(InstallProfile::HeadlessNext)
        );
        assert_eq!(
            DatabaseEngine::parse_cli_value("postgresql"),
            Ok(DatabaseEngine::Postgres)
        );
    }
}
