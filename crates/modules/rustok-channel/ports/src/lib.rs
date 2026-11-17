/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::{collections::BTreeSet, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use uuid::Uuid;

#[cfg(any(feature = "grpc-client", feature = "grpc-server"))]
pub mod grpc;

pub const CHANNEL_INDEX_IDENTITY_READ_CLAIM: &str = "channel:index_identity:read";
pub const MAX_CHANNEL_INDEX_IDENTITIES: usize = 1024;
pub const MAX_CHANNEL_INDEX_SLUGS: usize = 1024;
pub const MAX_CHANNEL_INDEX_SLUG_BYTES: usize = 100;
pub const MAX_CHANNEL_INDEX_READ_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChannelIndexIdentitySelector {
    GenerationOnly,
    All,
    Slugs(Vec<String>),
}

impl ChannelIndexIdentitySelector {
    pub fn validate(&self) -> Result<(), PortError> {
        let Self::Slugs(slugs) = self else {
            return Ok(());
        };
        let mut seen = BTreeSet::new();
        if slugs.is_empty()
            || slugs.len() > MAX_CHANNEL_INDEX_SLUGS
            || slugs.iter().any(|slug| {
                slug.is_empty()
                    || slug.len() > MAX_CHANNEL_INDEX_SLUG_BYTES
                    || slug.trim() != slug
                    || slug.bytes().any(|byte| byte.is_ascii_uppercase())
                    || !seen.insert(slug)
            })
        {
            return Err(PortError::validation(
                "channel.index_identity_selector_invalid",
                "Channel identity slugs must be unique, lowercase and within the selector bounds",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelIndexIdentitySnapshot {
    pub tenant_id: Uuid,
    pub generation: u64,
    pub channel_ids: Vec<Uuid>,
}

pub fn channel_index_read_scope(context: &PortContext) -> Result<(Uuid, Duration), PortError> {
    context.require_read_semantics()?;
    let id = context
        .tenant_id
        .parse::<Uuid>()
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| {
            PortError::validation(
                "channel.index_identity_tenant_invalid",
                "Channel identity reads require a non-nil tenant UUID",
            )
        })?;
    let deadline = Duration::from_millis(context.deadline_ms.unwrap_or_default());
    if deadline > MAX_CHANNEL_INDEX_READ_DEADLINE {
        return Err(PortError::validation(
            "channel.index_identity_deadline_invalid",
            "Channel identity read deadline exceeds 30 seconds",
        ));
    }
    Ok((id, deadline))
}

/// Reads a tenant generation and matching identities as one owner observation.
/// Inactive Channels are identity members. The generation is zero before any identity history.
#[async_trait]
pub trait ChannelIndexIdentityReadPort: Send + Sync {
    async fn read_index_identity(
        &self,
        context: PortContext,
        selector: ChannelIndexIdentitySelector,
    ) -> Result<ChannelIndexIdentitySnapshot, PortError>;
}
