use rustok_graphql::{GRAPHQL_ENDPOINT, GraphqlRequest, execute};
use serde::{Deserialize, Serialize};

use super::native_server_adapter::ApiError;
use crate::model::{IggyConnectorConfiguration, IggyConnectorForm, IggyConnectorUpdate};

const CONFIGURATION_QUERY: &str = "query IggyConnectorConfiguration { iggyConnectorConfiguration { activeMode desiredMode bundledAvailable externalAddresses externalUsername passwordResolver passwordKey passwordConfigured tlsEnabled tlsDomain configured configurationError restartRequired } }";
const UPDATE_MUTATION: &str = "mutation UpdateIggyConnectorConfiguration($input: UpdateIggyConnectorConfigurationInput!) { updateIggyConnectorConfiguration(input: $input) { desiredMode configured restartRequired } }";

#[derive(Serialize)]
struct EmptyVariables {}

#[derive(Deserialize)]
struct ConfigurationResponse {
    #[serde(rename = "iggyConnectorConfiguration")]
    configuration: IggyConnectorConfiguration,
}

#[derive(Serialize)]
struct UpdateVariables {
    input: UpdateInput,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInput {
    mode: String,
    external_addresses: Vec<String>,
    external_username: String,
    password_resolver: String,
    password_key: String,
    tls_enabled: bool,
    tls_domain: Option<String>,
}

#[derive(Deserialize)]
struct UpdateResponse {
    #[serde(rename = "updateIggyConnectorConfiguration")]
    update: UpdatePayload,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdatePayload {
    desired_mode: String,
    configured: bool,
    restart_required: bool,
}

pub(super) async fn fetch_configuration(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<IggyConnectorConfiguration, ApiError> {
    let response: ConfigurationResponse = execute(
        GRAPHQL_ENDPOINT,
        GraphqlRequest::new(CONFIGURATION_QUERY, Some(EmptyVariables {})),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))?;
    Ok(response.configuration)
}

pub(super) async fn update_configuration(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: IggyConnectorForm,
) -> Result<IggyConnectorUpdate, ApiError> {
    let response: UpdateResponse = execute(
        GRAPHQL_ENDPOINT,
        GraphqlRequest::new(
            UPDATE_MUTATION,
            Some(UpdateVariables {
                input: UpdateInput {
                    mode: input.mode,
                    external_addresses: input.external_addresses,
                    external_username: input.external_username,
                    password_resolver: input.password_resolver,
                    password_key: input.password_key,
                    tls_enabled: input.tls_enabled,
                    tls_domain: input.tls_domain,
                },
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))?;
    Ok(IggyConnectorUpdate {
        desired_mode: response.update.desired_mode,
        configured: response.update.configured,
        restart_required: response.update.restart_required,
    })
}
