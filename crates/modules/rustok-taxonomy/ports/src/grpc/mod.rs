use std::time::Duration;

use rustok_api::{PortError, PortErrorKind};
use tonic::{
    Code,
    metadata::{Ascii, MetadataValue},
};
use uuid::Uuid;

#[cfg(feature = "grpc-client")]
mod client;
#[cfg(feature = "grpc-server")]
mod server;

#[cfg(feature = "grpc-client")]
pub use client::GrpcTaxonomyTermNamesProvider;
#[cfg(feature = "grpc-server")]
pub use server::TaxonomyTermNamesGrpcService;

pub mod proto {
    tonic::include_proto!("rustok.taxonomy.v1");
}

const AUTHORIZATION: &str = "authorization";
const MAX_REQUEST_BYTES: usize = 16 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_CONTEXT_FIELD_BYTES: usize = 128;
const MAX_DEADLINE_MS: u64 = 30_000;
const MAX_TOKEN_BYTES: usize = 4096;

fn authorization(secret: &str) -> Result<MetadataValue<Ascii>, PortError> {
    if secret.is_empty()
        || secret.len() > MAX_TOKEN_BYTES
        || !secret.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(invalid_credential());
    }
    let mut metadata: MetadataValue<Ascii> = format!("Bearer {secret}")
        .parse()
        .map_err(|_| invalid_credential())?;
    metadata.set_sensitive(true);
    Ok(metadata)
}

fn invalid_credential() -> PortError {
    PortError::validation(
        "taxonomy.grpc_token_invalid",
        "Taxonomy gRPC credential is invalid",
    )
}

fn non_nil_uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value).ok().filter(|id| !id.is_nil())
}

fn validate_context(request: &proto::ReadTermNamesRequest) -> Result<(), PortError> {
    if [
        Some(request.correlation_id.as_str()),
        Some(request.locale.as_str()),
        request.fallback_locale.as_deref(),
        request.traceparent.as_deref(),
        request.causation_id.as_deref(),
        request.channel.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.len() > MAX_CONTEXT_FIELD_BYTES)
    {
        return Err(PortError::validation(
            "taxonomy.grpc_context_invalid",
            "Taxonomy gRPC context is invalid",
        ));
    }
    Ok(())
}

fn timeout(deadline_ms: u64) -> Result<Duration, PortError> {
    if deadline_ms == 0 {
        return Err(PortError::timeout(
            "port.deadline_required",
            "port calls require deadline semantics",
        ));
    }
    if deadline_ms > MAX_DEADLINE_MS {
        return Err(PortError::validation(
            "taxonomy.grpc_deadline_invalid",
            "Taxonomy gRPC deadline exceeds 30 seconds",
        ));
    }
    Ok(Duration::from_millis(deadline_ms))
}

fn status_code(kind: &PortErrorKind) -> Code {
    match kind {
        PortErrorKind::Validation => Code::InvalidArgument,
        PortErrorKind::NotFound => Code::NotFound,
        PortErrorKind::Conflict => Code::Aborted,
        PortErrorKind::Forbidden => Code::PermissionDenied,
        PortErrorKind::Unavailable => Code::Unavailable,
        PortErrorKind::Timeout => Code::DeadlineExceeded,
        PortErrorKind::InvariantViolation => Code::Internal,
    }
}
