/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::{collections::HashSet, sync::Arc};

use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use super::{
    AUTHORIZATION, MAX_MESSAGE_BYTES, authorization,
    proto::{
        IdentitySelector, ReadIdentityRequest, ReadIdentityResponse,
        channel_index_identity_service_server::{
            ChannelIndexIdentityService, ChannelIndexIdentityServiceServer,
        },
    },
};
use crate::{
    CHANNEL_INDEX_IDENTITY_READ_CLAIM, ChannelIndexIdentityReadPort, ChannelIndexIdentitySelector,
};

pub struct ChannelIndexIdentityGrpcService {
    owner: Arc<dyn ChannelIndexIdentityReadPort>,
    credential_digest: [u8; 32],
    tenants: HashSet<Uuid>,
}

impl ChannelIndexIdentityGrpcService {
    pub fn new(
        owner: Arc<dyn ChannelIndexIdentityReadPort>,
        secret: &str,
        tenants: impl IntoIterator<Item = Uuid>,
    ) -> Result<Self, PortError> {
        let credential_digest = Sha256::digest(authorization(secret)?.as_encoded_bytes()).into();
        let mut allowed = HashSet::new();
        for tenant in tenants {
            if tenant.is_nil() || allowed.len() >= 128 || !allowed.insert(tenant) {
                return Err(invalid_tenants());
            }
        }
        if allowed.is_empty() {
            return Err(invalid_tenants());
        }
        Ok(Self {
            owner,
            credential_digest,
            tenants: allowed,
        })
    }

    pub fn into_server(self) -> ChannelIndexIdentityServiceServer<Self> {
        ChannelIndexIdentityServiceServer::new(self)
            .max_encoding_message_size(MAX_MESSAGE_BYTES)
            .max_decoding_message_size(MAX_MESSAGE_BYTES)
    }
}

#[tonic::async_trait]
impl ChannelIndexIdentityService for ChannelIndexIdentityGrpcService {
    async fn read_identity(
        &self,
        request: Request<ReadIdentityRequest>,
    ) -> Result<Response<ReadIdentityResponse>, Status> {
        let secret = request
            .metadata()
            .get(AUTHORIZATION)
            .filter(|value| value.as_encoded_bytes().len() <= 4103)
            .ok_or_else(unauthenticated)?;
        let digest: [u8; 32] = Sha256::digest(secret.as_encoded_bytes()).into();
        if !bool::from(digest.ct_eq(&self.credential_digest)) {
            return Err(unauthenticated());
        }
        let input = request.into_inner();
        let tenant_id = input
            .tenant_id
            .parse::<Uuid>()
            .map_err(|_| Status::invalid_argument("Channel identity requires a tenant UUID"))?;
        if !self.tenants.contains(&tenant_id) {
            return Err(Status::permission_denied(
                "Channel identity tenant is not authorized",
            ));
        }
        let selector = match IdentitySelector::try_from(input.selector) {
            Ok(IdentitySelector::GenerationOnly) if input.slugs.is_empty() => {
                ChannelIndexIdentitySelector::GenerationOnly
            }
            Ok(IdentitySelector::All) if input.slugs.is_empty() => {
                ChannelIndexIdentitySelector::All
            }
            Ok(IdentitySelector::Slugs) => ChannelIndexIdentitySelector::Slugs(input.slugs),
            _ => {
                return Err(Status::invalid_argument(
                    "Invalid Channel identity selector",
                ));
            }
        };
        let mut context = PortContext::new(
            tenant_id.to_string(),
            PortActor::service("channel-index-identity"),
            "und",
            input.correlation_id,
        )
        .with_claim(CHANNEL_INDEX_IDENTITY_READ_CLAIM);
        context.deadline_ms = Some(input.deadline_ms);
        context.traceparent = input.traceparent;
        let result = self
            .owner
            .read_index_identity(context, selector)
            .await
            .map_err(to_status)?;
        Ok(Response::new(ReadIdentityResponse {
            tenant_id: result.tenant_id.to_string(),
            generation: result.generation,
            channel_ids: result.channel_ids.iter().map(Uuid::to_string).collect(),
        }))
    }
}

fn unauthenticated() -> Status {
    Status::unauthenticated("Channel identity authentication failed")
}

fn invalid_tenants() -> PortError {
    PortError::validation(
        "channel.index_identity_tenants_invalid",
        "Channel identity service requires one to 128 distinct tenant UUIDs",
    )
}

fn to_status(error: PortError) -> Status {
    match error.kind {
        PortErrorKind::Validation if error.code == "channel.index_identity_limit" => {
            Status::resource_exhausted(error.message)
        }
        PortErrorKind::Validation => Status::invalid_argument(error.message),
        PortErrorKind::Forbidden => Status::permission_denied(error.message),
        PortErrorKind::Timeout => Status::deadline_exceeded(error.message),
        PortErrorKind::Unavailable => Status::unavailable(error.message),
        _ => Status::internal("Invalid Channel identity snapshot"),
    }
}
