use std::collections::HashMap;

use async_graphql::dataloader::Loader;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::models::_entities::tenants;

/// Loader for Tenant names
#[derive(Clone)]
pub struct TenantNameLoader {
    db: DatabaseConnection,
}

impl TenantNameLoader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl Loader<Uuid> for TenantNameLoader {
    type Value = String;
    type Error = async_graphql::Error;

    fn load(
        &self,
        keys: &[Uuid],
    ) -> impl std::future::Future<Output = Result<HashMap<Uuid, Self::Value>, Self::Error>> + Send
    {
        let db = self.db.clone();
        let keys = keys.to_vec();

        async move {
            let tenants = tenants::Entity::find()
                .filter(tenants::Column::Id.is_in(keys))
                .all(&db)
                .await
                .map_err(|err| async_graphql::Error::new(err.to_string()))?;

            Ok(tenants
                .into_iter()
                .map(|tenant| (tenant.id, tenant.name))
                .collect())
        }
    }
}
