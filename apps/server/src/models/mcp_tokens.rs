use std::str::FromStr;

use chrono::Utc;
use rustok_api::{Permission, has_effective_permission};
use rustok_mcp::McpActorType;
use sea_orm::{Condition, QueryFilter, QueryOrder, entity::prelude::*};

use crate::models::{mcp_clients, mcp_policies, users};
use crate::services::rbac_service::RbacService;

pub use super::_entities::mcp_tokens::{ActiveModel, Column, Entity, Model, Relation};

impl Entity {
    pub async fn find_active_by_hash(
        db: &DatabaseConnection,
        token_hash: &str,
    ) -> Result<Option<Model>, DbErr> {
        let token = Self::find()
            .filter(
                Condition::all()
                    .add(Column::TokenHash.eq(token_hash))
                    .add(Column::RevokedAt.is_null())
                    .add(
                        Condition::any()
                            .add(Column::ExpiresAt.is_null())
                            .add(Column::ExpiresAt.gt(Utc::now())),
                    ),
            )
            .one(db)
            .await?;
        let Some(token) = token else {
            return Ok(None);
        };

        let client = mcp_clients::Entity::find_by_id(token.client_id)
            .filter(mcp_clients::Column::TenantId.eq(token.tenant_id))
            .one(db)
            .await?;
        let Some(client) = client.filter(mcp_clients::Model::is_active) else {
            return Ok(None);
        };

        if client.actor_type() == McpActorType::HumanUser && client.delegated_user_id.is_none() {
            return Ok(None);
        }

        let policy = mcp_policies::Entity::find_by_client(db, client.id).await?;
        if policy
            .as_ref()
            .is_some_and(|policy| policy.tenant_id != client.tenant_id)
        {
            return Ok(None);
        }

        if let Some(delegated_user_id) = client.delegated_user_id {
            let delegated_user = users::Entity::find_by_id(delegated_user_id)
                .filter(users::Column::TenantId.eq(client.tenant_id))
                .one(db)
                .await?;
            let Some(delegated_user) = delegated_user.filter(users::Model::is_active) else {
                return Ok(None);
            };

            if let Some(policy) = policy {
                let authoritative = RbacService::get_user_permissions_authoritative(
                    db,
                    &client.tenant_id,
                    &delegated_user.id,
                )
                .await
                .map_err(|error| DbErr::Custom(error.to_string()))?;

                for raw in policy.granted_permissions_list() {
                    let Ok(permission) = Permission::from_str(raw.trim()) else {
                        return Ok(None);
                    };
                    if !has_effective_permission(&authoritative, &permission) {
                        return Ok(None);
                    }
                }
            }
        }

        Ok(Some(token))
    }

    pub async fn find_by_client(
        db: &DatabaseConnection,
        client_id: Uuid,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::ClientId.eq(client_id))
            .order_by_desc(Column::CreatedAt)
            .all(db)
            .await
    }
}

impl Model {
    pub fn is_active(&self) -> bool {
        let not_revoked = self.revoked_at.is_none();
        let not_expired = self
            .expires_at
            .map(|expires_at| {
                let expires_at_utc: chrono::DateTime<Utc> = expires_at.into();
                expires_at_utc > Utc::now()
            })
            .unwrap_or(true);
        not_revoked && not_expired
    }
}

#[cfg(test)]
mod tests {
    use rustok_api::{Permission, has_effective_permission};

    #[test]
    fn manage_permission_can_back_a_narrow_delegated_grant() {
        assert!(has_effective_permission(
            &[Permission::PAGES_MANAGE],
            &Permission::PAGES_READ,
        ));
        assert!(!has_effective_permission(
            &[Permission::PAGES_READ],
            &Permission::PAGES_MANAGE,
        ));
    }
}
