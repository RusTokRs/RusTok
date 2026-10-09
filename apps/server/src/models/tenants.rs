use sea_orm::EntityTrait;
use sea_orm::prelude::*;

pub use rustok_tenant::entities::tenant::{self, ActiveModel, Column, Entity, Model, Relation};

pub fn new_tenant_active_model(name: &str, slug: &str) -> ActiveModel {
    ActiveModel::new(name, slug)
}

pub trait TenantActiveModelExt {
    fn new(name: &str, slug: &str) -> Self;
}

impl TenantActiveModelExt for ActiveModel {
    fn new(name: &str, slug: &str) -> Self {
        ActiveModel::new(name, slug)
    }
}

pub async fn find_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<Model>, DbErr> {
    <Entity as EntityTrait>::find_by_id(id).one(db).await
}

pub async fn find_by_slug(db: &DatabaseConnection, slug: &str) -> Result<Option<Model>, DbErr> {
    Entity::find().filter(Column::Slug.eq(slug)).one(db).await
}

pub async fn find_by_domain(db: &DatabaseConnection, domain: &str) -> Result<Option<Model>, DbErr> {
    Entity::find()
        .filter(Column::Domain.eq(domain))
        .one(db)
        .await
}

pub async fn find_active(db: &DatabaseConnection) -> Result<Vec<Model>, DbErr> {
    Entity::find()
        .filter(Column::IsActive.eq(true))
        .all(db)
        .await
}

pub async fn find_or_create(
    db: &DatabaseConnection,
    name: &str,
    slug: &str,
    domain: Option<&str>,
) -> Result<Model, DbErr> {
    if let Some(existing) = find_by_slug(db, slug).await? {
        return Ok(existing);
    }

    let mut tenant = new_tenant_active_model(name, slug);
    tenant.domain = sea_orm::ActiveValue::Set(domain.map(|value| value.to_string()));
    match tenant.insert(db).await {
        Ok(created) => Ok(created),
        Err(err) => {
            if let Some(existing) = find_by_slug(db, slug).await? {
                Ok(existing)
            } else {
                Err(err)
            }
        }
    }
}

pub trait TenantEntityExt {
    fn find_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> impl std::future::Future<Output = Result<Option<Model>, DbErr>> + Send;

    fn find_by_slug<'a>(
        db: &'a DatabaseConnection,
        slug: &'a str,
    ) -> impl std::future::Future<Output = Result<Option<Model>, DbErr>> + Send + 'a;

    fn find_by_domain<'a>(
        db: &'a DatabaseConnection,
        domain: &'a str,
    ) -> impl std::future::Future<Output = Result<Option<Model>, DbErr>> + Send + 'a;

    fn find_active(
        db: &DatabaseConnection,
    ) -> impl std::future::Future<Output = Result<Vec<Model>, DbErr>> + Send;

    fn find_or_create<'a>(
        db: &'a DatabaseConnection,
        name: &'a str,
        slug: &'a str,
        domain: Option<&'a str>,
    ) -> impl std::future::Future<Output = Result<Model, DbErr>> + Send + 'a;
}

impl TenantEntityExt for Entity {
    fn find_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> impl std::future::Future<Output = Result<Option<Model>, DbErr>> + Send {
        find_by_id(db, id)
    }

    fn find_by_slug<'a>(
        db: &'a DatabaseConnection,
        slug: &'a str,
    ) -> impl std::future::Future<Output = Result<Option<Model>, DbErr>> + Send + 'a {
        find_by_slug(db, slug)
    }

    fn find_by_domain<'a>(
        db: &'a DatabaseConnection,
        domain: &'a str,
    ) -> impl std::future::Future<Output = Result<Option<Model>, DbErr>> + Send + 'a {
        find_by_domain(db, domain)
    }

    fn find_active(
        db: &DatabaseConnection,
    ) -> impl std::future::Future<Output = Result<Vec<Model>, DbErr>> + Send {
        find_active(db)
    }

    fn find_or_create<'a>(
        db: &'a DatabaseConnection,
        name: &'a str,
        slug: &'a str,
        domain: Option<&'a str>,
    ) -> impl std::future::Future<Output = Result<Model, DbErr>> + Send + 'a {
        find_or_create(db, name, slug, domain)
    }
}
