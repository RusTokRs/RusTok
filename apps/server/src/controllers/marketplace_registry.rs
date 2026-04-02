use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{
        header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH},
        HeaderMap, HeaderName, HeaderValue, Response, StatusCode,
    },
    response::IntoResponse,
    routing::get,
    Json,
};
use loco_rs::app::AppContext;
use loco_rs::controller::Routes;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;

use crate::error::Error;
use crate::modules::{CatalogManifestModule, ManifestManager, ModulesManifest};
use crate::services::marketplace_catalog::{
    legacy_registry_catalog_module_path, legacy_registry_catalog_path,
    registry_catalog_from_modules, registry_catalog_module_path, registry_catalog_path,
    RegistryCatalogModule, RegistryCatalogResponse,
};

#[derive(Debug, Default, Deserialize, ToSchema, utoipa::IntoParams)]
struct RegistryCatalogListParams {
    search: Option<String>,
    category: Option<String>,
    tag: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

/// GET /v1/catalog - Reference read-only marketplace registry catalog
#[utoipa::path(
    get,
    path = "/v1/catalog",
    tag = "marketplace",
    params(
        RegistryCatalogListParams,
        ("If-None-Match" = Option<String>, Header, description = "Conditional request ETag")
    ),
    responses(
        (
            status = 200,
            description = "Schema-versioned reference catalog of first-party modules with optional filtering and paging",
            body = RegistryCatalogResponse,
            headers(
                ("etag" = String, description = "Current entity tag for conditional GET"),
                ("cache-control" = String, description = "Shared cache policy for the reference registry"),
                ("x-total-count" = i64, description = "Total number of modules in the filtered collection before limit/offset")
            )
        ),
        (
            status = 304,
            description = "Catalog has not changed since the provided ETag",
            headers(
                ("etag" = String, description = "Current entity tag for conditional GET"),
                ("cache-control" = String, description = "Shared cache policy for the reference registry"),
                ("x-total-count" = i64, description = "Total number of modules in the filtered collection before limit/offset")
            )
        )
    )
)]
async fn catalog(
    State(_ctx): State<AppContext>,
    headers: HeaderMap,
    Query(params): Query<RegistryCatalogListParams>,
) -> Result<Response<Body>, Error> {
    let first_party_modules = sort_catalog_modules(filter_catalog_modules(
        first_party_catalog_modules()?,
        &params,
    ));
    let (first_party_modules, total_count) = paginate_catalog_modules(first_party_modules, &params);
    let payload = registry_catalog_from_modules(first_party_modules);

    build_registry_response(&headers, &payload, Some(total_count))
}

/// GET /v1/catalog/{slug} - Reference read-only marketplace registry module detail
#[utoipa::path(
    get,
    path = "/v1/catalog/{slug}",
    tag = "marketplace",
    params(
        ("slug" = String, Path, description = "Module slug"),
        ("If-None-Match" = Option<String>, Header, description = "Conditional request ETag")
    ),
    responses(
        (
            status = 200,
            description = "Normalized first-party module detail from the reference registry catalog",
            body = RegistryCatalogModule,
            headers(
                ("etag" = String, description = "Current entity tag for conditional GET"),
                ("cache-control" = String, description = "Shared cache policy for the reference registry")
            )
        ),
        (
            status = 304,
            description = "Module detail has not changed since the provided ETag",
            headers(
                ("etag" = String, description = "Current entity tag for conditional GET"),
                ("cache-control" = String, description = "Shared cache policy for the reference registry")
            )
        ),
        (
            status = 404,
            description = "Module is not present in the reference registry catalog"
        )
    )
)]
async fn catalog_module(
    State(_ctx): State<AppContext>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Result<Response<Body>, Error> {
    let module = first_party_catalog_modules()?
        .into_iter()
        .find(|module| module.slug == slug)
        .map(RegistryCatalogModule::from_catalog_module)
        .ok_or(Error::NotFound)?;

    build_registry_response(&headers, &module, None)
}

pub fn routes() -> Routes {
    Routes::new()
        .add(registry_catalog_path(), get(catalog))
        .add(legacy_registry_catalog_path(), get(catalog))
        .add(registry_catalog_module_path(), get(catalog_module))
        .add(legacy_registry_catalog_module_path(), get(catalog_module))
}

fn first_party_catalog_modules() -> Result<Vec<CatalogManifestModule>, Error> {
    let manifest = ManifestManager::load().unwrap_or_else(|error| {
        tracing::warn!(
            error = %error,
            "Failed to load modules manifest for registry catalog; falling back to builtin catalog"
        );
        ModulesManifest::default()
    });
    let modules = catalog_modules_with_builtin_fallback(&manifest)
        .map_err(|error| Error::Message(format!("Failed to build marketplace catalog: {error}")))?;

    Ok(modules
        .into_iter()
        .filter(|module| module.ownership == "first_party")
        .collect())
}

fn filter_catalog_modules(
    modules: Vec<CatalogManifestModule>,
    params: &RegistryCatalogListParams,
) -> Vec<CatalogManifestModule> {
    let search = params
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let category = params
        .category
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let tag = params
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    modules
        .into_iter()
        .filter(|module| {
            search.is_none_or(|search| {
                let search = search.to_ascii_lowercase();
                module.slug.to_ascii_lowercase().contains(&search)
                    || module
                        .name
                        .as_deref()
                        .is_some_and(|name| name.to_ascii_lowercase().contains(&search))
                    || module.description.as_deref().is_some_and(|description| {
                        description.to_ascii_lowercase().contains(&search)
                    })
            })
        })
        .filter(|module| {
            category.is_none_or(|category| {
                module
                    .category
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(category))
            })
        })
        .filter(|module| {
            tag.is_none_or(|tag| {
                module
                    .tags
                    .iter()
                    .any(|value| value.eq_ignore_ascii_case(tag))
            })
        })
        .collect()
}

fn catalog_modules_with_builtin_fallback(
    manifest: &ModulesManifest,
) -> Result<Vec<CatalogManifestModule>, crate::modules::ManifestError> {
    match ManifestManager::catalog_modules(manifest) {
        Ok(modules) => Ok(modules),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Registry catalog generation fell back to builtin first-party module catalog"
            );
            ManifestManager::catalog_modules(&ModulesManifest::default())
        }
    }
}

fn sort_catalog_modules(mut modules: Vec<CatalogManifestModule>) -> Vec<CatalogManifestModule> {
    modules.sort_by(|left, right| {
        left.slug
            .cmp(&right.slug)
            .then_with(|| left.crate_name.cmp(&right.crate_name))
    });
    modules
}

fn paginate_catalog_modules(
    modules: Vec<CatalogManifestModule>,
    params: &RegistryCatalogListParams,
) -> (Vec<CatalogManifestModule>, usize) {
    let total_count = modules.len();
    let offset = params.offset.unwrap_or(0).min(total_count);
    let limit = params.limit.map(|value| value.min(100));

    let modules = modules
        .into_iter()
        .skip(offset)
        .take(limit.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();

    (modules, total_count)
}

fn build_registry_response<T>(
    headers: &HeaderMap,
    payload: &T,
    total_count: Option<usize>,
) -> Result<Response<Body>, Error>
where
    T: serde::Serialize,
{
    let etag = registry_etag(payload)?;
    let etag_header = HeaderValue::from_str(&etag)
        .map_err(|err| Error::Message(format!("Failed to build registry ETag header: {err}")))?;
    let total_count_header = total_count.map(registry_total_count_header).transpose()?;
    if request_matches_etag(headers, &etag) {
        let mut builder = Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(CACHE_CONTROL, registry_cache_control())
            .header(ETAG, etag_header.clone());
        if let Some(total_count_header) = total_count_header.as_ref() {
            builder = builder.header(registry_total_count_header_name(), total_count_header);
        }
        return builder.body(Body::empty()).map_err(|err| {
            Error::Message(format!("Failed to build registry 304 response: {err}"))
        });
    }

    let mut response = Json(payload).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, registry_cache_control());
    response.headers_mut().insert(ETAG, etag_header);
    if let Some(total_count_header) = total_count_header {
        response
            .headers_mut()
            .insert(registry_total_count_header_name(), total_count_header);
    }

    Ok(response)
}

fn registry_cache_control() -> HeaderValue {
    HeaderValue::from_static("public, max-age=60")
}

fn registry_total_count_header_name() -> HeaderName {
    HeaderName::from_static("x-total-count")
}

fn registry_total_count_header(total_count: usize) -> Result<HeaderValue, Error> {
    HeaderValue::from_str(&total_count.to_string()).map_err(|err| {
        Error::Message(format!(
            "Failed to build registry total-count header: {err}"
        ))
    })
}

fn registry_etag<T>(payload: &T) -> Result<String, Error>
where
    T: serde::Serialize,
{
    let body = serde_json::to_vec(payload)
        .map_err(|err| Error::Message(format!("Failed to serialize registry payload: {err}")))?;
    let hash = Sha256::digest(body);
    Ok(format!("\"{}\"", hex::encode(hash)))
}

fn request_matches_etag(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .any(|candidate| candidate == "*" || candidate == etag)
        })
        .unwrap_or(false)
}
