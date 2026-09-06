#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use rustok_page_builder::runtime_scenario_release::RuntimeScenarioReleaseBaseline;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SAVE_MUTATION: &str = "mutation SavePageBuilderScenarioBaseline($pageId: UUID!, $input: SaveGqlPageBuilderScenarioBaselineInput!) { savePageBuilderScenarioBaseline(pageId: $pageId, input: $input) { baseline } }";
const DELETE_MUTATION: &str = "mutation DeletePageBuilderScenarioBaseline($pageId: UUID!, $expectedBaselineHash: String) { deletePageBuilderScenarioBaseline(pageId: $pageId, expectedBaselineHash: $expectedBaselineHash) }";

#[derive(Debug, Serialize)]
struct SaveVariables {
    #[serde(rename = "pageId")]
    page_id: String,
    input: SaveInput,
}

#[derive(Debug, Serialize)]
struct SaveInput {
    baseline: Value,
    #[serde(
        rename = "expectedBaselineHash",
        skip_serializing_if = "Option::is_none"
    )]
    expected_baseline_hash: Option<String>,
    #[serde(rename = "promotionNote", skip_serializing_if = "Option::is_none")]
    promotion_note: Option<String>,
}

#[derive(Debug, Serialize)]
struct DeleteVariables {
    #[serde(rename = "pageId")]
    page_id: String,
    #[serde(
        rename = "expectedBaselineHash",
        skip_serializing_if = "Option::is_none"
    )]
    expected_baseline_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BaselinePayload {
    baseline: Value,
}

#[derive(Debug, Deserialize)]
struct SaveResponse {
    #[serde(rename = "savePageBuilderScenarioBaseline")]
    saved: BaselinePayload,
}

#[derive(Debug, Deserialize)]
struct DeleteResponse {
    #[serde(rename = "deletePageBuilderScenarioBaseline")]
    deleted: bool,
}

pub async fn save(
    token: Option<String>,
    tenant_slug: Option<String>,
    page_id: String,
    baseline: RuntimeScenarioReleaseBaseline,
    expected_baseline_hash: Option<String>,
    promotion_note: Option<String>,
) -> Result<RuntimeScenarioReleaseBaseline, GraphqlHttpError> {
    let baseline = serde_json::to_value(baseline).map_err(|error| {
        GraphqlHttpError::Graphql(format!("Unable to encode scenario baseline: {error}"))
    })?;
    let response: SaveResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            SAVE_MUTATION,
            Some(SaveVariables {
                page_id,
                input: SaveInput {
                    baseline,
                    expected_baseline_hash,
                    promotion_note,
                },
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await?;
    serde_json::from_value(response.saved.baseline).map_err(|error| {
        GraphqlHttpError::Graphql(format!(
            "Invalid saved Page Builder scenario baseline response: {error}"
        ))
    })
}

pub async fn delete(
    token: Option<String>,
    tenant_slug: Option<String>,
    page_id: String,
    expected_baseline_hash: Option<String>,
) -> Result<bool, GraphqlHttpError> {
    let response: DeleteResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            DELETE_MUTATION,
            Some(DeleteVariables {
                page_id,
                expected_baseline_hash,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await?;
    Ok(response.deleted)
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
