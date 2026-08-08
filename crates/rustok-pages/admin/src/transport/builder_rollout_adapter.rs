#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use rustok_page_builder::rollout::BuilderCapabilityFlags;
use serde::Deserialize;
use serde_json::json;

const PAGE_BUILDER_ROLLOUT_QUERY: &str = "query PageBuilderRolloutSnapshot { pageBuilderRolloutSnapshot { tenantSlug builderEnabled previewEnabled propertiesEnabled publishEnabled providerHealthObserved } }";

#[derive(Debug, Deserialize)]
struct PageBuilderRolloutResponse {
    #[serde(rename = "pageBuilderRolloutSnapshot")]
    page_builder_rollout_snapshot: PageBuilderRolloutPayload,
}

#[derive(Debug, Deserialize)]
struct PageBuilderRolloutPayload {
    #[serde(rename = "tenantSlug")]
    tenant_slug: String,
    #[serde(rename = "builderEnabled")]
    builder_enabled: bool,
    #[serde(rename = "previewEnabled")]
    preview_enabled: bool,
    #[serde(rename = "propertiesEnabled")]
    properties_enabled: bool,
    #[serde(rename = "publishEnabled")]
    publish_enabled: bool,
    #[serde(rename = "providerHealthObserved")]
    provider_health_observed: bool,
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

pub async fn fetch(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<(String, BuilderCapabilityFlags), GraphqlHttpError> {
    let response: PageBuilderRolloutResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(PAGE_BUILDER_ROLLOUT_QUERY, Some(json!({}))),
        token,
        tenant_slug,
        None,
    )
    .await?;
    let payload = response.page_builder_rollout_snapshot;
    if payload.provider_health_observed {
        return Err(GraphqlHttpError::Graphql(
            "Pages rollout snapshot unexpectedly claimed observed provider health".to_string(),
        ));
    }
    let flags = BuilderCapabilityFlags {
        builder_enabled: payload.builder_enabled,
        preview_enabled: payload.preview_enabled,
        properties_enabled: payload.properties_enabled,
        publish_enabled: payload.publish_enabled,
    };
    flags
        .validate()
        .map_err(|error| GraphqlHttpError::Graphql(error.to_string()))?;
    Ok((payload.tenant_slug, flags))
}
