use std::collections::BTreeSet;

use thiserror::Error;
use url::Url;

/// Host-parsed build-surface facts consumed by the module control plane.
/// They contain no filesystem, server, or frontend implementation handles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformBuildSurfaceContract {
    pub embed_admin: bool,
    pub embed_storefront: bool,
    pub admin: PlatformAdminBuildSurfaceContract,
    pub storefronts: Vec<PlatformStorefrontBuildSurfaceContract>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformAdminBuildSurfaceContract {
    pub stack: String,
    pub public_url: String,
    pub redirect_uris: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformStorefrontBuildSurfaceContract {
    pub id: String,
    pub stack: String,
    pub public_url: String,
    pub redirect_uris: Vec<String>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PlatformBuildSurfaceValidationError {
    #[error("Standalone admin requires build.admin.public_url")]
    MissingAdminPublicUrl,
    #[error("Standalone admin requires at least one build.admin.redirect_uris entry")]
    MissingAdminRedirectUris,
    #[error("Each build.storefront entry requires a non-empty id")]
    MissingStorefrontId,
    #[error("Duplicate storefront id '{0}'")]
    DuplicateStorefrontId(String),
    #[error("Standalone storefront '{id}' requires public_url")]
    MissingStorefrontPublicUrl { id: String },
    #[error("Standalone storefront '{id}' requires at least one redirect_uri")]
    MissingStorefrontRedirectUris { id: String },
    #[error("{field} contains invalid URL '{value}': {reason}")]
    InvalidUrl {
        field: String,
        value: String,
        reason: String,
    },
}

/// Validates deployment-facing surface semantics after the host decodes its
/// manifest. Filesystem layout, environment loading, and frontend bootstrap
/// remain host adapter concerns.
pub fn validate_platform_build_surface_contract(
    contract: &PlatformBuildSurfaceContract,
) -> Result<(), PlatformBuildSurfaceValidationError> {
    if !contract.embed_admin && !contract.admin.stack.trim().is_empty() {
        if contract.admin.public_url.trim().is_empty() {
            return Err(PlatformBuildSurfaceValidationError::MissingAdminPublicUrl);
        }
        if contract.admin.redirect_uris.is_empty() {
            return Err(PlatformBuildSurfaceValidationError::MissingAdminRedirectUris);
        }
        validate_urls(&contract.admin.redirect_uris, "build.admin.redirect_uris")?;
        validate_url(&contract.admin.public_url, "build.admin.public_url")?;
    }

    let mut storefront_ids = BTreeSet::new();
    for storefront in &contract.storefronts {
        if storefront.id.trim().is_empty() {
            return Err(PlatformBuildSurfaceValidationError::MissingStorefrontId);
        }
        if !storefront_ids.insert(&storefront.id) {
            return Err(PlatformBuildSurfaceValidationError::DuplicateStorefrontId(
                storefront.id.clone(),
            ));
        }

        let standalone = !contract.embed_storefront || storefront.stack == "next";
        if !standalone {
            continue;
        }
        if storefront.public_url.trim().is_empty() {
            return Err(
                PlatformBuildSurfaceValidationError::MissingStorefrontPublicUrl {
                    id: storefront.id.clone(),
                },
            );
        }
        if storefront.redirect_uris.is_empty() {
            return Err(
                PlatformBuildSurfaceValidationError::MissingStorefrontRedirectUris {
                    id: storefront.id.clone(),
                },
            );
        }
        validate_url(
            &storefront.public_url,
            &format!("build.storefront[{}].public_url", storefront.id),
        )?;
        validate_urls(
            &storefront.redirect_uris,
            &format!("build.storefront[{}].redirect_uris", storefront.id),
        )?;
    }

    Ok(())
}

fn validate_urls(urls: &[String], field: &str) -> Result<(), PlatformBuildSurfaceValidationError> {
    for value in urls {
        validate_url(value, field)?;
    }
    Ok(())
}

fn validate_url(value: &str, field: &str) -> Result<(), PlatformBuildSurfaceValidationError> {
    Url::parse(value).map_err(|error| PlatformBuildSurfaceValidationError::InvalidUrl {
        field: field.to_string(),
        value: value.to_string(),
        reason: error.to_string(),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> PlatformBuildSurfaceContract {
        PlatformBuildSurfaceContract {
            embed_admin: false,
            embed_storefront: false,
            admin: PlatformAdminBuildSurfaceContract {
                stack: "next".to_string(),
                public_url: "https://admin.example.test".to_string(),
                redirect_uris: vec!["https://admin.example.test/auth/callback".to_string()],
            },
            storefronts: vec![PlatformStorefrontBuildSurfaceContract {
                id: "default".to_string(),
                stack: "next".to_string(),
                public_url: "https://store.example.test".to_string(),
                redirect_uris: vec!["https://store.example.test/auth/callback".to_string()],
            }],
        }
    }

    #[test]
    fn standalone_surfaces_require_urls_and_distinct_ids() {
        assert!(validate_platform_build_surface_contract(&contract()).is_ok());

        let mut missing_url = contract();
        missing_url.admin.public_url.clear();
        assert!(matches!(
            validate_platform_build_surface_contract(&missing_url),
            Err(PlatformBuildSurfaceValidationError::MissingAdminPublicUrl)
        ));

        let mut duplicate_storefront = contract();
        duplicate_storefront
            .storefronts
            .push(duplicate_storefront.storefronts[0].clone());
        assert!(matches!(
            validate_platform_build_surface_contract(&duplicate_storefront),
            Err(PlatformBuildSurfaceValidationError::DuplicateStorefrontId(
                _
            ))
        ));
    }
}
