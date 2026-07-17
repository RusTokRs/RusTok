from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    source = path.read_text()
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: {label}: expected 1 match, got {count}")
    path.write_text(source.replace(old, new, 1))


settings = Path("apps/server/src/common/settings.rs")
replace_once(
    settings,
    '''impl std::fmt::Display for TenantResolutionMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TenantSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
''',
    '''impl std::fmt::Display for TenantResolutionMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TenantRuntimeProfile {
    #[default]
    MultiTenant,
    SingleTenant,
    Development,
}

impl TenantRuntimeProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MultiTenant => "multi_tenant",
            Self::SingleTenant => "single_tenant",
            Self::Development => "development",
        }
    }

    pub const fn derives_tenant_from_request(self) -> bool {
        !matches!(self, Self::SingleTenant)
    }
}

impl std::fmt::Display for TenantRuntimeProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TenantSettings {
    #[serde(default)]
    pub profile: TenantRuntimeProfile,
    /// Compatibility switch validated against `profile`.
    /// `single_tenant` requires false; all request-derived profiles require true.
    #[serde(default = "default_true")]
    pub enabled: bool,
''',
    "insert tenant runtime profile",
)

old_policy = '''#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TenantSettingsError {
    InvalidHeaderName(String),
    MissingSubdomainBaseDomain,
    FallbackRequiresHeaderMode,
    FallbackForbiddenInProduction,
}

impl std::fmt::Display for TenantSettingsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHeaderName(value) => {
                write!(formatter, "rustok.tenant.header_name `{value}` is not a valid HTTP header name")
            }
            Self::MissingSubdomainBaseDomain => formatter.write_str(
                "rustok.tenant.base_domains must contain at least one domain when resolution=subdomain",
            ),
            Self::FallbackRequiresHeaderMode => formatter.write_str(
                "rustok.tenant.fallback_mode=default_tenant is only valid with resolution=header",
            ),
            Self::FallbackForbiddenInProduction => formatter.write_str(
                "rustok.tenant.fallback_mode=default_tenant is forbidden in production",
            ),
        }
    }
}

impl std::error::Error for TenantSettingsError {}

impl TenantSettings {
    pub fn validate(&self) -> Result<(), TenantSettingsError> {
        if !self.enabled {
            return Ok(());
        }

        axum::http::HeaderName::from_bytes(self.header_name.as_bytes())
            .map_err(|_| TenantSettingsError::InvalidHeaderName(self.header_name.clone()))?;

        if self.resolution == TenantResolutionMode::Subdomain && self.base_domains.is_empty() {
            return Err(TenantSettingsError::MissingSubdomainBaseDomain);
        }

        if self.fallback_mode == TenantFallbackMode::DefaultTenant
            && self.resolution != TenantResolutionMode::Header
        {
            return Err(TenantSettingsError::FallbackRequiresHeaderMode);
        }

        Ok(())
    }

    pub fn validate_for_environment(&self, production: bool) -> Result<(), TenantSettingsError> {
        self.validate()?;
        if self.enabled && production && self.fallback_mode == TenantFallbackMode::DefaultTenant {
            return Err(TenantSettingsError::FallbackForbiddenInProduction);
        }
        Ok(())
    }
}
'''
new_policy = '''#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TenantSettingsError {
    ProfileEnabledMismatch {
        profile: TenantRuntimeProfile,
        enabled: bool,
    },
    InvalidHeaderName(String),
    MissingSubdomainBaseDomain,
    FallbackRequiresDevelopmentProfile,
    FallbackRequiresHeaderMode,
    DevelopmentProfileForbiddenInProduction,
}

impl std::fmt::Display for TenantSettingsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProfileEnabledMismatch { profile, enabled } => write!(
                formatter,
                "rustok.tenant.profile={profile} is inconsistent with rustok.tenant.enabled={enabled}; single_tenant requires enabled=false and request-derived profiles require enabled=true"
            ),
            Self::InvalidHeaderName(value) => {
                write!(formatter, "rustok.tenant.header_name `{value}` is not a valid HTTP header name")
            }
            Self::MissingSubdomainBaseDomain => formatter.write_str(
                "rustok.tenant.base_domains must contain at least one domain when resolution=subdomain",
            ),
            Self::FallbackRequiresDevelopmentProfile => formatter.write_str(
                "rustok.tenant.fallback_mode=default_tenant requires rustok.tenant.profile=development",
            ),
            Self::FallbackRequiresHeaderMode => formatter.write_str(
                "rustok.tenant.fallback_mode=default_tenant is only valid with resolution=header",
            ),
            Self::DevelopmentProfileForbiddenInProduction => formatter.write_str(
                "rustok.tenant.profile=development is forbidden in production; use multi_tenant or single_tenant",
            ),
        }
    }
}

impl std::error::Error for TenantSettingsError {}

impl TenantSettings {
    pub fn validate(&self) -> Result<(), TenantSettingsError> {
        let expected_enabled = self.profile.derives_tenant_from_request();
        if self.enabled != expected_enabled {
            return Err(TenantSettingsError::ProfileEnabledMismatch {
                profile: self.profile,
                enabled: self.enabled,
            });
        }

        if self.fallback_mode == TenantFallbackMode::DefaultTenant
            && self.profile != TenantRuntimeProfile::Development
        {
            return Err(TenantSettingsError::FallbackRequiresDevelopmentProfile);
        }

        if self.profile == TenantRuntimeProfile::SingleTenant {
            return Ok(());
        }

        axum::http::HeaderName::from_bytes(self.header_name.as_bytes())
            .map_err(|_| TenantSettingsError::InvalidHeaderName(self.header_name.clone()))?;

        if self.resolution == TenantResolutionMode::Subdomain && self.base_domains.is_empty() {
            return Err(TenantSettingsError::MissingSubdomainBaseDomain);
        }

        if self.fallback_mode == TenantFallbackMode::DefaultTenant
            && self.resolution != TenantResolutionMode::Header
        {
            return Err(TenantSettingsError::FallbackRequiresHeaderMode);
        }

        Ok(())
    }

    pub fn validate_for_environment(&self, production: bool) -> Result<(), TenantSettingsError> {
        self.validate()?;
        if production && self.profile == TenantRuntimeProfile::Development {
            return Err(TenantSettingsError::DevelopmentProfileForbiddenInProduction);
        }
        Ok(())
    }
}
'''
replace_once(settings, old_policy, new_policy, "replace tenant policy")
replace_once(
    settings,
    '''        Self {
            enabled: true,
            resolution: TenantResolutionMode::Header,
''',
    '''        Self {
            profile: TenantRuntimeProfile::MultiTenant,
            enabled: true,
            resolution: TenantResolutionMode::Header,
''',
    "default tenant profile",
)
replace_once(
    settings,
    '''        RateLimitBackendKind, RelayTargetKind, RustokSettings, TenantFallbackMode,
        TenantResolutionMode, TenantSettingsError,
''',
    '''        RateLimitBackendKind, RelayTargetKind, RustokSettings, TenantFallbackMode,
        TenantResolutionMode, TenantRuntimeProfile, TenantSettingsError,
''',
    "test imports",
)
replace_once(
    settings,
    '''    #[test]
    fn tenant_policy_rejects_invalid_fallback_combination() {
        let mut settings = RustokSettings::default();
        settings.tenant.resolution = TenantResolutionMode::Host;
        settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;
        assert_eq!(
            settings.tenant.validate(),
            Err(TenantSettingsError::FallbackRequiresHeaderMode)
        );
    }

    #[test]
    fn tenant_policy_rejects_development_fallback_in_production() {
        let mut settings = RustokSettings::default();
        settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;
        assert_eq!(
            settings.tenant.validate_for_environment(true),
            Err(TenantSettingsError::FallbackForbiddenInProduction)
        );
    }
''',
    '''    #[test]
    fn tenant_policy_requires_profile_and_enabled_to_agree() {
        let mut settings = RustokSettings::default();
        settings.tenant.profile = TenantRuntimeProfile::SingleTenant;
        assert_eq!(
            settings.tenant.validate(),
            Err(TenantSettingsError::ProfileEnabledMismatch {
                profile: TenantRuntimeProfile::SingleTenant,
                enabled: true,
            })
        );
        settings.tenant.enabled = false;
        assert!(settings.tenant.validate().is_ok());
    }

    #[test]
    fn tenant_policy_rejects_fallback_outside_development_profile() {
        let mut settings = RustokSettings::default();
        settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;
        assert_eq!(
            settings.tenant.validate(),
            Err(TenantSettingsError::FallbackRequiresDevelopmentProfile)
        );
    }

    #[test]
    fn tenant_policy_rejects_invalid_development_fallback_combination() {
        let mut settings = RustokSettings::default();
        settings.tenant.profile = TenantRuntimeProfile::Development;
        settings.tenant.resolution = TenantResolutionMode::Host;
        settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;
        assert_eq!(
            settings.tenant.validate(),
            Err(TenantSettingsError::FallbackRequiresHeaderMode)
        );
    }

    #[test]
    fn tenant_policy_rejects_development_profile_in_production() {
        let mut settings = RustokSettings::default();
        settings.tenant.profile = TenantRuntimeProfile::Development;
        assert_eq!(
            settings.tenant.validate_for_environment(true),
            Err(TenantSettingsError::DevelopmentProfileForbiddenInProduction)
        );
    }
''',
    "tenant profile tests",
)

resolution = Path("apps/server/src/middleware/tenant_resolution.rs")
replace_once(
    resolution,
    '''    settings::{RustokSettings, TenantFallbackMode, TenantResolutionMode},
''',
    '''    settings::{
        RustokSettings, TenantFallbackMode, TenantResolutionMode, TenantRuntimeProfile,
    },
''',
    "resolution profile import",
)
replace_once(
    resolution,
    '''    if !settings.tenant.enabled {
''',
    '''    if settings.tenant.profile == TenantRuntimeProfile::SingleTenant {
''',
    "single tenant profile resolution",
)
replace_once(
    resolution,
    '''        let mut settings = RustokSettings::default();
        settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;
''',
    '''        let mut settings = RustokSettings::default();
        settings.tenant.profile = TenantRuntimeProfile::Development;
        settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;
''',
    "development fallback test profile",
)
replace_once(
    resolution,
    '''    #[test]
    fn strict_header_mode_rejects_missing_header() {
''',
    '''    #[test]
    fn explicit_single_tenant_profile_resolves_default_without_request_assertion() {
        let mut settings = RustokSettings::default();
        settings.tenant.profile = TenantRuntimeProfile::SingleTenant;
        settings.tenant.enabled = false;
        let resolution = resolve_request(&request("/api/users"), &settings).expect("single tenant");
        assert_eq!(resolution.source, TenantResolutionSource::SingleTenantDefault);
        assert_eq!(
            resolution.identifier,
            ResolvedTenantIdentifier::Uuid(settings.tenant.default_id)
        );
    }

    #[test]
    fn strict_header_mode_rejects_missing_header() {
''',
    "single tenant resolver test",
)

for relative, profile in [
    ("apps/server/config/development.yaml", "development"),
    ("apps/server/config/test.yaml", "development"),
    ("apps/server/config/production.redis.example.yaml", "multi_tenant"),
]:
    path = Path(relative)
    replace_once(
        path,
        '''    tenant:\n      enabled: true\n''',
        f'''    tenant:\n      profile: {profile}\n      enabled: true\n''',
        "explicit tenant profile",
    )


decision = Path("DECISIONS/2026-04-03-request-trust-and-tenant-hardening.md")
replace_once(
    decision,
    '''- Add explicit tenant fallback policy with `settings.rustok.tenant.fallback_mode = "disabled" | "default_tenant"`.
- Keep the default production posture as strict:
''',
    '''- Add an explicit tenant runtime profile with `settings.rustok.tenant.profile = "multi_tenant" | "single_tenant" | "development"`.
- Retain `settings.rustok.tenant.enabled` as a compatibility switch, but validate it against the profile: `single_tenant` requires `false`; request-derived profiles require `true`.
- Permit `settings.rustok.tenant.fallback_mode = "default_tenant"` only in the `development` profile and only with header resolution.
- Reject the entire `development` profile in production, even when fallback is disabled.
- Keep the default production posture as strict:
''',
    "profile decision",
)
replace_once(
    decision,
    '''- Dev/test environments can still opt into `default_tenant` fallback when convenient, but this must now be explicit in configuration.
''',
    '''- Dev/test environments can opt into the explicit `development` profile and then enable `default_tenant` fallback when needed.
- Production deployments must declare either `multi_tenant` or `single_tenant`; implicit single-tenant behavior is rejected.
''',
    "profile consequences",
)
