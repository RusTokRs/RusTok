use std::time::Duration;

use rustok_api::PortError;
use tonic::metadata::{Ascii, MetadataValue};
use uuid::Uuid;

#[cfg(feature = "grpc-client")]
mod client;
mod payload;
#[cfg(feature = "grpc-server")]
mod server;

#[cfg(feature = "grpc-client")]
pub use client::GrpcFlexAttachedSchemaProvider;
#[cfg(feature = "grpc-server")]
pub use server::FlexAttachedSchemaGrpcService;

pub mod proto {
    tonic::include_proto!("rustok.flex.v1");
}

const AUTHORIZATION: &str = "authorization";
const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_DEADLINE: Duration = Duration::from_secs(30);

fn authorization(secret: &str) -> Result<MetadataValue<Ascii>, PortError> {
    if secret.is_empty()
        || secret.len() > 4096
        || !secret.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(PortError::validation(
            "flex.grpc_token_invalid",
            "Flex gRPC token is invalid",
        ));
    }
    let mut metadata: MetadataValue<Ascii> = format!("Bearer {secret}").parse().map_err(|_| {
        PortError::validation("flex.grpc_token_invalid", "Flex gRPC token is invalid")
    })?;
    metadata.set_sensitive(true);
    Ok(metadata)
}

fn tenant_uuid(tenant: &str) -> Option<Uuid> {
    Uuid::parse_str(tenant).ok().filter(|id| !id.is_nil())
}

fn call_timeout(deadline_ms: u64) -> Option<Duration> {
    let duration = Duration::from_millis(deadline_ms);
    (!duration.is_zero() && duration <= MAX_DEADLINE).then_some(duration)
}

pub fn parse_snapshot_tenants(value: &str) -> Result<Vec<Uuid>, PortError> {
    let mut tenants = std::collections::HashSet::new();
    let mut ordered = Vec::new();
    for entry in value.split(',') {
        let tenant = tenant_uuid(entry.trim()).filter(|id| tenants.insert(*id));
        if let Some(tenant) = tenant.filter(|_| ordered.len() < 128) {
            ordered.push(tenant);
        } else {
            return Err(PortError::validation(
                "flex.grpc_snapshot_tenants_invalid",
                "Flex schema transport requires one to 128 distinct tenant UUIDs",
            ));
        }
    }
    Ok(ordered)
}
