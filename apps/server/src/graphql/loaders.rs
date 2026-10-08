use std::collections::HashMap;
use std::fmt::Display;

use async_graphql::dataloader::Loader;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use rustok_tenant::entities::tenant as tenants;

/// Loader for Tenant names.
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
        let key_count = keys.len();
        let keys = keys.to_vec();

        async move {
            let tenants = tenants::Entity::find()
                .filter(tenants::Column::Id.is_in(keys))
                .all(&db)
                .await
                .map_err(|error| tenant_name_loader_error(error, key_count))?;

            Ok(tenants
                .into_iter()
                .map(|tenant| (tenant.id, tenant.name))
                .collect())
        }
    }
}

fn tenant_name_loader_error(error: impl Display, key_count: usize) -> async_graphql::Error {
    tracing::error!(
        %error,
        key_count,
        "Tenant name GraphQL loader database lookup failed"
    );
    async_graphql::Error::new("Tenant name lookup failed")
}

#[cfg(test)]
mod tests {
    use super::tenant_name_loader_error;

    #[test]
    fn tenant_name_loader_error_redacts_backend_diagnostics() {
        let error = tenant_name_loader_error(
            "database password=secret table=tenants query=SELECT * FROM tenants",
            3,
        );

        assert_eq!(error.message, "Tenant name lookup failed");
        assert!(
            !error
                .message
                .contains("database password=secret table=tenants")
        );
    }
}
