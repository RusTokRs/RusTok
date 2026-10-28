use rustok_api::graphql::GraphqlRuntimeInputs;
use std::sync::Arc;

use crate::controllers::realtime::MessengerRoomHub;
use crate::e2ee::DeviceKeyRegistry;

/// Manifest-attached runtime capabilities for the Messenger GraphQL schema.
#[derive(Clone, Default)]
pub struct MessengerGraphqlRuntimeData {
    pub device_keys: Arc<DeviceKeyRegistry>,
    pub room_hub: Arc<MessengerRoomHub>,
}

/// Attach shared device registry and real-time room hub to the GraphQL schema.
pub fn attach_schema_data(
    inputs: &GraphqlRuntimeInputs,
) -> Result<MessengerGraphqlRuntimeData, String> {
    let device_keys = inputs
        .shared_get::<Arc<DeviceKeyRegistry>>()
        .unwrap_or_else(|| Arc::new(DeviceKeyRegistry::new()));
    let room_hub = inputs
        .shared_get::<Arc<MessengerRoomHub>>()
        .unwrap_or_else(|| Arc::new(MessengerRoomHub::new()));

    Ok(MessengerGraphqlRuntimeData {
        device_keys,
        room_hub,
    })
}
