//! Inventory-owned source state. No foreign-key cascade may erase retained revisions/deletes.

pub(super) mod baseline {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "inventory_source_baselines")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub tenant_id: Uuid,
        pub baseline_id: Uuid,
        pub actor_id: Uuid,
        pub active: bool,
        pub last_variant_id: Option<Uuid>,
        pub record_count: i64,
        pub last_page_id: Option<Uuid>,
        pub last_page_after: Option<Uuid>,
        pub last_page_limit: Option<i32>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

pub(super) mod record {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "inventory_source_records")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub tenant_id: Uuid,
        #[sea_orm(primary_key, auto_increment = false)]
        pub variant_id: Uuid,
        pub baseline_id: Uuid,
        pub inventory_item_id: Uuid,
        pub product_id: Uuid,
        pub source_version: i64,
        pub deleted: bool,
        pub stocked: i64,
        pub reserved: i64,
        pub incoming: i64,
        pub binding_digest: String,
        pub last_change_id: Uuid,
        pub updated_at: DateTimeWithTimeZone,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

pub(super) mod change {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "inventory_source_changes")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub change_id: Uuid,
        pub tenant_id: Uuid,
        pub variant_id: Uuid,
        pub baseline_id: Uuid,
        pub source_version: i64,
        pub kind: String,
        pub created_at: DateTimeWithTimeZone,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

pub(super) mod page {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "inventory_source_baseline_pages")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub tenant_id: Uuid,
        #[sea_orm(primary_key, auto_increment = false)]
        pub page_id: Uuid,
        pub baseline_id: Uuid,
        pub after_variant_id: Option<Uuid>,
        pub page_limit: i32,
        pub next_after: Option<Uuid>,
        pub active: bool,
        pub record_count: i64,
        pub created_at: DateTimeWithTimeZone,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
