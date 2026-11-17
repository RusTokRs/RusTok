/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::{net::IpAddr, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use tonic::{
    Code, Request, Status,
    metadata::{Ascii, MetadataValue},
    transport::{Channel, ClientTlsConfig, Endpoint},
};
use uuid::Uuid;

use super::{
    AUTHORIZATION, MAX_MESSAGE_BYTES, authorization,
    proto::{
        IdentitySelector, ReadIdentityRequest,
        channel_index_identity_service_client::ChannelIndexIdentityServiceClient,
    },
};
use crate::{
    ChannelIndexIdentityReadPort, ChannelIndexIdentitySelector, ChannelIndexIdentitySnapshot,
    MAX_CHANNEL_INDEX_IDENTITIES, channel_index_read_scope,
};

#[derive(Clone)]
pub struct GrpcChannelIndexIdentity {
    client: ChannelIndexIdentityServiceClient<Channel>,
    credential: MetadataValue<Ascii>,
}

impl GrpcChannelIndexIdentity {
    pub async fn connect(
        address: impl Into<String>,
        secret: &str,
        tls: Option<ClientTlsConfig>,
        allow_insecure_loopback: bool,
    ) -> Result<Self, PortError> {
        let credential = authorization(secret)?;
        let channel = endpoint(address.into(), tls, allow_insecure_loopback)?
            .connect()
            .await
            .map_err(|_| {
                PortError::unavailable(
                    "channel.index_identity_connect_failed",
                    "Unable to connect to Channel identity service",
                )
            })?;
        Ok(Self::from_channel(channel, credential))
    }

    /// Validates deployment configuration and lets the first call open the connection.
    pub fn connect_lazy(
        address: impl Into<String>,
        secret: &str,
        tls: Option<ClientTlsConfig>,
        allow_insecure_loopback: bool,
    ) -> Result<Self, PortError> {
        let credential = authorization(secret)?;
        let channel = endpoint(address.into(), tls, allow_insecure_loopback)?.connect_lazy();
        Ok(Self::from_channel(channel, credential))
    }

    fn from_channel(channel: Channel, credential: MetadataValue<Ascii>) -> Self {
        Self {
            client: ChannelIndexIdentityServiceClient::new(channel)
                .max_encoding_message_size(MAX_MESSAGE_BYTES)
                .max_decoding_message_size(MAX_MESSAGE_BYTES),
            credential,
        }
    }
}

fn endpoint(
    address: String,
    tls: Option<ClientTlsConfig>,
    allow_insecure_loopback: bool,
) -> Result<Endpoint, PortError> {
    let mut endpoint = Endpoint::from_shared(address)
        .map_err(|_| invalid_endpoint())?
        .connect_timeout(Duration::from_secs(5));
    match endpoint.uri().scheme_str() {
        Some("https") => {
            endpoint = endpoint
                .tls_config(tls.unwrap_or_else(|| ClientTlsConfig::new().with_webpki_roots()))
                .map_err(|_| invalid_endpoint())?;
        }
        Some("http")
            if tls.is_none()
                && allow_insecure_loopback
                && endpoint.uri().host().is_some_and(|host| {
                    host.trim_matches(['[', ']'])
                        .parse::<IpAddr>()
                        .is_ok_and(|address| address.is_loopback())
                }) => {}
        _ => return Err(invalid_endpoint()),
    }
    Ok(endpoint)
}

#[async_trait]
impl ChannelIndexIdentityReadPort for GrpcChannelIndexIdentity {
    async fn read_index_identity(
        &self,
        context: PortContext,
        selector: ChannelIndexIdentitySelector,
    ) -> Result<ChannelIndexIdentitySnapshot, PortError> {
        let (tenant_id, deadline) = channel_index_read_scope(&context)?;
        selector.validate()?;
        let (selector, slugs) = match selector {
            ChannelIndexIdentitySelector::GenerationOnly => {
                (IdentitySelector::GenerationOnly, Vec::new())
            }
            ChannelIndexIdentitySelector::All => (IdentitySelector::All, Vec::new()),
            ChannelIndexIdentitySelector::Slugs(slugs) => (IdentitySelector::Slugs, slugs),
        };
        let mut request = Request::new(ReadIdentityRequest {
            tenant_id: tenant_id.to_string(),
            selector: selector.into(),
            slugs,
            deadline_ms: context.deadline_ms.unwrap_or_default(),
            correlation_id: context.correlation_id,
            traceparent: context.traceparent,
        });
        request.set_timeout(deadline);
        request
            .metadata_mut()
            .insert(AUTHORIZATION, self.credential.clone());
        let response = self
            .client
            .clone()
            .read_identity(request)
            .await
            .map_err(port_error)?
            .into_inner();
        let channel_ids = response
            .channel_ids
            .iter()
            .map(|id| {
                id.parse::<Uuid>()
                    .ok()
                    .filter(|id| !id.is_nil())
                    .ok_or_else(invalid_snapshot)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if response.tenant_id != tenant_id.to_string()
            || response.generation > i64::MAX as u64
            || (!channel_ids.is_empty()
                && (response.generation == 0 || selector == IdentitySelector::GenerationOnly))
            || channel_ids.len() > MAX_CHANNEL_INDEX_IDENTITIES
            || channel_ids.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid_snapshot());
        }
        Ok(ChannelIndexIdentitySnapshot {
            tenant_id,
            generation: response.generation,
            channel_ids,
        })
    }
}

fn invalid_endpoint() -> PortError {
    PortError::validation(
        "channel.index_identity_endpoint_invalid",
        "Channel identity endpoint requires TLS or explicitly enabled loopback HTTP",
    )
}

fn invalid_snapshot() -> PortError {
    PortError::invariant_violation(
        "channel.index_identity_snapshot_invalid",
        "Invalid Channel identity snapshot",
    )
}

fn port_error(status: Status) -> PortError {
    let code = status.code();
    let reason = status.message().to_owned();
    match code {
        Code::Unauthenticated | Code::PermissionDenied => {
            PortError::forbidden("channel.index_identity_forbidden", reason)
        }
        Code::InvalidArgument => {
            PortError::validation("channel.index_identity_selector_invalid", reason)
        }
        Code::ResourceExhausted => PortError::validation("channel.index_identity_limit", reason),
        Code::DeadlineExceeded | Code::Cancelled => {
            PortError::timeout("channel.index_identity_timeout", reason)
        }
        Code::Unavailable => PortError::unavailable("channel.index_identity_unavailable", reason),
        _ => invalid_snapshot(),
    }
}
