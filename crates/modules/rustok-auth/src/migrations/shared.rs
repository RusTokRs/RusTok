use sea_orm_migration::prelude::*;

/// Platform-core table references used in auth migration foreign keys.
#[derive(Iden)]
pub enum Tenants {
    Table,
    Id,
}

#[derive(Iden)]
pub enum Users {
    Table,
    Id,
}
