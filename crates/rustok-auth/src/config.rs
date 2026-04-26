use serde::{Deserialize, Serialize};

const DEFAULT_ACCESS_EXPIRATION_SECS: u64 = 900; // 15 minutes
const DEFAULT_REFRESH_EXPIRATION_SECS: u64 = 60 * 60 * 24 * 30; // 30 days

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
/// it uses (Loco YAML, env vars, etc.). `rustok-auth` never reads config files.
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
