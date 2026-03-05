use crate::error::RbacError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthzEngine {
    Relation,
    Casbin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RbacAuthzMode {
    RelationOnly,
    DualRead,
    CasbinShadow,
    CasbinOnly,
}

const AUTHZ_MODE_ENV: &str = "RUSTOK_RBAC_AUTHZ_MODE";
const RELATION_DUAL_READ_FLAG_ALIASES: [&str; 3] = [
    "RUSTOK_RBAC_RELATION_DUAL_READ_ENABLED",
    "RBAC_RELATION_DUAL_READ_ENABLED",
    "rbac_relation_dual_read_enabled",
];
const RELATION_ENFORCEMENT_FLAG_ALIASES: [&str; 3] = [
    "RUSTOK_RBAC_RELATION_ENFORCEMENT_ENABLED",
    "RBAC_RELATION_ENFORCEMENT_ENABLED",
    "rbac_relation_enforcement_enabled",
];
const LEGACY_ROLE_FALLBACK_FLAG_ALIASES: [&str; 3] = [
    "RUSTOK_RBAC_LEGACY_ROLE_FALLBACK_ENABLED",
    "RBAC_LEGACY_ROLE_FALLBACK_ENABLED",
    "rbac_legacy_role_fallback_enabled",
];
const CASBIN_SHADOW_FLAG_ALIASES: [&str; 3] = [
    "RUSTOK_RBAC_CASBIN_SHADOW_ENABLED",
    "RBAC_CASBIN_SHADOW_ENABLED",
    "rbac_casbin_shadow_enabled",
];
const CASBIN_ENFORCEMENT_FLAG_ALIASES: [&str; 3] = [
    "RUSTOK_RBAC_CASBIN_ENFORCEMENT_ENABLED",
    "RBAC_CASBIN_ENFORCEMENT_ENABLED",
    "rbac_casbin_enforcement_enabled",
];

impl RbacAuthzMode {
    pub fn try_parse(value: &str) -> Result<Self, RbacError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dual_read" | "dual-read" | "dual" => Ok(Self::DualRead),
            "relation_only" | "relation-only" | "relation" => Ok(Self::RelationOnly),
            "casbin_shadow" | "casbin-shadow" | "casbin_shadow_read" => Ok(Self::CasbinShadow),
            "casbin_only" | "casbin-only" | "casbin" => Ok(Self::CasbinOnly),
            _ => Err(RbacError::InvalidAuthzMode {
                value: value.to_string(),
            }),
        }
    }

    pub fn parse(value: &str) -> Self {
        Self::try_parse(value).unwrap_or(Self::RelationOnly)
    }

    pub fn from_env() -> Self {
        if let Ok(raw_mode) = std::env::var(AUTHZ_MODE_ENV) {
            return Self::parse(&raw_mode);
        }

        if CASBIN_ENFORCEMENT_FLAG_ALIASES
            .iter()
            .any(|name| env_flag_enabled(name))
        {
            return Self::CasbinOnly;
        }

        if CASBIN_SHADOW_FLAG_ALIASES
            .iter()
            .any(|name| env_flag_enabled(name))
        {
            return Self::CasbinShadow;
        }

        if RELATION_DUAL_READ_FLAG_ALIASES
            .iter()
            .any(|name| env_flag_enabled(name))
        {
            return Self::DualRead;
        }

        if RELATION_ENFORCEMENT_FLAG_ALIASES
            .iter()
            .any(|name| env_flag_enabled(name))
        {
            return Self::RelationOnly;
        }

        if LEGACY_ROLE_FALLBACK_FLAG_ALIASES
            .iter()
            .any(|name| env_flag_enabled(name))
        {
            return Self::DualRead;
        }

        Self::RelationOnly
    }

    pub fn active_engine(self) -> AuthzEngine {
        match self {
            Self::CasbinOnly => AuthzEngine::Casbin,
            Self::RelationOnly | Self::DualRead | Self::CasbinShadow => AuthzEngine::Relation,
        }
    }

    pub fn is_dual_read(self) -> bool {
        self == Self::DualRead
    }

    pub fn is_casbin_shadow(self) -> bool {
        self == Self::CasbinShadow
    }

    pub fn is_casbin_only(self) -> bool {
        self == Self::CasbinOnly
    }

    pub fn should_run_legacy_role_shadow(self) -> bool {
        self == Self::DualRead
    }

    pub fn should_run_casbin_shadow(self) -> bool {
        self == Self::CasbinShadow
    }
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|raw| {
            matches!(
                raw.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "enabled"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{
        AuthzEngine, RbacAuthzMode, AUTHZ_MODE_ENV, CASBIN_ENFORCEMENT_FLAG_ALIASES,
        CASBIN_SHADOW_FLAG_ALIASES, LEGACY_ROLE_FALLBACK_FLAG_ALIASES,
        RELATION_DUAL_READ_FLAG_ALIASES, RELATION_ENFORCEMENT_FLAG_ALIASES,
    };
    use crate::error::RbacError;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    struct EnvVarGuard {
        name: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn capture(name: &'static str) -> Self {
            let previous = std::env::var(name).ok();
            Self { name, previous }
        }

        fn set(&self, value: &str) {
            unsafe { std::env::set_var(self.name, value) };
        }

        fn remove(&self) {
            unsafe { std::env::remove_var(self.name) };
        }

        fn restore(&self) {
            if let Some(previous) = self.previous.as_ref() {
                unsafe { std::env::set_var(self.name, previous) };
            } else {
                unsafe { std::env::remove_var(self.name) };
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            self.restore();
        }
    }

    #[test]
    fn parse_dual_read_case_insensitive() {
        assert_eq!(RbacAuthzMode::parse("DUAL_READ"), RbacAuthzMode::DualRead);
    }

    #[test]
    fn parse_defaults_to_relation_only() {
        assert_eq!(RbacAuthzMode::parse("legacy"), RbacAuthzMode::RelationOnly);
    }

    #[test]
    fn try_parse_returns_validation_error_for_unknown_mode() {
        assert_eq!(
            RbacAuthzMode::try_parse("legacy"),
            Err(RbacError::InvalidAuthzMode {
                value: "legacy".to_string(),
            }),
        );
    }

    #[test]
    fn parse_supports_dual_read_aliases() {
        assert_eq!(RbacAuthzMode::parse("dual-read"), RbacAuthzMode::DualRead);
        assert_eq!(RbacAuthzMode::parse("dual"), RbacAuthzMode::DualRead);
    }

    #[test]
    fn parse_supports_relation_only_aliases() {
        assert_eq!(
            RbacAuthzMode::parse("relation_only"),
            RbacAuthzMode::RelationOnly
        );
        assert_eq!(
            RbacAuthzMode::parse("relation-only"),
            RbacAuthzMode::RelationOnly
        );
        assert_eq!(
            RbacAuthzMode::parse("relation"),
            RbacAuthzMode::RelationOnly
        );
    }

    #[test]
    fn parse_supports_casbin_modes() {
        assert_eq!(
            RbacAuthzMode::parse("casbin-shadow"),
            RbacAuthzMode::CasbinShadow
        );
        assert_eq!(RbacAuthzMode::parse("casbin"), RbacAuthzMode::CasbinOnly);
    }

    #[test]
    fn from_env_supports_all_legacy_dual_read_flag_aliases() {
        let _lock = env_lock();
        let mode_env = EnvVarGuard::capture(AUTHZ_MODE_ENV);
        mode_env.remove();

        for name in RELATION_DUAL_READ_FLAG_ALIASES {
            let alias = EnvVarGuard::capture(name);
            alias.set("true");
            assert_eq!(RbacAuthzMode::from_env(), RbacAuthzMode::DualRead);
        }
    }

    #[test]
    fn from_env_supports_all_relation_enforcement_flag_aliases() {
        let _lock = env_lock();
        let mode_env = EnvVarGuard::capture(AUTHZ_MODE_ENV);
        mode_env.remove();

        for name in RELATION_ENFORCEMENT_FLAG_ALIASES {
            let alias = EnvVarGuard::capture(name);
            alias.set("true");
            assert_eq!(RbacAuthzMode::from_env(), RbacAuthzMode::RelationOnly);
        }
    }

    #[test]
    fn from_env_supports_legacy_role_fallback_aliases_as_dual_read_shadow() {
        let _lock = env_lock();
        let mode_env = EnvVarGuard::capture(AUTHZ_MODE_ENV);
        mode_env.remove();

        for name in LEGACY_ROLE_FALLBACK_FLAG_ALIASES {
            let alias = EnvVarGuard::capture(name);
            alias.set("true");
            assert_eq!(RbacAuthzMode::from_env(), RbacAuthzMode::DualRead);
        }
    }

    #[test]
    fn from_env_supports_casbin_shadow_aliases() {
        let _lock = env_lock();
        let mode_env = EnvVarGuard::capture(AUTHZ_MODE_ENV);
        mode_env.remove();

        for name in CASBIN_SHADOW_FLAG_ALIASES {
            let alias = EnvVarGuard::capture(name);
            alias.set("true");
            assert_eq!(RbacAuthzMode::from_env(), RbacAuthzMode::CasbinShadow);
        }
    }

    #[test]
    fn from_env_supports_casbin_enforcement_aliases() {
        let _lock = env_lock();
        let mode_env = EnvVarGuard::capture(AUTHZ_MODE_ENV);
        mode_env.remove();

        for name in CASBIN_ENFORCEMENT_FLAG_ALIASES {
            let alias = EnvVarGuard::capture(name);
            alias.set("true");
            assert_eq!(RbacAuthzMode::from_env(), RbacAuthzMode::CasbinOnly);
        }
    }

    #[test]
    fn authz_mode_env_has_priority_over_flag_aliases() {
        let _lock = env_lock();
        let mode_env = EnvVarGuard::capture(AUTHZ_MODE_ENV);
        mode_env.set("relation_only");

        for name in CASBIN_ENFORCEMENT_FLAG_ALIASES {
            let alias = EnvVarGuard::capture(name);
            alias.set("true");
            assert_eq!(RbacAuthzMode::from_env(), RbacAuthzMode::RelationOnly);
        }
    }

    #[test]
    fn relation_enforcement_alias_has_priority_over_legacy_fallback_alias() {
        let _lock = env_lock();
        let mode_env = EnvVarGuard::capture(AUTHZ_MODE_ENV);
        mode_env.remove();

        let fallback_alias = EnvVarGuard::capture(LEGACY_ROLE_FALLBACK_FLAG_ALIASES[0]);
        fallback_alias.set("true");
        let relation_enforcement_alias = EnvVarGuard::capture(RELATION_ENFORCEMENT_FLAG_ALIASES[0]);
        relation_enforcement_alias.set("true");

        assert_eq!(RbacAuthzMode::from_env(), RbacAuthzMode::RelationOnly);
    }

    #[test]
    fn relation_dual_read_alias_has_priority_over_relation_enforcement_alias() {
        let _lock = env_lock();
        let mode_env = EnvVarGuard::capture(AUTHZ_MODE_ENV);
        mode_env.remove();

        let relation_enforcement_alias = EnvVarGuard::capture(RELATION_ENFORCEMENT_FLAG_ALIASES[0]);
        relation_enforcement_alias.set("true");
        let dual_alias = EnvVarGuard::capture(RELATION_DUAL_READ_FLAG_ALIASES[0]);
        dual_alias.set("true");

        assert_eq!(RbacAuthzMode::from_env(), RbacAuthzMode::DualRead);
    }

    #[test]
    fn ignores_disabled_alias_values() {
        let _lock = env_lock();
        let mode_env = EnvVarGuard::capture(AUTHZ_MODE_ENV);
        mode_env.remove();

        let disabled_relation_enforcement =
            EnvVarGuard::capture(RELATION_ENFORCEMENT_FLAG_ALIASES[0]);
        disabled_relation_enforcement.set("false");
        let enabled_fallback = EnvVarGuard::capture(LEGACY_ROLE_FALLBACK_FLAG_ALIASES[0]);
        enabled_fallback.set("on");

        assert_eq!(RbacAuthzMode::from_env(), RbacAuthzMode::DualRead);
    }

    #[test]
    fn casbin_enforcement_alias_has_priority_over_casbin_shadow_and_dual_read() {
        let _lock = env_lock();
        let mode_env = EnvVarGuard::capture(AUTHZ_MODE_ENV);
        mode_env.remove();

        let shadow_alias = EnvVarGuard::capture(CASBIN_SHADOW_FLAG_ALIASES[0]);
        shadow_alias.set("true");
        let dual_alias = EnvVarGuard::capture(RELATION_DUAL_READ_FLAG_ALIASES[0]);
        dual_alias.set("true");
        let enforce_alias = EnvVarGuard::capture(CASBIN_ENFORCEMENT_FLAG_ALIASES[0]);
        enforce_alias.set("true");

        assert_eq!(RbacAuthzMode::from_env(), RbacAuthzMode::CasbinOnly);
    }

    #[test]
    fn exposes_active_engine_and_shadow_controls() {
        assert_eq!(
            RbacAuthzMode::DualRead.active_engine(),
            AuthzEngine::Relation
        );
        assert_eq!(
            RbacAuthzMode::CasbinShadow.active_engine(),
            AuthzEngine::Relation
        );
        assert_eq!(
            RbacAuthzMode::CasbinOnly.active_engine(),
            AuthzEngine::Casbin
        );
        assert!(RbacAuthzMode::DualRead.should_run_legacy_role_shadow());
        assert!(RbacAuthzMode::CasbinShadow.should_run_casbin_shadow());
        assert!(!RbacAuthzMode::CasbinOnly.should_run_casbin_shadow());
    }
}
