use sea_orm_migration::prelude::*;

/// Platform-core table references used in commerce migration foreign keys.
#[derive(Iden)]
pub enum Tenants {
    Table,
    Id,
}
