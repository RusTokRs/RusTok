use crate::error::RbacError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RbacAuthzMode {
    RelationOnly,
    DualRead,
}

const AUTHZ_MODE_ENV: &str = "RUSTOK_RBAC_AUTHZ_MODE";
const RELATION_DUAL_READ_FLAG_ALIASES: [&str; 3] = [
    "RUSTOK_RBAC_RELATION_DUAL_READ_ENABLED",
    "RBAC_RELATION_DUAL_READ_ENABLED",
    "rbac_relation_dual_read_enabled",
];

impl RbacAuthzMode {
    pub fn try_parse(value: &str) -> Result<Self, RbacError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dual_read" | "dual-read" | "dual" => Ok(Self::DualRead),
            "relation_only" | "relation-only" | "relation" => Ok(Self::RelationOnly),
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

        if RELATION_DUAL_READ_FLAG_ALIASES
            .iter()
            .any(|name| env_flag_enabled(name))
        {
            return Self::DualRead;
        }

        Self::RelationOnly
    }

    pub fn is_dual_read(self) -> bool {
        self == Self::DualRead
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
    use super::{RbacAuthzMode, AUTHZ_MODE_ENV, RELATION_DUAL_READ_FLAG_ALIASES};
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
            // SAFETY: tests serialize environment mutations via `EnvVarGuard` lock.
            unsafe {
                std::env::set_var(self.name, value);
            }
        }

        fn remove(&self) {
            // SAFETY: tests serialize environment mutations via `EnvVarGuard` lock.
            unsafe {
                std::env::remove_var(self.name);
            }
        }

        fn restore(&self) {
            if let Some(previous) = self.previous.as_ref() {
                // SAFETY: tests serialize environment mutations via `EnvVarGuard` lock.
                unsafe {
                    std::env::set_var(self.name, previous);
                }
            } else {
                // SAFETY: tests serialize environment mutations via `EnvVarGuard` lock.
                unsafe {
                    std::env::remove_var(self.name);
                }
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
    fn authz_mode_env_has_priority_over_legacy_flag_aliases() {
        let _lock = env_lock();
        let mode_env = EnvVarGuard::capture(AUTHZ_MODE_ENV);
        mode_env.set("relation_only");

        for name in RELATION_DUAL_READ_FLAG_ALIASES {
            let alias = EnvVarGuard::capture(name);
            alias.set("true");
            assert_eq!(RbacAuthzMode::from_env(), RbacAuthzMode::RelationOnly);
        }
    }
}
