use super::{ApiError, configured_tenant_slug};
use crate::model::{StorefrontMenu, StorefrontMenuLocation};
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};
const QUERY: &str = r#"query StorefrontActiveMenu($location: GqlMenuLocation!, $locale: String) {
  activeMenu(location: $location, locale: $locale) { id effectiveLocale name location items { id title url icon children { id title url icon children { id title url icon } } } }
}"#;
#[derive(Debug, Deserialize)]
struct Response {
    #[serde(rename = "activeMenu")]
    active_menu: Option<StorefrontMenu>,
}
#[derive(Debug, Serialize)]
struct Variables {
    location: StorefrontMenuLocation,
    locale: Option<String>,
}
pub async fn fetch_active_menu(
    location: StorefrontMenuLocation,
    locale: Option<String>,
) -> Result<Option<StorefrontMenu>, ApiError> {
    let response: Response = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(QUERY, Some(Variables { location, locale })),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))?;
    Ok(response.active_menu)
}
fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}
