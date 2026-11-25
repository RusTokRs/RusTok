use std::env;

use anyhow::{Context, Result, bail};
use rustok_taxonomy_ports::grpc::GrpcTaxonomyTermNamesProvider;

const ENDPOINT_ENV: &str = "RUSTOK_PRODUCT_TAXONOMY_GRPC_ENDPOINT";
const TOKEN_ENV: &str = "RUSTOK_TAXONOMY_TERM_NAMES_GRPC_BEARER_TOKEN";
const LOOPBACK_ENV: &str = "RUSTOK_PRODUCT_TAXONOMY_GRPC_ALLOW_INSECURE_LOOPBACK";
const CA_CERT_ENV: &str = "RUSTOK_PRODUCT_TAXONOMY_GRPC_CA_CERT_PATH";

pub(super) fn provider_from_environment() -> Result<Option<GrpcTaxonomyTermNamesProvider>> {
    let endpoint = match env::var(ENDPOINT_ENV) {
        Ok(endpoint) => endpoint,
        Err(env::VarError::NotPresent) => {
            for setting in [LOOPBACK_ENV, CA_CERT_ENV] {
                if env::var_os(setting).is_some() {
                    bail!("{setting} requires {ENDPOINT_ENV}");
                }
            }
            return Ok(None);
        }
        Err(error) => return Err(error).context(ENDPOINT_ENV),
    };
    let token = super::required_secret_env(TOKEN_ENV)?;
    let allow_loopback = super::optional_bool(LOOPBACK_ENV, false)?;
    let ca_pem = env::var_os(CA_CERT_ENV)
        .map(std::fs::read)
        .transpose()
        .context(CA_CERT_ENV)?;
    Ok(Some(
        GrpcTaxonomyTermNamesProvider::new(&endpoint, &token, allow_loopback, ca_pem.as_deref())
            .context("Taxonomy term name gRPC configuration is invalid")?,
    ))
}
