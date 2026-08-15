use std::{fmt, time::Duration};

use crate::error::{Error, Result};
use crate::services::server_runtime_context::ServerRuntimeContext;

const PROVIDER_ENV: &str = "RUSTOK_PRODUCT_CATALOG_PROVIDER";
const GRPC_ENDPOINT_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT";
const GRPC_BEARER_TOKEN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN";
const TLS_DOMAIN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_TLS_DOMAIN";
const CONNECT_TIMEOUT_MS_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_CONNECT_TIMEOUT_MS";
const ALLOW_INSECURE_LOOPBACK_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK";
const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 5_000;

#[derive(Debug, Clone, Eq, PartialEq)]
enum ProductCatalogDeployment {
    Embedded,
    Grpc(ProductCatalogGrpcDeployment),
}

#[derive(Clone, Eq, PartialEq)]
struct ProductCatalogBearerSecret(String);

impl ProductCatalogBearerSecret {
    fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for ProductCatalogBearerSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ProductCatalogGrpcDeployment {
    endpoint: String,
    bearer_token: ProductCatalogBearerSecret,
    tls_domain: Option<String>,
    connect_timeout_ms: u64,
    allow_insecure_loopback: bool,
}

/// Selects and connects the deployment-owned Product catalog provider before host composition.
///
/// The default is the embedded owner service. When `grpc` is selected, invalid configuration,
/// authentication, or connection failure aborts startup; the server never silently falls back to
/// embedded execution.
pub async fn configure_product_catalog_deployment(ctx: &ServerRuntimeContext) -> Result<()> {
    #[cfg(feature = "mod-product")]
    {
        use std::sync::Arc;

        use rustok_product::ProductCatalogReadRuntime;
        use rustok_product_transport::{
            GrpcProductCatalogReadConnectionConfig, ProductCatalogGrpcBearerToken,
        };

        let deployment = deployment_from_environment().map_err(Error::Message)?;
        let ProductCatalogDeployment::Grpc(remote) = deployment else {
            return Ok(());
        };

        let authentication = ProductCatalogGrpcBearerToken::new(remote.bearer_token.expose())
            .map_err(|error| {
                Error::Message(format!(
                    "remote Product catalog authentication configuration failed: {error}"
                ))
            })?;
        let provider = GrpcProductCatalogReadConnectionConfig::new(remote.endpoint)
            .with_tls_domain(remote.tls_domain)
            .with_connect_timeout(Duration::from_millis(remote.connect_timeout_ms))
            .allow_insecure_loopback(remote.allow_insecure_loopback)
            .connect()
            .await
            .map_err(|error| {
                Error::Message(format!(
                    "remote Product catalog provider initialization failed: {error}"
                ))
            })?
            .with_authentication(authentication);
        ctx.shared_insert(ProductCatalogReadRuntime::external(Arc::new(provider)));

        tracing::info!(
            provider = "grpc",
            authentication = "bearer",
            insecure_loopback = remote.allow_insecure_loopback,
            "Product catalog deployment provider initialized"
        );
        Ok(())
    }

    #[cfg(not(feature = "mod-product"))]
    {
        let _ = ctx;
        Ok(())
    }
}

fn deployment_from_environment() -> std::result::Result<ProductCatalogDeployment, String> {
    parse_deployment(
        optional_env(PROVIDER_ENV).as_deref(),
        optional_env(GRPC_ENDPOINT_ENV).as_deref(),
        optional_secret_env(GRPC_BEARER_TOKEN_ENV).as_deref(),
        optional_env(TLS_DOMAIN_ENV).as_deref(),
        optional_env(CONNECT_TIMEOUT_MS_ENV).as_deref(),
        optional_env(ALLOW_INSECURE_LOOPBACK_ENV).as_deref(),
    )
}

fn parse_deployment(
    provider: Option<&str>,
    endpoint: Option<&str>,
    bearer_token: Option<&str>,
    tls_domain: Option<&str>,
    connect_timeout_ms: Option<&str>,
    allow_insecure_loopback: Option<&str>,
) -> std::result::Result<ProductCatalogDeployment, String> {
    let provider = provider
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("embedded")
        .to_ascii_lowercase();
    let endpoint = normalize_optional(endpoint);
    let bearer_token = bearer_token
        .filter(|value| !value.is_empty())
        .map(|value| ProductCatalogBearerSecret::new(value.to_string()));
    let tls_domain = normalize_optional(tls_domain);
    let timeout = parse_timeout(connect_timeout_ms)?;
    let allow_insecure_loopback_is_configured = allow_insecure_loopback
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let allow_insecure_loopback =
        parse_bool(allow_insecure_loopback, ALLOW_INSECURE_LOOPBACK_ENV, false)?;

    match provider.as_str() {
        "embedded" => {
            if endpoint.is_some()
                || bearer_token.is_some()
                || tls_domain.is_some()
                || connect_timeout_ms.is_some()
                || allow_insecure_loopback_is_configured
            {
                return Err(format!(
                    "remote Product catalog variables require {PROVIDER_ENV}=grpc"
                ));
            }
            Ok(ProductCatalogDeployment::Embedded)
        }
        "grpc" => {
            let endpoint = endpoint.ok_or_else(|| {
                format!("{GRPC_ENDPOINT_ENV} is required when {PROVIDER_ENV}=grpc")
            })?;
            let bearer_token = bearer_token.ok_or_else(|| {
                format!("{GRPC_BEARER_TOKEN_ENV} is required when {PROVIDER_ENV}=grpc")
            })?;
            Ok(ProductCatalogDeployment::Grpc(
                ProductCatalogGrpcDeployment {
                    endpoint,
                    bearer_token,
                    tls_domain,
                    connect_timeout_ms: timeout,
                    allow_insecure_loopback,
                },
            ))
        }
        _ => Err(format!("{PROVIDER_ENV} must be either embedded or grpc")),
    }
}

fn parse_timeout(value: Option<&str>) -> std::result::Result<u64, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_CONNECT_TIMEOUT_MS);
    };
    value
        .parse::<u64>()
        .map_err(|_| format!("{CONNECT_TIMEOUT_MS_ENV} must be an integer number of milliseconds"))
}

fn parse_bool(value: Option<&str>, name: &str, default: bool) -> std::result::Result<bool, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(default);
    };
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(format!("{name} must be a boolean")),
    }
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn optional_secret_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{
        ProductCatalogBearerSecret, ProductCatalogDeployment, ProductCatalogGrpcDeployment,
        parse_deployment,
    };

    #[test]
    fn embedded_is_the_default() {
        assert_eq!(
            parse_deployment(None, None, None, None, None, None).unwrap(),
            ProductCatalogDeployment::Embedded
        );
    }

    #[test]
    fn grpc_requires_an_endpoint() {
        let error = parse_deployment(Some("grpc"), None, Some("catalog-secret"), None, None, None)
            .expect_err("remote Product deployment without endpoint must fail closed");
        assert!(error.contains("RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT"));
    }

    #[test]
    fn grpc_requires_a_bearer_token() {
        let error = parse_deployment(
            Some("grpc"),
            Some("https://product-catalog.internal"),
            None,
            None,
            None,
            None,
        )
        .expect_err("remote Product deployment without credential must fail closed");
        assert!(error.contains("RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN"));
    }

    #[test]
    fn grpc_configuration_preserves_transport_inputs() {
        assert_eq!(
            parse_deployment(
                Some("grpc"),
                Some("https://product-catalog.internal:7443"),
                Some("catalog-secret"),
                Some("product-catalog.internal"),
                Some("2500"),
                Some("false"),
            )
            .unwrap(),
            ProductCatalogDeployment::Grpc(ProductCatalogGrpcDeployment {
                endpoint: "https://product-catalog.internal:7443".to_string(),
                bearer_token: ProductCatalogBearerSecret::new("catalog-secret"),
                tls_domain: Some("product-catalog.internal".to_string()),
                connect_timeout_ms: 2500,
                allow_insecure_loopback: false,
            })
        );
    }

    #[test]
    fn bearer_secret_debug_is_redacted() {
        let deployment = parse_deployment(
            Some("grpc"),
            Some("https://product-catalog.internal:7443"),
            Some("catalog-secret"),
            None,
            None,
            None,
        )
        .unwrap();
        let debug = format!("{deployment:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("catalog-secret"));
    }

    #[test]
    fn remote_variables_are_not_silently_ignored_in_embedded_mode() {
        let error = parse_deployment(
            Some("embedded"),
            Some("https://product-catalog.internal"),
            None,
            None,
            None,
            None,
        )
        .expect_err("remote Product variables in embedded mode must be rejected");
        assert!(error.contains("require RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc"));
    }

    #[test]
    fn bearer_token_is_not_silently_ignored_in_embedded_mode() {
        let error = parse_deployment(
            Some("embedded"),
            None,
            Some("catalog-secret"),
            None,
            None,
            None,
        )
        .expect_err("remote Product credential in embedded mode must be rejected");
        assert!(error.contains("require RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc"));
    }

    #[test]
    fn explicit_loopback_flag_is_not_silently_ignored_in_embedded_mode() {
        let error = parse_deployment(Some("embedded"), None, None, None, None, Some("false"))
            .expect_err("explicit remote loopback configuration must require grpc mode");
        assert!(error.contains("require RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc"));
    }

    #[test]
    fn invalid_boolean_is_rejected() {
        let error = parse_deployment(
            Some("grpc"),
            Some("https://product-catalog.internal"),
            Some("catalog-secret"),
            None,
            None,
            Some("maybe"),
        )
        .expect_err("invalid Product loopback flag must fail closed");
        assert!(error.contains("must be a boolean"));
    }
}
