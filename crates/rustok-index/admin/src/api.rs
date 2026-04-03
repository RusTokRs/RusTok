use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::IndexAdminBootstrap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub async fn fetch_bootstrap() -> Result<IndexAdminBootstrap, ApiError> {
    index_bootstrap_native().await.map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "index/bootstrap")]
async fn index_bootstrap_native() -> Result<IndexAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_core::RusToKModule;
        use sea_orm::ConnectionTrait;

        let app_ctx = expect_context::<AppContext>();
        let _auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        let db = app_ctx.db.clone();
        let backend = db.get_database_backend();

        let module = rustok_index::IndexModule;
        Ok(IndexAdminBootstrap {
            tenant: IndexTenantSnapshot {
                id: tenant.id.to_string(),
                slug: tenant.slug,
                name: tenant.name,
                default_locale: tenant.default_locale,
            },
            module: IndexModuleSnapshot {
                slug: module.slug().to_string(),
                name: module.name().to_string(),
                description: module.description().to_string(),
                supports_postgres_fts: matches!(backend, sea_orm::DbBackend::Postgres),
                document_types: vec![
                    "node".to_string(),
                    "product".to_string(),
                    "category".to_string(),
                ],
            },
            counters: vec![
                IndexCounterSnapshot {
                    key: "content".to_string(),
                    label: "Content index rows".to_string(),
                    value: query_scalar_i64(
                        &db,
                        backend,
                        "SELECT COUNT(*) AS value FROM index_content WHERE tenant_id = $1",
                        tenant.id,
                    )
                    .await
                    .unwrap_or_default() as u64,
                },
                IndexCounterSnapshot {
                    key: "products".to_string(),
                    label: "Product index rows".to_string(),
                    value: query_scalar_i64(
                        &db,
                        backend,
                        "SELECT COUNT(*) AS value FROM index_products WHERE tenant_id = $1",
                        tenant.id,
                    )
                    .await
                    .unwrap_or_default() as u64,
                },
                IndexCounterSnapshot {
                    key: "search".to_string(),
                    label: "Search index rows".to_string(),
                    value: query_scalar_i64(
                        &db,
                        backend,
                        "SELECT COUNT(*) AS value FROM search_index WHERE tenant_id = $1",
                        tenant.id,
                    )
                    .await
                    .unwrap_or_default() as u64,
                },
            ],
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "rustok-index-admin requires the `ssr` feature for native bootstrap",
        ))
    }
}

#[cfg(feature = "ssr")]
async fn query_scalar_i64(
    db: &sea_orm::DatabaseConnection,
    backend: sea_orm::DbBackend,
    sql: &str,
    tenant_id: uuid::Uuid,
) -> Result<i64, ServerFnError> {
    use sea_orm::{ConnectionTrait, QueryResult, Statement};

    let row = db
        .query_one(Statement::from_sql_and_values(
            backend,
            sql,
            [tenant_id.into()],
        ))
        .await
        .map_err(ServerFnError::new)?;
    Ok(row
        .and_then(|row: QueryResult| row.try_get::<i64>("", "value").ok())
        .unwrap_or_default())
}
