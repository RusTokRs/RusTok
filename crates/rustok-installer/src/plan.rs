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

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Demo => "demo",
            Self::Test => "test",
            Self::Production => "production",
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

/// Immutable selected-distribution identity bound by a trusted executable host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallComposition {
    pub revision: String,
    pub hash: String,
}

/// Deployment topology selected for one installation plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallTopology {
    pub schema_version: u8,
    pub mode: InstallTopologyMode,
    /// A wizard may submit an unbound topology shape. CLI and HTTP hosts bind
    /// this field from their selected distribution before preflight or apply.
    pub composition: Option<InstallComposition>,
    pub surfaces: Vec<InstallSurface>,
    pub roles: Vec<InstallRoleAssignment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallTopologyMode {
    Monolith,
    Distributed,
}

impl InstallTopologyMode {
    pub fn parse_cli_value(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "monolith" => Ok(Self::Monolith),
            "distributed" => Ok(Self::Distributed),
            _ => Err(format!("unknown install topology `{value}`")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallRole {
    Monolith,
    Api,
    AdminSsr,
    StorefrontSsr,
    Worker,
    Registry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallSurface {
    Api,
    Admin,
    Storefront,
    Worker,
    Registry,
}

impl InstallRole {
    /// Returns whether a process role may own a selected runtime surface.
    ///
    /// A monolith intentionally owns every selected surface. Distributed roles
    /// are single-purpose so an adapter cannot silently deploy an API-capable
    /// process in place of a worker, registry, or SSR surface.
    pub fn supports_surface(self, surface: InstallSurface) -> bool {
        match self {
            Self::Monolith => true,
            Self::Api => surface == InstallSurface::Api,
            Self::AdminSsr => surface == InstallSurface::Admin,
            Self::StorefrontSsr => surface == InstallSurface::Storefront,
            Self::Worker => surface == InstallSurface::Worker,
            Self::Registry => surface == InstallSurface::Registry,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Monolith => "monolith",
            Self::Api => "api",
            Self::AdminSsr => "admin_ssr",
            Self::StorefrontSsr => "storefront_ssr",
            Self::Worker => "worker",
            Self::Registry => "registry",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRoleAssignment {
    pub role: InstallRole,
    pub surfaces: Vec<InstallSurface>,
}

impl InstallTopology {
    pub fn for_mode(mode: InstallTopologyMode) -> Self {
        let surfaces = vec![
            InstallSurface::Api,
            InstallSurface::Admin,
            InstallSurface::Storefront,
            InstallSurface::Worker,
        ];
        let roles = match mode {
            InstallTopologyMode::Monolith => vec![InstallRoleAssignment {
                role: InstallRole::Monolith,
                surfaces: surfaces.clone(),
            }],
            InstallTopologyMode::Distributed => vec![
                InstallRoleAssignment {
                    role: InstallRole::Api,
                    surfaces: vec![InstallSurface::Api],
                },
                InstallRoleAssignment {
                    role: InstallRole::AdminSsr,
                    surfaces: vec![InstallSurface::Admin],
                },
                InstallRoleAssignment {
                    role: InstallRole::StorefrontSsr,
                    surfaces: vec![InstallSurface::Storefront],
                },
                InstallRoleAssignment {
                    role: InstallRole::Worker,
                    surfaces: vec![InstallSurface::Worker],
                },
            ],
        };
        Self {
            schema_version: 1,
            mode,
            composition: None,
            surfaces,
            roles,
        }
    }

    pub fn bind_composition(mut self, revision: String, hash: String) -> Self {
        self.composition = Some(InstallComposition { revision, hash });
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "unsupported install topology schema version `{}`",
                self.schema_version
            ));
        }
        let composition = self.composition.as_ref().ok_or_else(|| {
            "install topology is not bound to a distribution composition".to_string()
        })?;
        if composition.revision.trim().is_empty() {
            return Err("install topology composition revision is required".to_string());
        }
        if composition.hash.len() != 64
            || !composition
                .hash
                .chars()
                .all(|value| value.is_ascii_hexdigit())
        {
            return Err(
                "install topology composition hash must be a SHA-256 hex value".to_string(),
            );
        }
        let mut selected_surfaces = std::collections::BTreeSet::new();
        for surface in &self.surfaces {
            if !selected_surfaces.insert(*surface as u8) {
                return Err("install topology selects a surface more than once".to_string());
            }
        }
        if selected_surfaces.is_empty() {
            return Err("install topology must select at least one surface".to_string());
        }
        let mut assigned_roles = std::collections::BTreeSet::new();
        let mut assigned_surfaces = std::collections::BTreeSet::new();
        for assignment in &self.roles {
            if !assigned_roles.insert(assignment.role as u8) {
                return Err("install topology assigns a role more than once".to_string());
            }
            if self.mode == InstallTopologyMode::Monolith
                && assignment.role != InstallRole::Monolith
            {
                return Err("monolith topology may only use the monolith role".to_string());
            }
            if self.mode == InstallTopologyMode::Distributed
                && assignment.role == InstallRole::Monolith
            {
                return Err("distributed topology may not use the monolith role".to_string());
            }
            if assignment.surfaces.is_empty() {
                return Err(
                    "install topology role assignment must own at least one surface".to_string(),
                );
            }
            for surface in &assignment.surfaces {
                let key = *surface as u8;
                if !selected_surfaces.contains(&key) || !assigned_surfaces.insert(key) {
                    return Err("install topology assigns a surface more than once or outside its selection".to_string());
                }
                if !assignment.role.supports_surface(*surface) {
                    return Err(format!(
                        "install role `{}` may not own surface `{}`",
                        serde_name(assignment.role),
                        serde_name(*surface),
                    ));
                }
            }
        }
        if self.mode == InstallTopologyMode::Monolith && self.roles.len() != 1 {
            return Err("monolith topology requires exactly one role".to_string());
        }
        if self.mode == InstallTopologyMode::Distributed && self.roles.len() < 2 {
            return Err("distributed topology requires at least two roles".to_string());
        }
        if assigned_surfaces != selected_surfaces {
            return Err(
                "install topology leaves a selected surface without exactly one owner".to_string(),
            );
        }
        Ok(())
    }
}

fn serde_name<T: Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .expect("install topology enum serialization must be infallible")
        .as_str()
        .expect("install topology enum serialization must produce a string")
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub environment: InstallEnvironment,
    pub profile: InstallProfile,
    pub database: DatabaseConfig,
    pub tenant: TenantBootstrap,
    pub admin: AdminBootstrap,
    pub modules: ModuleSelection,
    pub topology: InstallTopology,
    pub seed_profile: SeedProfile,
    pub secrets_mode: SecretMode,
}

impl InstallPlan {
    pub fn production_minimal(
        database_url: SecretValue,
        tenant: TenantBootstrap,
        admin: AdminBootstrap,
        composition: InstallComposition,
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
            topology: InstallTopology::for_mode(InstallTopologyMode::Monolith)
                .bind_composition(composition.revision, composition.hash),
            seed_profile: SeedProfile::Minimal,
            secrets_mode: SecretMode::ExternalSecret,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InstallRole, InstallSurface, InstallTopology, InstallTopologyMode, SeedProfile};

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

    #[test]
    fn topology_requires_bound_composition_and_exact_surface_ownership() {
        let unbound = InstallTopology::for_mode(InstallTopologyMode::Monolith);
        assert!(unbound.validate().is_err());

        let distributed = InstallTopology::for_mode(InstallTopologyMode::Distributed)
            .bind_composition("distribution@1".to_string(), "a".repeat(64));
        assert!(distributed.validate().is_ok());
    }

    #[test]
    fn distributed_topology_rejects_a_role_owning_another_roles_surface() {
        let mut topology = InstallTopology::for_mode(InstallTopologyMode::Distributed)
            .bind_composition("distribution@1".to_string(), "a".repeat(64));
        let api = topology
            .roles
            .iter_mut()
            .find(|assignment| assignment.role == InstallRole::Api)
            .unwrap();
        api.surfaces = vec![InstallSurface::Worker];

        assert_eq!(
            topology.validate().unwrap_err(),
            "install role `api` may not own surface `worker`"
        );
    }
}
