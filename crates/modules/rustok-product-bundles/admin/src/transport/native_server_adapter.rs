use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::model::{
    BundleAdminCommand, BundleAdminCommandResult, BundleAdminDirectory, BundleAdminFilters,
    BundleAdminRecord,
};
#[cfg(feature = "ssr")]
use crate::model::{BundleAdminItemRecord, BundleAdminListItem, BundleAdminTranslation};

#[derive(Debug, Clone)]
pub struct NativeBundleAdminError(pub String);

impl Display for NativeBundleAdminError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeBundleAdminError {}

impl From<ServerFnError> for NativeBundleAdminError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_directory(
    filters: BundleAdminFilters,
) -> Result<BundleAdminDirectory, NativeBundleAdminError> {
    bundle_directory_native(filters).await.map_err(Into::into)
}

pub async fn load_detail(
    bundle_id: String,
) -> Result<BundleAdminRecord, NativeBundleAdminError> {
    bundle_detail_native(bundle_id).await.map_err(Into::into)
}

pub async fn execute_command(
    idempotency_key: String,
    command: BundleAdminCommand,
) -> Result<BundleAdminCommandResult, NativeBundleAdminError> {
    bundle_command_native(idempotency_key, command)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "bundle/directory")]
async fn bundle_directory_native(
    filters: BundleAdminFilters,
) -> Result<BundleAdminDirectory, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
        use rustok_product_bundles::{BundleFilter, BundlePort, BundleService};

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_tenant(&auth, &tenant)?;

        let page = filters.page.max(1);
        let per_page = filters.per_page.clamp(1, 100);
        let service = BundleService::new(runtime.db_clone());

        let response = service
            .list_bundles(
                tenant.id,
                BundleFilter {
                    search: filters.search,
                    status: filters.status,
                    bundle_type: filters.bundle_type,
                },
                page,
                per_page,
                None,
            )
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        Ok(BundleAdminDirectory {
            items: response
                .items
                .into_iter()
                .map(|b| BundleAdminListItem {
                    id: b.id.to_string(),
                    tenant_id: b.tenant_id.to_string(),
                    slug: b.slug,
                    name: b.name,
                    description: b.description,
                    bundle_type: b.bundle_type,
                    status: b.status,
                    discount_type: b.discount_type,
                    discount_value: b.discount_value.to_string(),
                    items_count: b.items.len() as i64,
                    created_at: b.created_at.to_rfc3339(),
                    updated_at: b.updated_at.to_rfc3339(),
                })
                .collect(),
            total: response.total,
            page,
            per_page,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = filters;
        Err(ServerFnError::new(
            "bundle directory requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "bundle/detail")]
async fn bundle_detail_native(
    bundle_id: String,
) -> Result<BundleAdminRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
        use rustok_product_bundles::{BundlePort, BundleService};

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_tenant(&auth, &tenant)?;

        let id = parse_uuid(bundle_id.as_str(), "bundle_id")?;
        let service = BundleService::new(runtime.db_clone());

        let bundle = service
            .get_bundle(tenant.id, id, None)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        Ok(BundleAdminRecord {
            id: bundle.id.to_string(),
            tenant_id: bundle.tenant_id.to_string(),
            bundle_product_id: bundle.bundle_product_id.map(|id| id.to_string()),
            slug: bundle.slug,
            name: bundle.name,
            description: bundle.description,
            bundle_type: bundle.bundle_type,
            status: bundle.status,
            discount_type: bundle.discount_type,
            discount_value: bundle.discount_value.to_string(),
            metadata: bundle.metadata,
            created_at: bundle.created_at.to_rfc3339(),
            updated_at: bundle.updated_at.to_rfc3339(),
            translations: bundle
                .translations
                .into_iter()
                .map(|t| BundleAdminTranslation {
                    locale: t.locale,
                    name: t.name,
                    description: t.description,
                })
                .collect(),
            items: bundle
                .items
                .into_iter()
                .map(|i| BundleAdminItemRecord {
                    id: i.id.to_string(),
                    bundle_id: i.bundle_id.to_string(),
                    product_id: i.product_id.to_string(),
                    variant_id: i.variant_id.map(|id| id.to_string()),
                    quantity: i.quantity,
                    is_optional: i.is_optional,
                    discount_rate: i.discount_rate.map(|d| d.to_string()),
                    position: i.position,
                })
                .collect(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = bundle_id;
        Err(ServerFnError::new(
            "bundle detail requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "bundle/command")]
async fn bundle_command_native(
    idempotency_key: String,
    command: BundleAdminCommand,
) -> Result<BundleAdminCommandResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
        use rustok_product_bundles::{
            BundlePort, BundleService, CreateBundleInput, UpdateBundleInput,
            dto::BundleItemInput,
        };
        use rust_decimal::Decimal;
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

        let service = BundleService::new(runtime.db_clone());

        match command {
            BundleAdminCommand::Create { draft } => {
                let discount_val = if draft.discount_value.trim().is_empty() {
                    Decimal::default()
                } else {
                    Decimal::from_str(draft.discount_value.trim()).unwrap_or_default()
                };

                let created = service
                    .create_bundle(
                        tenant.id,
                        CreateBundleInput {
                            bundle_product_id: None,
                            slug: draft.slug,
                            bundle_type: Some(draft.bundle_type),
                            status: Some(draft.status),
                            discount_type: Some(draft.discount_type),
                            discount_value: Some(discount_val),
                            metadata: None,
                            translations: vec![rustok_product_bundles::dto::BundleTranslationInput {
                                locale: "en".to_string(),
                                name: draft.name,
                                description: draft.description,
                            }],
                            items: vec![],
                        },
                    )
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                Ok(BundleAdminCommandResult {
                    bundle: Some(BundleAdminRecord {
                        id: created.id.to_string(),
                        tenant_id: created.tenant_id.to_string(),
                        bundle_product_id: created.bundle_product_id.map(|id| id.to_string()),
                        slug: created.slug,
                        name: created.name,
                        description: created.description,
                        bundle_type: created.bundle_type,
                        status: created.status,
                        discount_type: created.discount_type,
                        discount_value: created.discount_value.to_string(),
                        metadata: created.metadata,
                        created_at: created.created_at.to_rfc3339(),
                        updated_at: created.updated_at.to_rfc3339(),
                        translations: Vec::new(),
                        items: Vec::new(),
                    }),
                    success: true,
                })
            }
            BundleAdminCommand::Update { id, draft } => {
                let bundle_id = parse_uuid(id.as_str(), "bundle_id")?;
                let translations = draft.name.map(|name| {
                    vec![rustok_product_bundles::dto::BundleTranslationInput {
                        locale: "en".to_string(),
                        name,
                        description: draft.description.clone(),
                    }]
                });
                let discount_value = draft
                    .discount_value
                    .and_then(|val| Decimal::from_str(val.trim()).ok());

                let updated = service
                    .update_bundle(
                        tenant.id,
                        bundle_id,
                        UpdateBundleInput {
                            bundle_product_id: None,
                            slug: draft.slug,
                            bundle_type: draft.bundle_type,
                            status: draft.status,
                            discount_type: draft.discount_type,
                            discount_value,
                            metadata: None,
                            translations,
                        },
                    )
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                Ok(BundleAdminCommandResult {
                    bundle: Some(BundleAdminRecord {
                        id: updated.id.to_string(),
                        tenant_id: updated.tenant_id.to_string(),
                        bundle_product_id: updated.bundle_product_id.map(|id| id.to_string()),
                        slug: updated.slug,
                        name: updated.name,
                        description: updated.description,
                        bundle_type: updated.bundle_type,
                        status: updated.status,
                        discount_type: updated.discount_type,
                        discount_value: updated.discount_value.to_string(),
                        metadata: updated.metadata,
                        created_at: updated.created_at.to_rfc3339(),
                        updated_at: updated.updated_at.to_rfc3339(),
                        translations: Vec::new(),
                        items: Vec::new(),
                    }),
                    success: true,
                })
            }
            BundleAdminCommand::Delete { id } => {
                let bundle_id = parse_uuid(id.as_str(), "bundle_id")?;
                service
                    .delete_bundle(tenant.id, bundle_id)
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                Ok(BundleAdminCommandResult {
                    bundle: None,
                    success: true,
                })
            }
            BundleAdminCommand::AddItem {
                bundle_id,
                product_id,
                variant_id,
                quantity,
                is_optional,
            } => {
                let b_id = parse_uuid(bundle_id.as_str(), "bundle_id")?;
                let p_id = parse_uuid(product_id.as_str(), "product_id")?;
                let v_id = variant_id
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| parse_uuid(s, "variant_id"))
                    .transpose()?;

                let _ = service
                    .add_bundle_item(
                        tenant.id,
                        b_id,
                        BundleItemInput {
                            product_id: p_id,
                            variant_id: v_id,
                            quantity,
                            is_optional: Some(is_optional),
                            discount_rate: None,
                            position: None,
                        },
                    )
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                let refreshed = service
                    .get_bundle(tenant.id, b_id, None)
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                Ok(BundleAdminCommandResult {
                    bundle: Some(BundleAdminRecord {
                        id: refreshed.id.to_string(),
                        tenant_id: refreshed.tenant_id.to_string(),
                        bundle_product_id: refreshed.bundle_product_id.map(|id| id.to_string()),
                        slug: refreshed.slug,
                        name: refreshed.name,
                        description: refreshed.description,
                        bundle_type: refreshed.bundle_type,
                        status: refreshed.status,
                        discount_type: refreshed.discount_type,
                        discount_value: refreshed.discount_value.to_string(),
                        metadata: refreshed.metadata,
                        created_at: refreshed.created_at.to_rfc3339(),
                        updated_at: refreshed.updated_at.to_rfc3339(),
                        translations: Vec::new(),
                        items: refreshed
                            .items
                            .into_iter()
                            .map(|i| BundleAdminItemRecord {
                                id: i.id.to_string(),
                                bundle_id: i.bundle_id.to_string(),
                                product_id: i.product_id.to_string(),
                                variant_id: i.variant_id.map(|id| id.to_string()),
                                quantity: i.quantity,
                                is_optional: i.is_optional,
                                discount_rate: i.discount_rate.map(|d| d.to_string()),
                                position: i.position,
                            })
                            .collect(),
                    }),
                    success: true,
                })
            }
            BundleAdminCommand::RemoveItem {
                bundle_id,
                item_id,
            } => {
                let b_id = parse_uuid(bundle_id.as_str(), "bundle_id")?;
                let i_id = parse_uuid(item_id.as_str(), "item_id")?;

                service
                    .remove_bundle_item(tenant.id, b_id, i_id)
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                let refreshed = service
                    .get_bundle(tenant.id, b_id, None)
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                Ok(BundleAdminCommandResult {
                    bundle: Some(BundleAdminRecord {
                        id: refreshed.id.to_string(),
                        tenant_id: refreshed.tenant_id.to_string(),
                        bundle_product_id: refreshed.bundle_product_id.map(|id| id.to_string()),
                        slug: refreshed.slug,
                        name: refreshed.name,
                        description: refreshed.description,
                        bundle_type: refreshed.bundle_type,
                        status: refreshed.status,
                        discount_type: refreshed.discount_type,
                        discount_value: refreshed.discount_value.to_string(),
                        metadata: refreshed.metadata,
                        created_at: refreshed.created_at.to_rfc3339(),
                        updated_at: refreshed.updated_at.to_rfc3339(),
                        translations: Vec::new(),
                        items: refreshed
                            .items
                            .into_iter()
                            .map(|i| BundleAdminItemRecord {
                                id: i.id.to_string(),
                                bundle_id: i.bundle_id.to_string(),
                                product_id: i.product_id.to_string(),
                                variant_id: i.variant_id.map(|id| id.to_string()),
                                quantity: i.quantity,
                                is_optional: i.is_optional,
                                discount_rate: i.discount_rate.map(|d| d.to_string()),
                                position: i.position,
                            })
                            .collect(),
                    }),
                    success: true,
                })
            }
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (idempotency_key, command);
        Err(ServerFnError::new(
            "bundle commands require the `ssr` feature",
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
            "Permission denied: bundle tenant mismatch",
        ));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new(format!("Invalid {field}")))
}
