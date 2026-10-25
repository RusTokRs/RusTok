use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use rustok_core::UserStatus;

#[derive(Clone, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    #[serde(skip)]
    pub password_hash: String,
    pub name: Option<String>,
    pub status: UserStatus,
    pub email_verified_at: Option<DateTimeWithTimeZone>,
    pub last_login_at: Option<DateTimeWithTimeZone>,
    pub failed_login_count: i32,
    pub locked_until: Option<DateTimeWithTimeZone>,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tenants::Entity",
        from = "Column::TenantId",
        to = "super::tenants::Column::Id"
    )]
    Tenant,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Model")
            .field("id", &self.id)
            .field("tenant_id", &self.tenant_id)
            .field("email", &self.email)
            .field("password_hash", &"<redacted>")
            .field("name", &self.name)
            .field("status", &self.status)
            .field("email_verified_at", &self.email_verified_at)
            .field("last_login_at", &self.last_login_at)
            .field("failed_login_count", &self.failed_login_count)
            .field("locked_until", &self.locked_until)
            .field("metadata", &self.metadata)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

impl Related<super::tenants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tenant.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
