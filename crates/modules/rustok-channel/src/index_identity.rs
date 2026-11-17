/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortContext, PortError};
use rustok_channel_ports::{
    CHANNEL_INDEX_IDENTITY_READ_CLAIM, ChannelIndexIdentityReadPort, ChannelIndexIdentitySelector,
    ChannelIndexIdentitySnapshot, MAX_CHANNEL_INDEX_IDENTITIES, channel_index_read_scope,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value};
use uuid::Uuid;

use crate::index_projection::POSTGRES_IDENTITY_GENERATIONS;

#[derive(Clone, Debug)]
pub struct PostgresChannelIndexIdentity {
    db: DatabaseConnection,
}

impl PostgresChannelIndexIdentity {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Runs inside the caller's existing owner composition snapshot when embedded.
    pub async fn read_in(
        connection: &impl ConnectionTrait,
        tenant_id: Uuid,
        selector: ChannelIndexIdentitySelector,
    ) -> Result<ChannelIndexIdentitySnapshot, PortError> {
        selector.validate()?;
        if tenant_id.is_nil() {
            return Err(PortError::validation(
                "channel.index_identity_tenant_invalid",
                "Channel identity reads require a non-nil tenant UUID",
            ));
        }
        if connection.get_database_backend() != DbBackend::Postgres {
            return Err(PortError::invariant_violation(
                "channel.index_identity_backend_unsupported",
                "Channel identity snapshots require PostgreSQL",
            ));
        }
        let snapshot = connection
            .query_one_raw(snapshot_query(tenant_id, selector))
            .await
            .map_err(|error| {
                tracing::warn!(%error, "Channel identity snapshot read failed");
                PortError::unavailable(
                    "channel.index_identity_unavailable",
                    "Unable to read the Channel identity snapshot",
                )
            })?
            .ok_or_else(invalid_snapshot)?;
        if snapshot
            .try_get::<bool>("", "is_standby")
            .map_err(|_| invalid_snapshot())?
        {
            return Err(PortError::unavailable(
                "channel.index_identity_primary_required",
                "Channel identity snapshots require the primary database",
            ));
        }
        let generation = snapshot
            .try_get::<Option<i64>>("", "generation")
            .map_err(|_| invalid_snapshot())?;
        let unversioned = snapshot
            .try_get::<bool>("", "has_unversioned_identity")
            .map_err(|_| invalid_snapshot())?;
        let generation = match (generation, unversioned) {
            (Some(value), false) if value > 0 => {
                u64::try_from(value).map_err(|_| invalid_snapshot())?
            }
            (None, false) => 0,
            _ => return Err(invalid_snapshot()),
        };
        let channel_ids = snapshot
            .try_get::<Vec<Uuid>>("", "channel_ids")
            .map_err(|_| invalid_snapshot())?;
        if channel_ids.len() > MAX_CHANNEL_INDEX_IDENTITIES {
            return Err(PortError::validation(
                "channel.index_identity_limit",
                "Channel identity snapshot exceeds the maximum number of identities",
            ));
        }
        if channel_ids.iter().any(Uuid::is_nil)
            || channel_ids.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid_snapshot());
        }
        Ok(ChannelIndexIdentitySnapshot {
            tenant_id,
            generation,
            channel_ids,
        })
    }
}

#[async_trait]
impl ChannelIndexIdentityReadPort for PostgresChannelIndexIdentity {
    async fn read_index_identity(
        &self,
        context: PortContext,
        selector: ChannelIndexIdentitySelector,
    ) -> Result<ChannelIndexIdentitySnapshot, PortError> {
        let (tenant_id, deadline) = channel_index_read_scope(&context)?;
        if context.actor.kind != PortActorKind::Service
            || !context
                .claims
                .iter()
                .any(|claim| claim == CHANNEL_INDEX_IDENTITY_READ_CLAIM)
        {
            return Err(PortError::forbidden(
                "channel.index_identity_forbidden",
                "Channel identity snapshots require the dedicated service grant",
            ));
        }
        tokio::time::timeout(deadline, Self::read_in(&self.db, tenant_id, selector))
            .await
            .map_err(|_| {
                PortError::timeout(
                    "channel.index_identity_timeout",
                    "Channel identity snapshot timed out",
                )
            })?
    }
}

fn snapshot_query(tenant_id: Uuid, selector: ChannelIndexIdentitySelector) -> Statement {
    let mut values = vec![tenant_id.into()];
    let fetch_limit = (MAX_CHANNEL_INDEX_IDENTITIES + 1) as i64;
    let ids = match selector {
        ChannelIndexIdentitySelector::GenerationOnly => "ARRAY[]::uuid[]",
        ChannelIndexIdentitySelector::All => {
            values.push(Value::from(fetch_limit));
            "ARRAY(SELECT id FROM channels WHERE tenant_id = $1 ORDER BY id LIMIT $2)"
        }
        ChannelIndexIdentitySelector::Slugs(slugs) => {
            values.push(Value::from(slugs));
            values.push(Value::from(fetch_limit));
            "ARRAY(SELECT id FROM channels WHERE tenant_id = $1
                AND lower(btrim(slug)) = ANY($2::text[]) ORDER BY id LIMIT $3)"
        }
    };
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "SELECT version.generation, {ids} AS channel_ids, pg_is_in_recovery() AS is_standby,
                 version.generation IS NULL AND EXISTS(
                     SELECT 1 FROM channels WHERE tenant_id = $1
                 ) AS has_unversioned_identity
             FROM (SELECT (SELECT generation FROM {POSTGRES_IDENTITY_GENERATIONS} generations
                           WHERE tenant_id = $1) AS generation) version"
        ),
        values,
    )
}

fn invalid_snapshot() -> PortError {
    PortError::invariant_violation(
        "channel.index_identity_snapshot_invalid",
        "Invalid Channel identity snapshot",
    )
}
