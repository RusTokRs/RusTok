use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tenant_modules")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub enabled: bool,
    pub settings: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tenant::Entity",
        from = "Column::TenantId",
        to = "super::tenant::Column::Id"
    )]
    Tenant,
}

impl Related<super::tenant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tenant.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    pub async fn is_enabled(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<bool, DbErr> {
        let result = Self::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::ModuleSlug.eq(module_slug))
            .filter(Column::Enabled.eq(true))
            .one(db)
            .await?;

        Ok(result.is_some())
    }
}
