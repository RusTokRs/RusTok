use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum Tenants {
    Table,
    Id,
}

#[derive(Iden)]
pub enum OAuthApps {
    #[iden = "oauth_apps"]
    Table,
    Id,
}
