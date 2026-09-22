use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql, graphql_url};
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

