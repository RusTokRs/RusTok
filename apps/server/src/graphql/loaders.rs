use std::collections::HashMap;

use async_graphql::dataloader::Loader;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::models::_entities::tenants;
#[cfg(feature = "mod-content")]
use rustok_content::entities::{self, body, node, node_translation};

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

/// Loader for Node entities - prevents N+1 queries when loading multiple nodes
#[cfg(feature = "mod-content")]
#[derive(Clone)]
pub struct NodeLoader {
    db: DatabaseConnection,
}

#[cfg(feature = "mod-content")]
impl NodeLoader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[cfg(feature = "mod-content")]
impl Loader<Uuid> for NodeLoader {
    type Value = node::Model;
    type Error = async_graphql::Error;

    fn load(
        &self,
        keys: &[Uuid],
    ) -> impl std::future::Future<Output = Result<HashMap<Uuid, Self::Value>, Self::Error>> + Send
    {
        let db = self.db.clone();
        let keys = keys.to_vec();

        async move {
            let nodes = entities::Node::find()
                .filter(node::Column::Id.is_in(keys))
                .all(&db)
                .await
                .map_err(|err| async_graphql::Error::new(err.to_string()))?;

            Ok(nodes.into_iter().map(|node| (node.id, node)).collect())
        }
    }
}

/// Loader for NodeTranslation entities - batches translation queries
#[cfg(feature = "mod-content")]
#[derive(Clone)]
pub struct NodeTranslationLoader {
    db: DatabaseConnection,
}

#[cfg(feature = "mod-content")]
impl NodeTranslationLoader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[cfg(feature = "mod-content")]
impl Loader<Uuid> for NodeTranslationLoader {
    type Value = Vec<node_translation::Model>;
    type Error = async_graphql::Error;

    fn load(
        &self,
        keys: &[Uuid],
    ) -> impl std::future::Future<Output = Result<HashMap<Uuid, Self::Value>, Self::Error>> + Send
    {
        let db = self.db.clone();
        let keys = keys.to_vec();

        async move {
            let translations = entities::NodeTranslation::find()
                .filter(node_translation::Column::NodeId.is_in(keys.clone()))
                .all(&db)
                .await
                .map_err(|err| async_graphql::Error::new(err.to_string()))?;

            let mut result: HashMap<Uuid, Vec<node_translation::Model>> = HashMap::new();

            for translation in translations {
                result
                    .entry(translation.node_id)
                    .or_default()
                    .push(translation);
            }

            // Ensure all keys have entries (even if empty)
            for key in keys {
                result.entry(key).or_default();
            }

            Ok(result)
        }
    }
}

/// Loader for NodeBody entities - batches body queries
#[cfg(feature = "mod-content")]
#[derive(Clone)]
pub struct NodeBodyLoader {
    db: DatabaseConnection,
}

#[cfg(feature = "mod-content")]
impl NodeBodyLoader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[cfg(feature = "mod-content")]
impl Loader<Uuid> for NodeBodyLoader {
    type Value = Vec<body::Model>;
    type Error = async_graphql::Error;

    fn load(
        &self,
        keys: &[Uuid],
    ) -> impl std::future::Future<Output = Result<HashMap<Uuid, Self::Value>, Self::Error>> + Send
    {
        let db = self.db.clone();
        let keys = keys.to_vec();

        async move {
            let bodies = entities::Body::find()
                .filter(body::Column::NodeId.is_in(keys.clone()))
                .all(&db)
                .await
                .map_err(|err| async_graphql::Error::new(err.to_string()))?;

            let mut result: HashMap<Uuid, Vec<body::Model>> = HashMap::new();

            for body in bodies {
                result.entry(body.node_id).or_default().push(body);
            }

            // Ensure all keys have entries (even if empty)
            for key in keys {
                result.entry(key).or_default();
            }

            Ok(result)
        }
    }
}
