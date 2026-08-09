#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use rustok_page_builder::{
    health::{ProviderHealthSnapshot, ProviderSloObservations},
    rollout::BuilderCapabilityFlags,
};
use serde::Deserialize;
use serde_json::json;

const PAGE_BUILDER_ROLLOUT_QUERY: &str = "query PageBuilderRolloutSnapshot { pageBuilderRolloutSnapshot { tenantSlug builderEnabled previewEnabled propertiesEnabled publishEnabled providerHealthObserved providerHealth { state degradationReasons previewP95Ms publishP95Ms sanitizeFailureRate runtimeErrorRate } } }";

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
    #[serde(rename = "providerHealth")]
    provider_health: Option<PageBuilderProviderHealthPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageBuilderProviderHealthPayload {
    state: String,
    degradation_reasons: Vec<String>,
    preview_p95_ms: i64,
    publish_p95_ms: i64,
    sanitize_failure_rate: f64,
    runtime_error_rate: f64,
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

fn health_transport_error(message: impl Into<String>) -> GraphqlHttpError {
    GraphqlHttpError::Graphql(format!(
        "Pages rollout provider health transport is invalid: {}",
        message.into()
    ))
}

fn parse_provider_health(
    provider_health_observed: bool,
    payload: Option<PageBuilderProviderHealthPayload>,
) -> Result<Option<ProviderHealthSnapshot>, GraphqlHttpError> {
    match (provider_health_observed, payload) {
        (false, None) => Ok(None),
        (false, Some(_)) => Err(health_transport_error(
            "providerHealth payload is present while providerHealthObserved is false",
        )),
        (true, None) => Err(health_transport_error(
            "providerHealthObserved is true but providerHealth payload is missing",
        )),
        (true, Some(payload)) => {
            let preview_p95_ms = u64::try_from(payload.preview_p95_ms)
                .map_err(|_| health_transport_error("previewP95Ms must be non-negative"))?;
            let publish_p95_ms = u64::try_from(payload.publish_p95_ms)
                .map_err(|_| health_transport_error("publishP95Ms must be non-negative"))?;
            for (name, value) in [
                ("sanitizeFailureRate", payload.sanitize_failure_rate),
                ("runtimeErrorRate", payload.runtime_error_rate),
            ] {
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(health_transport_error(format!(
                        "{name} must be finite and between 0 and 1"
                    )));
                }
            }

            let snapshot = ProviderHealthSnapshot::evaluate(ProviderSloObservations {
                preview_p95_ms,
                publish_p95_ms,
                sanitize_failure_rate: payload.sanitize_failure_rate,
                runtime_error_rate: payload.runtime_error_rate,
            });
            if payload.state != snapshot.state.as_str() {
                return Err(health_transport_error(format!(
                    "state `{}` does not match canonical evaluation `{}`",
                    payload.state,
                    snapshot.state.as_str()
                )));
            }
            let expected_reasons: Vec<_> = snapshot
                .degradation_reasons
                .iter()
                .map(|reason| reason.as_str().to_string())
                .collect();
            if payload.degradation_reasons != expected_reasons {
                return Err(health_transport_error(
                    "degradationReasons do not match canonical evaluation",
                ));
            }
            Ok(Some(snapshot))
        }
    }
}

pub async fn fetch(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<(String, BuilderCapabilityFlags, Option<ProviderHealthSnapshot>), GraphqlHttpError> {
    let response: PageBuilderRolloutResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(PAGE_BUILDER_ROLLOUT_QUERY, Some(json!({}))),
        token,
        tenant_slug,
        None,
    )
    .await?;
    let payload = response.page_builder_rollout_snapshot;
    let provider_health = parse_provider_health(
        payload.provider_health_observed,
        payload.provider_health,
    )?;
    let flags = BuilderCapabilityFlags {
        builder_enabled: payload.builder_enabled,
        preview_enabled: payload.preview_enabled,
        properties_enabled: payload.properties_enabled,
        publish_enabled: payload.publish_enabled,
    };
    flags
        .validate()
        .map_err(|error| GraphqlHttpError::Graphql(error.to_string()))?;
    Ok((payload.tenant_slug, flags, provider_health))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn health_payload(
        state: &str,
        degradation_reasons: Vec<&str>,
    ) -> PageBuilderProviderHealthPayload {
        PageBuilderProviderHealthPayload {
            state: state.to_string(),
            degradation_reasons: degradation_reasons.into_iter().map(str::to_string).collect(),
            preview_p95_ms: 1_600,
            publish_p95_ms: 2_000,
            sanitize_failure_rate: 0.0,
            runtime_error_rate: 0.0,
        }
    }

    #[test]
    fn provider_health_transport_requires_boolean_payload_consistency() {
        assert!(parse_provider_health(false, None).unwrap().is_none());
        assert!(parse_provider_health(false, Some(health_payload("degraded", vec!["provider_unhealthy"]))).is_err());
        assert!(parse_provider_health(true, None).is_err());
    }

    #[test]
    fn provider_health_transport_recomputes_canonical_state_and_reasons() {
        let snapshot = parse_provider_health(
            true,
            Some(health_payload("degraded", vec!["provider_unhealthy"])),
        )
        .expect("valid transport")
        .expect("observed health");
        assert_eq!(snapshot.state.as_str(), "degraded");

        assert!(parse_provider_health(
            true,
            Some(health_payload("ready", vec![])),
        )
        .is_err());
    }
}
