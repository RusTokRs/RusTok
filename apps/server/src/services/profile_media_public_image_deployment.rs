use std::time::Duration;

use crate::error::{Error, Result};
use crate::services::server_runtime_context::ServerRuntimeContext;

const PROVIDER_ENV: &str = "RUSTOK_PROFILE_MEDIA_PROVIDER";
const GRPC_ENDPOINT_ENV: &str = "RUSTOK_PROFILE_MEDIA_GRPC_ENDPOINT";
const PUBLIC_ORIGIN_ENV: &str = "RUSTOK_PROFILE_MEDIA_PUBLIC_ORIGIN";
const TLS_DOMAIN_ENV: &str = "RUSTOK_PROFILE_MEDIA_GRPC_TLS_DOMAIN";
const CONNECT_TIMEOUT_MS_ENV: &str = "RUSTOK_PROFILE_MEDIA_GRPC_CONNECT_TIMEOUT_MS";
const ALLOW_INSECURE_LOOPBACK_ENV: &str = "RUSTOK_PROFILE_MEDIA_GRPC_ALLOW_INSECURE_LOOPBACK";
const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 5_000;

#[derive(Debug, Clone, Eq, PartialEq)]
enum ProfileMediaPublicImageDeployment {
    Embedded,
    Grpc(ProfileMediaPublicImageGrpcDeployment),
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ProfileMediaPublicImageGrpcDeployment {
    endpoint: String,
    public_origin: Option<String>,
    tls_domain: Option<String>,
    connect_timeout_ms: u64,
    allow_insecure_loopback: bool,
}

pub async fn configure_profile_media_public_image_deployment(
    ctx: &ServerRuntimeContext,
) -> Result<()> {
    #[cfg(all(feature = "mod-profiles", feature = "mod-media"))]
    {
        use std::sync::Arc;

        use rustok_media::MediaPublicImageReadPort;
        use rustok_media_transport::GrpcMediaPublicImageConnectionConfig;
        use rustok_profiles::ProfileMediaPublicImageProvider;

        let deployment = deployment_from_environment().map_err(Error::Message)?;
        let ProfileMediaPublicImageDeployment::Grpc(remote) = deployment else {
            return Ok(());
        };

        let connection = GrpcMediaPublicImageConnectionConfig::new(remote.endpoint)
            .with_public_origin(remote.public_origin.clone())
            .with_tls_domain(remote.tls_domain)
            .with_connect_timeout(Duration::from_millis(remote.connect_timeout_ms))
            .allow_insecure_loopback(remote.allow_insecure_loopback);
        let provider = connection.connect().await.map_err(|error| {
            Error::Message(format!(
                "remote profile Media public-image provider initialization failed: {error}"
            ))
        })?;
        let provider: Arc<dyn MediaPublicImageReadPort> = Arc::new(provider);
        ctx.shared_insert(ProfileMediaPublicImageProvider::new(provider));

        tracing::info!(
            provider = "grpc",
            public_origin_configured = remote.public_origin.is_some(),
            insecure_loopback = remote.allow_insecure_loopback,
            "profile Media public-image deployment provider initialized"
        );
        Ok(())
    }

    #[cfg(not(all(feature = "mod-profiles", feature = "mod-media")))]
    {
        let _ = ctx;
        Ok(())
    }
}

fn deployment_from_environment() -> std::result::Result<ProfileMediaPublicImageDeployment, String> {
    parse_deployment(
        optional_env(PROVIDER_ENV).as_deref(),
        optional_env(GRPC_ENDPOINT_ENV).as_deref(),
        optional_env(PUBLIC_ORIGIN_ENV).as_deref(),
        optional_env(TLS_DOMAIN_ENV).as_deref(),
        optional_env(CONNECT_TIMEOUT_MS_ENV).as_deref(),
        optional_env(ALLOW_INSECURE_LOOPBACK_ENV).as_deref(),
    )
}

fn parse_deployment(
    provider: Option<&str>,
    endpoint: Option<&str>,
    public_origin: Option<&str>,
    tls_domain: Option<&str>,
    connect_timeout_ms: Option<&str>,
    allow_insecure_loopback: Option<&str>,
) -> std::result::Result<ProfileMediaPublicImageDeployment, String> {
    let provider = provider
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("embedded")
        .to_ascii_lowercase();
    let endpoint = normalize_optional(endpoint);
    let public_origin = normalize_optional(public_origin);
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
                || public_origin.is_some()
                || tls_domain.is_some()
                || connect_timeout_ms.is_some()
                || allow_insecure_loopback_is_configured
            {
                return Err(format!(
                    "remote Media variables require {PROVIDER_ENV}=grpc"
                ));
            }
            Ok(ProfileMediaPublicImageDeployment::Embedded)
        }
        "grpc" => {
            let endpoint = endpoint.ok_or_else(|| {
                format!("{GRPC_ENDPOINT_ENV} is required when {PROVIDER_ENV}=grpc")
            })?;
            Ok(ProfileMediaPublicImageDeployment::Grpc(
                ProfileMediaPublicImageGrpcDeployment {
                    endpoint,
                    public_origin,
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

#[cfg(test)]
mod tests {
    use super::{
        ProfileMediaPublicImageDeployment, ProfileMediaPublicImageGrpcDeployment, parse_deployment,
    };

    #[test]
    fn embedded_is_the_default() {
        assert_eq!(
            parse_deployment(None, None, None, None, None, None).unwrap(),
            ProfileMediaPublicImageDeployment::Embedded
        );
    }

    #[test]
    fn grpc_requires_an_endpoint() {
        let error = parse_deployment(Some("grpc"), None, None, None, None, None)
            .expect_err("remote deployment without endpoint must fail closed");
        assert!(error.contains("RUSTOK_PROFILE_MEDIA_GRPC_ENDPOINT"));
    }

    #[test]
    fn grpc_configuration_preserves_transport_inputs() {
        assert_eq!(
            parse_deployment(
                Some("grpc"),
                Some("https://media.internal:7443"),
                Some("https://media.example.com"),
                Some("media.internal"),
                Some("2500"),
                Some("false"),
            )
            .unwrap(),
            ProfileMediaPublicImageDeployment::Grpc(ProfileMediaPublicImageGrpcDeployment {
                endpoint: "https://media.internal:7443".to_string(),
                public_origin: Some("https://media.example.com".to_string()),
                tls_domain: Some("media.internal".to_string()),
                connect_timeout_ms: 2500,
                allow_insecure_loopback: false,
            })
        );
    }

    #[test]
    fn remote_variables_are_not_silently_ignored_in_embedded_mode() {
        let error = parse_deployment(
            Some("embedded"),
            Some("https://media.internal"),
            None,
            None,
            None,
            None,
        )
        .expect_err("remote variables in embedded mode must be rejected");
        assert!(error.contains("require RUSTOK_PROFILE_MEDIA_PROVIDER=grpc"));
    }

    #[test]
    fn explicit_loopback_flag_is_not_silently_ignored_in_embedded_mode() {
        let error = parse_deployment(Some("embedded"), None, None, None, None, Some("false"))
            .expect_err("explicit remote loopback configuration must require grpc mode");
        assert!(error.contains("require RUSTOK_PROFILE_MEDIA_PROVIDER=grpc"));
    }

    #[test]
    fn invalid_boolean_is_rejected() {
        let error = parse_deployment(
            Some("grpc"),
            Some("https://media.internal"),
            None,
            None,
            None,
            Some("maybe"),
        )
        .expect_err("invalid loopback flag must fail closed");
        assert!(error.contains("must be a boolean"));
    }
}
