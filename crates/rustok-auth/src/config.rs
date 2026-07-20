use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::{AuthError, Result};

const DEFAULT_ACCESS_EXPIRATION_SECS: u64 = 900; // 15 minutes
const DEFAULT_REFRESH_EXPIRATION_SECS: u64 = 60 * 60 * 24 * 30; // 30 days
const MIN_HS256_SECRET_BYTES: usize = 32;
const MIN_ACCESS_EXPIRATION_SECS: u64 = 60;
const MAX_ACCESS_EXPIRATION_SECS: u64 = 60 * 60 * 24 * 30;
const MIN_REFRESH_EXPIRATION_SECS: u64 = 60 * 5;
const MAX_REFRESH_EXPIRATION_SECS: u64 = 60 * 60 * 24 * 365;

#[derive(Debug, Deserialize, Serialize)]
struct Rs256KeyPairProbe {
    probe: String,
}

/// JWT signing algorithm selector.
///
/// - `HS256` (default): HMAC-SHA256, symmetric shared secret via `AuthConfig::secret`.
/// - `RS256`: RSA-SHA256, asymmetric — set `rsa_private_key_pem` for signing,
///   `rsa_public_key_pem` for verification.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum JwtAlgorithm {
    #[default]
    HS256,
    RS256,
}

/// Auth configuration — framework-agnostic.
///
/// The server is responsible for constructing this from whatever config source
/// it uses (YAML, environment variables, etc.). `rustok-auth` never reads config files.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub secret: String,
    pub access_expiration: u64,
    pub refresh_expiration: u64,
    pub issuer: String,
    pub audience: String,
    /// JWT signing algorithm. Defaults to `HS256`.
    pub algorithm: JwtAlgorithm,
    /// RSA private key in PEM format. Required when `algorithm = RS256` for token encoding.
    pub rsa_private_key_pem: Option<String>,
    /// RSA public key in PEM format. Required when `algorithm = RS256` for token decoding.
    pub rsa_public_key_pem: Option<String>,
}

impl AuthConfig {
    pub fn new(secret: String) -> Self {
        Self {
            secret,
            access_expiration: DEFAULT_ACCESS_EXPIRATION_SECS,
            refresh_expiration: DEFAULT_REFRESH_EXPIRATION_SECS,
            issuer: "rustok".to_string(),
            audience: "rustok-admin".to_string(),
            algorithm: JwtAlgorithm::HS256,
            rsa_private_key_pem: None,
            rsa_public_key_pem: None,
        }
    }

    pub fn with_expiration(mut self, access: u64, refresh: u64) -> Self {
        self.access_expiration = access;
        self.refresh_expiration = refresh;
        self
    }

    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = issuer.into();
        self
    }

    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.audience = audience.into();
        self
    }

    pub fn with_rs256(
        mut self,
        private_key_pem: impl Into<String>,
        public_key_pem: impl Into<String>,
    ) -> Self {
        self.algorithm = JwtAlgorithm::RS256;
        self.rsa_private_key_pem = Some(private_key_pem.into());
        self.rsa_public_key_pem = Some(public_key_pem.into());
        self
    }
}

/// Helper for loading auth settings from nested YAML `settings.rustok.auth`.
#[derive(Debug, Deserialize, Default)]
pub struct AuthSettingsOverrides {
    pub refresh_expiration: Option<u64>,
    pub issuer: Option<String>,
    pub audience: Option<String>,
    pub algorithm: Option<JwtAlgorithm>,
    pub rsa_private_key_pem: Option<String>,
    pub rsa_public_key_pem: Option<String>,
    pub rsa_private_key_env: Option<String>,
    pub rsa_public_key_env: Option<String>,
}

impl AuthSettingsOverrides {
    /// Apply overrides on top of a base `AuthConfig`.
    pub fn apply(self, config: &mut AuthConfig) {
        if let Some(v) = self.refresh_expiration {
            config.refresh_expiration = v;
        }
        if let Some(v) = self.issuer {
            config.issuer = v;
        }
        if let Some(v) = self.audience {
            config.audience = v;
        }
        if let Some(v) = self.algorithm {
            config.algorithm = v;
        }
        if let Some(v) = self.rsa_private_key_pem {
            config.rsa_private_key_pem = Some(v);
        }
        if let Some(v) = self.rsa_public_key_pem {
            config.rsa_public_key_pem = Some(v);
        }
    }
}

/// Build and validate `AuthConfig` from host-provided settings.
///
/// The host remains responsible for reading its framework config, but the
/// semantics of auth overrides, RS256 key resolution, defaults, and validation
/// live in `rustok-auth` so transport adapters do not duplicate auth lifecycle
/// rules.
pub fn build_auth_config(
    secret: String,
    access_expiration: u64,
    auth_settings: AuthSettingsOverrides,
) -> Result<AuthConfig> {
    build_auth_config_with_env(secret, access_expiration, auth_settings, |name| {
        std::env::var(name)
            .map_err(|_| AuthError::Internal(format!("Missing auth env var: {name}")))
    })
}

/// Build and validate `AuthConfig` with an injectable env resolver for tests.
pub fn build_auth_config_with_env<F>(
    secret: String,
    access_expiration: u64,
    auth_settings: AuthSettingsOverrides,
    mut env_resolver: F,
) -> Result<AuthConfig>
where
    F: FnMut(&str) -> Result<String>,
{
    let refresh_expiration = auth_settings
        .refresh_expiration
        .unwrap_or(DEFAULT_REFRESH_EXPIRATION_SECS);

    let mut config = AuthConfig::new(secret).with_expiration(access_expiration, refresh_expiration);

    if let Some(issuer) = auth_settings.issuer {
        config = config.with_issuer(issuer);
    }
    if let Some(audience) = auth_settings.audience {
        config = config.with_audience(audience);
    }
    if let Some(algorithm) = auth_settings.algorithm {
        config.algorithm = algorithm;
    }

    config.rsa_private_key_pem = resolve_key_material(
        auth_settings.rsa_private_key_pem,
        auth_settings.rsa_private_key_env,
        &mut env_resolver,
    )?;
    config.rsa_public_key_pem = resolve_key_material(
        auth_settings.rsa_public_key_pem,
        auth_settings.rsa_public_key_env,
        &mut env_resolver,
    )?;

    validate_auth_config(&config)?;
    Ok(config)
}

fn resolve_key_material<F>(
    inline_pem: Option<String>,
    env_name: Option<String>,
    env_resolver: &mut F,
) -> Result<Option<String>>
where
    F: FnMut(&str) -> Result<String>,
{
    if let Some(env_name) = env_name.filter(|name| !name.trim().is_empty()) {
        let value = env_resolver(&env_name)?;
        if value.trim().is_empty() {
            return Err(AuthError::Internal(format!(
                "Auth env var {env_name} must not be empty"
            )));
        }
        return Ok(Some(value));
    }

    Ok(inline_pem.filter(|pem| !pem.trim().is_empty()))
}

/// Validate auth config invariants owned by `rustok-auth`.
pub fn validate_auth_config(config: &AuthConfig) -> Result<()> {
    if config.issuer.trim().is_empty() {
        return Err(AuthError::Internal(
            "JWT issuer must not be empty".to_string(),
        ));
    }
    if config.audience.trim().is_empty() {
        return Err(AuthError::Internal(
            "JWT audience must not be empty".to_string(),
        ));
    }
    if !(MIN_ACCESS_EXPIRATION_SECS..=MAX_ACCESS_EXPIRATION_SECS)
        .contains(&config.access_expiration)
    {
        return Err(AuthError::Internal(format!(
            "JWT access_expiration must be between {MIN_ACCESS_EXPIRATION_SECS} and {MAX_ACCESS_EXPIRATION_SECS} seconds"
        )));
    }
    if !(MIN_REFRESH_EXPIRATION_SECS..=MAX_REFRESH_EXPIRATION_SECS)
        .contains(&config.refresh_expiration)
    {
        return Err(AuthError::Internal(format!(
            "JWT refresh_expiration must be between {MIN_REFRESH_EXPIRATION_SECS} and {MAX_REFRESH_EXPIRATION_SECS} seconds"
        )));
    }
    if config.refresh_expiration < config.access_expiration {
        return Err(AuthError::Internal(
            "JWT refresh_expiration must be greater than or equal to access_expiration".to_string(),
        ));
    }

    match config.algorithm {
        JwtAlgorithm::HS256 => {
            if config.secret.len() < MIN_HS256_SECRET_BYTES {
                return Err(AuthError::Internal(format!(
                    "HS256 secret must contain at least {MIN_HS256_SECRET_BYTES} bytes"
                )));
            }
            if config.rsa_private_key_pem.is_some() || config.rsa_public_key_pem.is_some() {
                return Err(AuthError::Internal(
                    "HS256 configuration must not include RSA key material".to_string(),
                ));
            }
        }
        JwtAlgorithm::RS256 => {
            let private_key = config
                .rsa_private_key_pem
                .as_deref()
                .filter(|key| !key.trim().is_empty());
            let public_key = config
                .rsa_public_key_pem
                .as_deref()
                .filter(|key| !key.trim().is_empty());
            if private_key.is_none() || public_key.is_none() {
                return Err(AuthError::Internal(
                    "RS256 requires both non-empty rsa_private_key_pem and rsa_public_key_pem"
                        .to_string(),
                ));
            }
            validate_rs256_key_pair(private_key.unwrap(), public_key.unwrap())?;
        }
    }

    Ok(())
}

fn validate_rs256_key_pair(private_key_pem: &str, public_key_pem: &str) -> Result<()> {
    let encoding_key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
        .map_err(|_| AuthError::Internal("Invalid RS256 private key PEM".to_string()))?;
    let decoding_key = DecodingKey::from_rsa_pem(public_key_pem.as_bytes())
        .map_err(|_| AuthError::Internal("Invalid RS256 public key PEM".to_string()))?;
    let token = encode(
        &Header::new(Algorithm::RS256),
        &Rs256KeyPairProbe {
            probe: "rustok-auth-config".to_string(),
        },
        &encoding_key,
    )
    .map_err(|_| AuthError::Internal("RS256 private key cannot sign tokens".to_string()))?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.required_spec_claims.clear();
    validation.validate_exp = false;
    decode::<Rs256KeyPairProbe>(&token, &decoding_key, &validation).map_err(|_| {
        AuthError::Internal("RS256 private/public keys do not form a pair".to_string())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret() -> String {
        "test-secret-key-for-unit-tests-only-32bytes!".to_string()
    }

    #[test]
    fn build_auth_config_defaults_to_hs256() {
        let config =
            build_auth_config_with_env(secret(), 900, AuthSettingsOverrides::default(), |_| {
                panic!("env resolver should not be called")
            })
            .expect("auth config");

        assert_eq!(config.algorithm, JwtAlgorithm::HS256);
        assert_eq!(config.access_expiration, 900);
        assert_eq!(config.refresh_expiration, DEFAULT_REFRESH_EXPIRATION_SECS);
    }

    #[test]
    fn build_auth_config_resolves_rs256_keys_from_env() {
        let config = build_auth_config_with_env(
            secret(),
            900,
            AuthSettingsOverrides {
                algorithm: Some(JwtAlgorithm::RS256),
                rsa_private_key_env: Some("PRIVATE".to_string()),
                rsa_public_key_env: Some("PUBLIC".to_string()),
                ..AuthSettingsOverrides::default()
            },
            |name| match name {
                "PRIVATE" => Ok(crate::jwt::TEST_RSA_PRIVATE_KEY.to_string()),
                "PUBLIC" => Ok(crate::jwt::TEST_RSA_PUBLIC_KEY.to_string()),
                _ => panic!("unexpected env key"),
            },
        )
        .expect("auth config");

        assert_eq!(
            config.rsa_private_key_pem.as_deref(),
            Some(crate::jwt::TEST_RSA_PRIVATE_KEY)
        );
        assert_eq!(
            config.rsa_public_key_pem.as_deref(),
            Some(crate::jwt::TEST_RSA_PUBLIC_KEY)
        );
    }

    #[test]
    fn build_auth_config_rejects_rs256_without_keys() {
        let result = build_auth_config_with_env(
            secret(),
            900,
            AuthSettingsOverrides {
                algorithm: Some(JwtAlgorithm::RS256),
                ..AuthSettingsOverrides::default()
            },
            |_| panic!("env resolver should not be called"),
        );

        assert!(result.is_err());
    }

    #[test]
    fn build_auth_config_rejects_malformed_rs256_keys() {
        let result = build_auth_config_with_env(
            secret(),
            900,
            AuthSettingsOverrides {
                algorithm: Some(JwtAlgorithm::RS256),
                rsa_private_key_pem: Some("not-a-private-key".to_string()),
                rsa_public_key_pem: Some("not-a-public-key".to_string()),
                ..AuthSettingsOverrides::default()
            },
            |_| panic!("env resolver should not be called"),
        );

        assert!(result.is_err());
    }

    #[test]
    fn build_auth_config_rejects_mismatched_rs256_key_pair() {
        let different_public_key = crate::jwt::TEST_RSA_PUBLIC_KEY.replacen("3bmu", "3cmu", 1);
        let result = build_auth_config_with_env(
            secret(),
            900,
            AuthSettingsOverrides {
                algorithm: Some(JwtAlgorithm::RS256),
                rsa_private_key_pem: Some(crate::jwt::TEST_RSA_PRIVATE_KEY.to_string()),
                rsa_public_key_pem: Some(different_public_key),
                ..AuthSettingsOverrides::default()
            },
            |_| panic!("env resolver should not be called"),
        );

        assert!(result.is_err());
    }

    #[test]
    fn build_auth_config_rejects_weak_hs256_secret() {
        let result = build_auth_config_with_env(
            "short-secret".to_string(),
            900,
            AuthSettingsOverrides::default(),
            |_| panic!("env resolver should not be called"),
        );

        assert!(result.is_err());
    }

    #[test]
    fn build_auth_config_rejects_blank_issuer_and_audience() {
        for overrides in [
            AuthSettingsOverrides {
                issuer: Some("  ".to_string()),
                ..AuthSettingsOverrides::default()
            },
            AuthSettingsOverrides {
                audience: Some(String::new()),
                ..AuthSettingsOverrides::default()
            },
        ] {
            assert!(build_auth_config_with_env(secret(), 900, overrides, |_| {
                panic!("env resolver should not be called")
            })
            .is_err());
        }
    }

    #[test]
    fn build_auth_config_rejects_invalid_ttl_bounds() {
        assert!(build_auth_config_with_env(
            secret(),
            MIN_ACCESS_EXPIRATION_SECS - 1,
            AuthSettingsOverrides::default(),
            |_| panic!("env resolver should not be called"),
        )
        .is_err());

        assert!(build_auth_config_with_env(
            secret(),
            900,
            AuthSettingsOverrides {
                refresh_expiration: Some(300),
                ..AuthSettingsOverrides::default()
            },
            |_| panic!("env resolver should not be called"),
        )
        .is_err());
    }

    #[test]
    fn build_auth_config_rejects_rsa_material_with_hs256() {
        let result = build_auth_config_with_env(
            secret(),
            900,
            AuthSettingsOverrides {
                rsa_private_key_pem: Some("private".to_string()),
                rsa_public_key_pem: Some("public".to_string()),
                ..AuthSettingsOverrides::default()
            },
            |_| panic!("env resolver should not be called"),
        );

        assert!(result.is_err());
    }
}
