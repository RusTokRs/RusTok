use rustok_graphql::{GRAPHQL_ENDPOINT, GraphqlRequest, execute};
use serde::{Deserialize, Serialize};

use super::native_server_adapter::ApiError;
use crate::model::{EventDeliveryConfiguration, EventDeliveryUpdate};

const CONFIGURATION_QUERY: &str = "query EventDeliveryConfiguration { eventDeliveryConfiguration { activeProfile desiredProfile iggyMode iggyConfigured restartRequired } }";
const UPDATE_MUTATION: &str = "mutation UpdateEventDeliveryConfiguration($input: UpdateEventDeliveryConfigurationInput!) { updateEventDeliveryConfiguration(input: $input) { desiredProfile restartRequired } }";

#[derive(Serialize)]
struct EmptyVariables {}

#[derive(Deserialize)]
struct ConfigurationResponse {
    #[serde(rename = "eventDeliveryConfiguration")]
    configuration: ConfigurationPayload,
}

#[derive(Deserialize)]
struct ConfigurationPayload {
    #[serde(rename = "activeProfile")]
    active_profile: String,
    #[serde(rename = "desiredProfile")]
    desired_profile: String,
    #[serde(rename = "iggyMode")]
    iggy_mode: String,
    #[serde(rename = "iggyConfigured")]
    iggy_configured: bool,
    #[serde(rename = "restartRequired")]
    restart_required: bool,
}

#[derive(Serialize)]
struct UpdateVariables {
    input: UpdateInput,
}

#[derive(Serialize)]
struct UpdateInput {
    profile: String,
}

#[derive(Deserialize)]
struct UpdateResponse {
    #[serde(rename = "updateEventDeliveryConfiguration")]
    update: UpdatePayload,
}

#[derive(Deserialize)]
struct UpdatePayload {
    #[serde(rename = "desiredProfile")]
    desired_profile: String,
    #[serde(rename = "restartRequired")]
    restart_required: bool,
}

pub(super) async fn fetch_configuration(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<EventDeliveryConfiguration, ApiError> {
    let response: ConfigurationResponse = execute(
        GRAPHQL_ENDPOINT,
        GraphqlRequest::new(CONFIGURATION_QUERY, Some(EmptyVariables {})),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))?;

    Ok(EventDeliveryConfiguration {
        active_profile: response.configuration.active_profile,
        desired_profile: response.configuration.desired_profile,
        iggy_mode: response.configuration.iggy_mode,
        iggy_configured: response.configuration.iggy_configured,
        restart_required: response.configuration.restart_required,
    })
}

pub(super) async fn update_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    profile: String,
) -> Result<EventDeliveryUpdate, ApiError> {
    let response: UpdateResponse = execute(
        GRAPHQL_ENDPOINT,
        GraphqlRequest::new(
            UPDATE_MUTATION,
            Some(UpdateVariables {
                input: UpdateInput { profile },
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))?;

    Ok(EventDeliveryUpdate {
        desired_profile: response.update.desired_profile,
        restart_required: response.update.restart_required,
    })
}
