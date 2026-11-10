use std::{env, error::Error, io, time::Duration};

use rustok_api::{PortActor, PortContext, PortError};
use rustok_product::{
    ProductCatalogCommandPort, ProductCatalogReadPort, ProductProjectionRequest,
    ProductVariantIdentitiesRequest, ProductVariantIdentityListRequest,
    ProductVariantIdentityReadPort, ProductVariantIdentitySnapshot, PublishedProductsRequest,
    StorefrontVariantProductProjectionRequest, VariantProductProjectionRequest,
    dto::SetVariantAxesInput,
};
use rustok_product_transport::{
    GrpcProductCatalogReadConnectionConfig, ProductCatalogGrpcBearerToken,
};
use uuid::Uuid;

const ENDPOINT_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT";
const BEARER_TOKEN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN";
const TLS_DOMAIN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_TLS_DOMAIN";
const ALLOW_INSECURE_LOOPBACK_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK";
const CONNECT_TIMEOUT_MS_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_CONNECT_TIMEOUT_MS";
const TENANT_ID_ENV: &str = "RUSTOK_PRODUCT_CATALOG_EVIDENCE_TENANT_ID";
const PRODUCT_ID_ENV: &str = "RUSTOK_PRODUCT_CATALOG_EVIDENCE_PRODUCT_ID";
const VARIANT_ID_ENV: &str = "RUSTOK_PRODUCT_CATALOG_EVIDENCE_VARIANT_ID";
const LOCALE_ENV: &str = "RUSTOK_PRODUCT_CATALOG_EVIDENCE_LOCALE";
const FALLBACK_LOCALE_ENV: &str = "RUSTOK_PRODUCT_CATALOG_EVIDENCE_FALLBACK_LOCALE";
const CHANNEL_SLUG_ENV: &str = "RUSTOK_PRODUCT_CATALOG_EVIDENCE_CHANNEL_SLUG";
const ENABLE_COMMANDS_ENV: &str = "RUSTOK_PRODUCT_CATALOG_EVIDENCE_ENABLE_COMMANDS";
const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 5_000;

struct ProbeConfig {
    endpoint: String,
    bearer_token: String,
    tls_domain: Option<String>,
    allow_insecure_loopback: bool,
    connect_timeout: Duration,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
    locale: String,
    fallback_locale: Option<String>,
    channel_slug: Option<String>,
    enable_commands: bool,
}

impl ProbeConfig {
    fn from_environment() -> Result<Self, io::Error> {
        Ok(Self {
            endpoint: required_env(ENDPOINT_ENV)?,
            bearer_token: required_secret_env(BEARER_TOKEN_ENV)?,
            tls_domain: optional_env(TLS_DOMAIN_ENV)?,
            allow_insecure_loopback: optional_bool(ALLOW_INSECURE_LOOPBACK_ENV, false)?,
            connect_timeout: Duration::from_millis(optional_bounded_u64(
                CONNECT_TIMEOUT_MS_ENV,
                DEFAULT_CONNECT_TIMEOUT_MS,
                1,
                30_000,
            )?),
            tenant_id: required_uuid(TENANT_ID_ENV)?,
            product_id: required_uuid(PRODUCT_ID_ENV)?,
            variant_id: required_uuid(VARIANT_ID_ENV)?,
            locale: bounded_text(
                optional_env(LOCALE_ENV)?.as_deref().unwrap_or("en"),
                LOCALE_ENV,
                2,
                32,
            )?,
            fallback_locale: optional_env(FALLBACK_LOCALE_ENV)?
                .map(|value| bounded_text(&value, FALLBACK_LOCALE_ENV, 2, 32))
                .transpose()?,
            channel_slug: optional_env(CHANNEL_SLUG_ENV)?
                .map(|value| bounded_text(&value, CHANNEL_SLUG_ENV, 1, 64))
                .transpose()?,
            enable_commands: optional_bool(ENABLE_COMMANDS_ENV, false)?,
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = ProbeConfig::from_environment()?;
    let authentication = ProductCatalogGrpcBearerToken::new(config.bearer_token.as_str())?;
    let provider = GrpcProductCatalogReadConnectionConfig::new(config.endpoint)
        .with_tls_domain(config.tls_domain)
        .with_connect_timeout(config.connect_timeout)
        .allow_insecure_loopback(config.allow_insecure_loopback)
        .connect()
        .await?
        .with_authentication(authentication);

    let mut context = PortContext::new(
        config.tenant_id.to_string(),
        PortActor::service("product-catalog-runtime-evidence-probe"),
        config.locale.clone(),
        "product-catalog-separate-process-runtime-evidence",
    )
    .with_deadline(config.connect_timeout);
    if let Some(channel_slug) = config.channel_slug.as_ref() {
        context = context.with_channel(channel_slug.clone());
    }

    let product = provider
        .read_product_projection(
            context.clone(),
            ProductProjectionRequest {
                product_id: config.product_id,
                locale: Some(config.locale.clone()),
                fallback_locale: config.fallback_locale.clone(),
            },
        )
        .await
        .map_err(|error| port_error("read_product_projection", error))?;
    if product.id != config.product_id || product.tenant_id != config.tenant_id {
        return Err(invalid("product projection identity mismatch").into());
    }

    let variant_product = provider
        .read_variant_product_projection(
            context.clone(),
            VariantProductProjectionRequest {
                variant_id: config.variant_id,
                locale: Some(config.locale.clone()),
                fallback_locale: config.fallback_locale.clone(),
            },
        )
        .await
        .map_err(|error| port_error("read_variant_product_projection", error))?;
    if variant_product.id != config.product_id
        || variant_product.tenant_id != config.tenant_id
        || !variant_product
            .variants
            .iter()
            .any(|variant| variant.id == config.variant_id)
    {
        return Err(invalid("variant projection identity mismatch").into());
    }

    let storefront_product = provider
        .read_storefront_variant_product_projection(
            context.clone(),
            StorefrontVariantProductProjectionRequest {
                variant_id: config.variant_id,
                locale: Some(config.locale.clone()),
                fallback_locale: config.fallback_locale.clone(),
                public_channel_slug: config.channel_slug.clone(),
            },
        )
        .await
        .map_err(|error| port_error("read_storefront_variant_product_projection", error))?
        .ok_or_else(|| invalid("SKU fixture is unavailable on the storefront"))?;
    if storefront_product.id != config.product_id
        || storefront_product.tenant_id != config.tenant_id
        || !storefront_product
            .variants
            .iter()
            .any(|variant| variant.id == config.variant_id)
    {
        return Err(invalid("storefront SKU projection identity mismatch").into());
    }

    let published = provider
        .list_published_products(
            context.clone(),
            PublishedProductsRequest {
                locale: Some(config.locale),
                fallback_locale: config.fallback_locale,
                public_channel_slug: config.channel_slug,
                page: 1,
                per_page: 48,
            },
        )
        .await
        .map_err(|error| port_error("list_published_products", error))?;
    if published.page != 1 || published.per_page != 48 || published.total == 0 {
        return Err(invalid("published product list evidence fixture is empty or invalid").into());
    }

    let expected_identity = ProductVariantIdentitySnapshot {
        tenant_id: config.tenant_id,
        product_id: config.product_id,
        variant_id: config.variant_id,
    };
    let identities = provider
        .read_variant_identities(
            context.clone(),
            ProductVariantIdentitiesRequest {
                product_id: Some(config.product_id),
                variant_ids: vec![config.variant_id],
            },
        )
        .await
        .map_err(|error| port_error("read_variant_identities", error))?;
    if identities.as_slice() != std::slice::from_ref(&expected_identity) {
        return Err(invalid("SKU identity lookup did not match the fixture").into());
    }

    let product_identities = provider
        .list_product_variant_identities(
            context.clone(),
            ProductVariantIdentityListRequest {
                product_ids: vec![config.product_id],
            },
        )
        .await
        .map_err(|error| port_error("list_product_variant_identities", error))?;
    if !product_identities.contains(&expected_identity)
        || product_identities.iter().any(|identity| {
            identity.tenant_id != config.tenant_id || identity.product_id != config.product_id
        })
    {
        return Err(invalid("Product SKU identity list did not match the fixture scope").into());
    }

    if config.enable_commands {
        let command_provider = GrpcProductCatalogReadConnectionConfig::new(config.endpoint)
            .with_tls_domain(config.tls_domain)
            .with_connect_timeout(config.connect_timeout)
            .allow_insecure_loopback(config.allow_insecure_loopback)
            .connect_command()
            .await?
            .with_authentication(authentication);

        let preview = command_provider
            .preview_variant_axes(
                context,
                config.product_id,
                SetVariantAxesInput::default(),
            )
            .await
            .map_err(|error| port_error("preview_variant_axes", error))?;
        if preview.product_id != config.product_id {
            return Err(invalid("variant axes preview product identity mismatch").into());
        }

        println!(
            "PRODUCT_CATALOG_COMMAND_RUNTIME_PROBE_OK operations=1 preview_axes=matched product_id=matched"
        );
    }

    println!(
        "PRODUCT_CATALOG_RUNTIME_PROBE_OK operations=6 product_projection=matched variant_projection=matched published_list=nonempty sku_lookup=matched sku_list=matched storefront_variant=matched"
    );
    Ok(())
}

fn port_error(operation: &'static str, error: PortError) -> io::Error {
    io::Error::other(format!(
        "{operation} failed with Product port code {}",
        error.code
    ))
}

fn required_env(name: &str) -> Result<String, io::Error> {
    optional_env(name)?.ok_or_else(|| invalid(format!("{name} is required")))
}

fn required_secret_env(name: &str) -> Result<String, io::Error> {
    let value = env::var(name).map_err(|_| invalid(format!("{name} is required")))?;
    if value.is_empty() || value.trim() != value {
        return Err(invalid(format!(
            "{name} must be non-empty without surrounding whitespace"
        )));
    }
    Ok(value)
}

fn optional_env(name: &str) -> Result<Option<String>, io::Error> {
    let Some(value) = env::var(name).ok() else {
        return Ok(None);
    };
    let normalized = value.trim();
    if normalized.is_empty() {
        return Ok(None);
    }
    if normalized != value || normalized.chars().any(char::is_control) {
        return Err(invalid(format!(
            "{name} contains unsupported whitespace or control characters"
        )));
    }
    Ok(Some(normalized.to_string()))
}

fn required_uuid(name: &str) -> Result<Uuid, io::Error> {
    let value = required_env(name)?;
    Uuid::parse_str(&value).map_err(|_| invalid(format!("{name} must be a UUID")))
}

fn bounded_text(
    value: &str,
    name: &str,
    minimum: usize,
    maximum: usize,
) -> Result<String, io::Error> {
    if !(minimum..=maximum).contains(&value.len())
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(invalid(format!(
            "{name} is outside the runtime evidence boundary"
        )));
    }
    Ok(value.to_string())
}

fn optional_bool(name: &str, default: bool) -> Result<bool, io::Error> {
    let Some(value) = optional_env(name)? else {
        return Ok(default);
    };
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(invalid(format!("{name} must be a boolean"))),
    }
}

fn optional_bounded_u64(
    name: &str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, io::Error> {
    let Some(value) = optional_env(name)? else {
        return Ok(default);
    };
    let parsed = value
        .parse::<u64>()
        .map_err(|_| invalid(format!("{name} must be an integer")))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(invalid(format!(
            "{name} must be between {minimum} and {maximum}"
        )));
    }
    Ok(parsed)
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
