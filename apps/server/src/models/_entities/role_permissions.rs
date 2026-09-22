use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "role_permissions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub role_id: Uuid,
    pub permission_id: Uuid,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::roles::Entity",
        from = "Column::RoleId",
        to = "super::roles::Column::Id"
    )]
    Role,
    #[sea_orm(
        belongs_to = "super::permissions::Entity",
        from = "Column::PermissionId",
        to = "super::permissions::Column::Id"
    )]
    Permission,
}

impl Related<super::roles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Role.def()
    }
}

impl Related<super::permissions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Permission.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
