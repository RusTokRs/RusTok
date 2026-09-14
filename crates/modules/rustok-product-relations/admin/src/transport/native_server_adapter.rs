use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::model::{
    ProductRelationItem, ProductRelationsAdminCommand, ProductRelationsAdminCommandResult,
    ProductRelationsAdminFilters,
};

#[derive(Debug, Clone)]
pub struct NativeProductRelationsAdminError(pub String);

impl Display for NativeProductRelationsAdminError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeProductRelationsAdminError {}

impl From<ServerFnError> for NativeProductRelationsAdminError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_relations(
    filters: ProductRelationsAdminFilters,
) -> Result<Vec<ProductRelationItem>, NativeProductRelationsAdminError> {
    product_relations_list_native(filters).await.map_err(Into::into)
}

pub async fn execute_command(
    idempotency_key: String,
    command: ProductRelationsAdminCommand,
) -> Result<ProductRelationsAdminCommandResult, NativeProductRelationsAdminError> {
    product_relations_command_native(idempotency_key, command)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "product_relations/list")]
async fn product_relations_list_native(
    filters: ProductRelationsAdminFilters,
) -> Result<Vec<ProductRelationItem>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
        use rustok_product_relations::{
            ProductRelationService, ProductRelationsPort, dto::RelationType,
        };
        use std::str::FromStr;

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_tenant(&auth, &tenant)?;

        let product_id_str = filters
            .product_id
            .ok_or_else(|| ServerFnError::new("product_id is required"))?;
        let product_id = parse_uuid(&product_id_str, "product_id")?;

        let rtype = filters
            .relation_type
            .as_deref()
            .and_then(|s| RelationType::from_str(s).ok());

        let service = ProductRelationService::new(runtime.db_clone());
        let list = service
            .list_relations(tenant.id, product_id, rtype)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        Ok(list
            .into_iter()
            .map(|dto| ProductRelationItem {
                id: dto.id.to_string(),
                product_id: dto.product_id.to_string(),
                related_product_id: dto.related_product_id.to_string(),
                relation_type: dto.relation_type.as_str().to_string(),
                position: dto.position,
                metadata: dto.metadata,
                created_at: dto.created_at.to_rfc3339(),
                updated_at: dto.updated_at.to_rfc3339(),
            })
            .collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = filters;
        Err(ServerFnError::new(
            "product_relations list requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product_relations/command")]
async fn product_relations_command_native(
    idempotency_key: String,
    command: ProductRelationsAdminCommand,
) -> Result<ProductRelationsAdminCommandResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
        use rustok_product_relations::{
            ProductRelationService, ProductRelationsPort,
            dto::{CreateProductRelationInput, RelationType, ReorderProductRelationsInput},
        };
        use std::str::FromStr;

        let _ = idempotency_key;
        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_tenant(&auth, &tenant)?;

        let service = ProductRelationService::new(runtime.db_clone());

        match command {
            ProductRelationsAdminCommand::Add { draft } => {
                let p_id = parse_uuid(&draft.product_id, "product_id")?;
                let rel_p_id = parse_uuid(&draft.related_product_id, "related_product_id")?;
                let rtype = RelationType::from_str(&draft.relation_type)
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                let created = service
                    .create_relation(
                        tenant.id,
                        Some(auth.user_id),
                        CreateProductRelationInput {
                            product_id: p_id,
                            related_product_id: rel_p_id,
                            relation_type: rtype,
                            position: draft.position,
                            metadata: draft.metadata,
                        },
                    )
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                Ok(ProductRelationsAdminCommandResult {
                    item: Some(ProductRelationItem {
                        id: created.id.to_string(),
                        product_id: created.product_id.to_string(),
                        related_product_id: created.related_product_id.to_string(),
                        relation_type: created.relation_type.as_str().to_string(),
                        position: created.position,
                        metadata: created.metadata,
                        created_at: created.created_at.to_rfc3339(),
                        updated_at: created.updated_at.to_rfc3339(),
                    }),
                    items: vec![],
                    success: true,
                })
            }
            ProductRelationsAdminCommand::Remove { id } => {
                let rel_id = parse_uuid(&id, "relation_id")?;
                service
                    .delete_relation(tenant.id, Some(auth.user_id), rel_id)
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                Ok(ProductRelationsAdminCommandResult {
                    item: None,
                    items: vec![],
                    success: true,
                })
            }
            ProductRelationsAdminCommand::Reorder {
                product_id,
                relation_type,
                ordered_ids,
            } => {
                let p_id = parse_uuid(&product_id, "product_id")?;
                let rtype = RelationType::from_str(&relation_type)
                    .map_err(|e| ServerFnError::new(e.to_string()))?;
                let o_ids = ordered_ids
                    .into_iter()
                    .map(|id| parse_uuid(&id, "ordered_id"))
                    .collect::<Result<Vec<_>, _>>()?;

                let reordered = service
                    .reorder_relations(
                        tenant.id,
                        Some(auth.user_id),
                        ReorderProductRelationsInput {
                            product_id: p_id,
                            relation_type: rtype,
                            ordered_relation_ids: o_ids,
                        },
                    )
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                Ok(ProductRelationsAdminCommandResult {
                    item: None,
                    items: reordered
                        .into_iter()
                        .map(|dto| ProductRelationItem {
                            id: dto.id.to_string(),
                            product_id: dto.product_id.to_string(),
                            related_product_id: dto.related_product_id.to_string(),
                            relation_type: dto.relation_type.as_str().to_string(),
                            position: dto.position,
                            metadata: dto.metadata,
                            created_at: dto.created_at.to_rfc3339(),
                            updated_at: dto.updated_at.to_rfc3339(),
                        })
                        .collect(),
                    success: true,
                })
            }
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (idempotency_key, command);
        Err(ServerFnError::new(
            "product_relations command requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn ensure_tenant(
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
) -> Result<(), ServerFnError> {
    if auth.tenant_id != tenant.id {
        return Err(ServerFnError::new(
            "Permission denied: tenant mismatch",
        ));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new(format!("Invalid {field}")))
}
