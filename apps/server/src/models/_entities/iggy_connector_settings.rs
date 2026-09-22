use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "iggy_connector_settings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub mode: String,
    pub external_addresses: Json,
    pub external_username: String,
    pub password_resolver: Option<String>,
    pub password_key: Option<String>,
    pub secret_tenant_id: Option<Uuid>,
    pub tls_enabled: bool,
    pub tls_domain: Option<String>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
