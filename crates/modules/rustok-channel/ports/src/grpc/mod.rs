/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_api::PortError;
use tonic::metadata::{Ascii, MetadataValue};

#[cfg(feature = "grpc-client")]
mod client;
#[cfg(feature = "grpc-server")]
mod server;

#[cfg(feature = "grpc-client")]
pub use client::GrpcChannelIndexIdentity;
#[cfg(feature = "grpc-server")]
pub use server::ChannelIndexIdentityGrpcService;

mod proto {
    tonic::include_proto!("rustok.channel.v1");
}

const AUTHORIZATION: &str = "authorization";
const MAX_MESSAGE_BYTES: usize = 256 * 1024;

fn authorization(secret: &str) -> Result<MetadataValue<Ascii>, PortError> {
    if secret.is_empty()
        || secret.len() > 4096
        || !secret.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(invalid_secret());
    }
    let mut header: MetadataValue<Ascii> = format!("Bearer {secret}")
        .parse()
        .map_err(|_| invalid_secret())?;
    header.set_sensitive(true);
    Ok(header)
}

fn invalid_secret() -> PortError {
    PortError::validation(
        "channel.index_identity_token_invalid",
        "Invalid Channel identity service credential",
    )
}
