use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use serde::Deserialize;

use crate::model::*;

#[derive(Debug, Clone, thiserror::Error)]
pub enum ApiError {
    #[error("GraphQL error: {0}")]
    Graphql(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Network error: {0}")]
    Network(String),
}

impl From<rustok_graphql::GraphqlHttpError> for ApiError {
    fn from(err: rustok_graphql::GraphqlHttpError) -> Self {
        Self::Graphql(err.to_string())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

fn graphql_url() -> String {
    "/api/graphql".to_string()
}

const MESSENGER_OVERVIEW_QUERY: &str = r#"
query MessengerOverview {
    messengerOverview {
        activeConversationsCount
        streamTopic
        consumerGroup
        partitionsCount
        registeredDevicesCount
        matrixServerName
        federatedRoomsCount
    }
}
"#;

#[derive(Deserialize)]
struct MessengerOverviewResponse {
    #[serde(rename = "messengerOverview")]
    overview: Option<MessengerOverviewPayload>,
}

pub async fn fetch_overview(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MessengerOverviewPayload, ApiError> {
    let response: MessengerOverviewResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(MESSENGER_OVERVIEW_QUERY, None::<serde_json::Value>),
        token,
        tenant_slug,
        None,
    )
    .await?;

    Ok(response.overview.unwrap_or_else(crate::core::default_overview_payload))
}

const LIST_CHANNELS_QUERY: &str = r#"
query ListChannels {
    messengerChannels {
        id
        conversationType
        title
        topic
        memberCount
        createdAt
    }
}
"#;

#[derive(Deserialize)]
struct ListChannelsResponse {
    #[serde(rename = "messengerChannels")]
    channels: Option<Vec<ChannelListItemPayload>>,
}

pub async fn fetch_channels(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<ChannelListItemPayload>, ApiError> {
    let response: ListChannelsResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(LIST_CHANNELS_QUERY, None::<serde_json::Value>),
        token,
        tenant_slug,
        None,
    )
    .await?;

    Ok(response.channels.unwrap_or_default())
}

const LIST_E2EE_DEVICES_QUERY: &str = r#"
query ListE2eeDevices {
    messengerE2eeDevices {
        deviceId
        userId
        hasCurve25519
        hasEd25519
        otkCount
    }
}
"#;

#[derive(Deserialize)]
struct ListE2eeDevicesResponse {
    #[serde(rename = "messengerE2eeDevices")]
    devices: Option<Vec<E2eeDeviceAuditPayload>>,
}

pub async fn fetch_e2ee_devices(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<E2eeDeviceAuditPayload>, ApiError> {
    let response: ListE2eeDevicesResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(LIST_E2EE_DEVICES_QUERY, None::<serde_json::Value>),
        token,
        tenant_slug,
        None,
    )
    .await?;

    Ok(response.devices.unwrap_or_default())
}
