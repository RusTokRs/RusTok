use sea_orm_migration::prelude::*;

/// Platform-core table references used in commerce migration foreign keys.
#[derive(Iden)]
pub enum Tenants {
    Table,
    Id,
}

/// Content module table reference (for product image FK).
#[derive(Iden)]
pub enum Media {
    Table,
    Id,
}
