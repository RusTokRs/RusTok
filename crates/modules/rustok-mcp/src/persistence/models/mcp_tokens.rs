use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "mcp_tokens")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub client_id: Uuid,
    pub token_name: String,
    pub token_preview: String,
    pub token_hash: String,
    pub created_by: Option<Uuid>,
    pub last_used_at: Option<DateTimeWithTimeZone>,
    pub expires_at: Option<DateTimeWithTimeZone>,
    pub revoked_at: Option<DateTimeWithTimeZone>,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::mcp_clients::Entity",
        from = "Column::ClientId",
        to = "super::mcp_clients::Column::Id"
    )]
    McpClient,
}

impl Related<super::mcp_clients::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::McpClient.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

use chrono::Utc;
use sea_orm::{Condition, QueryFilter, QueryOrder};

impl Entity {
    pub async fn find_active_by_hash(
        db: &DatabaseConnection,
        token_hash: &str,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
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
            .await
    }

    pub async fn find_by_client_scoped(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        client_id: Uuid,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::TenantId.eq(tenant_id))
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
