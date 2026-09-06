#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::model::PageBuilderScenarioReleaseStatus;

const PAGE_BUILDER_SCENARIO_RELEASE_STATUS_QUERY: &str = "query PageBuilderScenarioReleaseStatus($pageId: UUID!) { pageBuilderScenarioReleaseStatus(pageId: $pageId) { pageId baselinePresent allowed status baselineId baselineHash visualChanges breakingChanges diagnostics } }";

#[derive(Debug, Serialize)]
struct Variables {
    #[serde(rename = "pageId")]
    page_id: String,
}

#[derive(Debug, Deserialize)]
struct Response {
    #[serde(rename = "pageBuilderScenarioReleaseStatus")]
    status: PageBuilderScenarioReleaseStatus,
}

pub async fn fetch(
    token: Option<String>,
    tenant_slug: Option<String>,
    page_id: String,
) -> Result<PageBuilderScenarioReleaseStatus, GraphqlHttpError> {
    let response: Response = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            PAGE_BUILDER_SCENARIO_RELEASE_STATUS_QUERY,
            Some(Variables { page_id }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await?;
    Ok(response.status)
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
