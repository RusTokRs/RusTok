use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::model::{
    BrandAdminCommand, BrandAdminCommandResult, BrandAdminDirectory, BrandAdminFilters,
    BrandAdminRecord,
};
#[cfg(feature = "ssr")]
use crate::model::{BrandAdminListItem, BrandAdminTranslation};

#[derive(Debug, Clone)]
pub struct NativeBrandAdminError(pub String);

impl Display for NativeBrandAdminError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeBrandAdminError {}

impl From<ServerFnError> for NativeBrandAdminError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_directory(
    filters: BrandAdminFilters,
) -> Result<BrandAdminDirectory, NativeBrandAdminError> {
    brand_directory_native(filters).await.map_err(Into::into)
}

pub async fn load_detail(
    brand_id: String,
) -> Result<BrandAdminRecord, NativeBrandAdminError> {
    brand_detail_native(brand_id).await.map_err(Into::into)
}

pub async fn execute_command(
    idempotency_key: String,
    command: BrandAdminCommand,
) -> Result<BrandAdminCommandResult, NativeBrandAdminError> {
    brand_command_native(idempotency_key, command)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "brand/directory")]
async fn brand_directory_native(
    filters: BrandAdminFilters,
) -> Result<BrandAdminDirectory, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
        use rustok_brand::{BrandFilter, BrandPort, BrandService};

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
        let service = BrandService::new(runtime.db_clone());

        let response = service
            .list_brands(
                tenant.id,
                BrandFilter {
                    search: filters.search,
                    is_active: filters.is_active,
                },
                page,
                per_page,
                None,
            )
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        Ok(BrandAdminDirectory {
            items: response
                .items
                .into_iter()
                .map(|b| BrandAdminListItem {
                    id: b.id.to_string(),
                    tenant_id: b.tenant_id.to_string(),
                    slug: b.slug,
                    name: b.name,
                    description: b.description,
                    website_url: b.website_url,
                    logo_url: b.logo_media_id.map(|id| id.to_string()),
                    is_active: b.is_active,
                    sort_order: 0,
                    products_count: 0,
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
            "brand directory requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "brand/detail")]
async fn brand_detail_native(
    brand_id: String,
) -> Result<BrandAdminRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
        use rustok_brand::{BrandPort, BrandService};

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_tenant(&auth, &tenant)?;

        let id = parse_uuid(brand_id.as_str(), "brand_id")?;
        let service = BrandService::new(runtime.db_clone());

        let brand = service
            .get_brand(tenant.id, id, None)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        Ok(BrandAdminRecord {
            id: brand.id.to_string(),
            tenant_id: brand.tenant_id.to_string(),
            slug: brand.slug,
            name: brand.name,
            description: brand.description,
            website_url: brand.website_url,
            logo_url: brand.logo_media_id.map(|id| id.to_string()),
            metadata: brand.metadata,
            is_active: brand.is_active,
            sort_order: 0,
            created_at: brand.created_at.to_rfc3339(),
            updated_at: brand.updated_at.to_rfc3339(),
            translations: brand
                .translations
                .into_iter()
                .map(|t| BrandAdminTranslation {
                    locale: t.locale,
                    name: t.name,
                    description: t.description,
                })
                .collect(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = brand_id;
        Err(ServerFnError::new(
            "brand detail requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "brand/command")]
async fn brand_command_native(
    idempotency_key: String,
    command: BrandAdminCommand,
) -> Result<BrandAdminCommandResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
        use rustok_brand::{
            BrandPort, BrandService, CreateBrandInput, UpdateBrandInput,
        };

        let _ = idempotency_key;
        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_tenant(&auth, &tenant)?;

        let service = BrandService::new(runtime.db_clone());

        match command {
            BrandAdminCommand::Create { draft } => {
                let created = service
                    .create_brand(
                        tenant.id,
                        CreateBrandInput {
                            slug: draft.slug,
                            logo_media_id: None,
                            banner_media_id: None,
                            website_url: if draft.website_url.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                                None
                            } else {
                                draft.website_url
                            },
                            is_active: Some(draft.is_active),
                            metadata: None,
                            translations: vec![rustok_brand::dto::BrandTranslationInput {
                                locale: "en".to_string(),
                                name: draft.name,
                                description: draft.description,
                            }],
                        },
                    )
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                Ok(BrandAdminCommandResult {
                    brand: Some(BrandAdminRecord {
                        id: created.id.to_string(),
                        tenant_id: created.tenant_id.to_string(),
                        slug: created.slug,
                        name: created.name,
                        description: created.description,
                        website_url: created.website_url,
                        logo_url: created.logo_media_id.map(|id| id.to_string()),
                        metadata: created.metadata,
                        is_active: created.is_active,
                        sort_order: 0,
                        created_at: created.created_at.to_rfc3339(),
                        updated_at: created.updated_at.to_rfc3339(),
                        translations: Vec::new(),
                    }),
                    success: true,
                })
            }
            BrandAdminCommand::Update { id, draft } => {
                let brand_id = parse_uuid(id.as_str(), "brand_id")?;
                let translations = draft.name.map(|name| {
                    vec![rustok_brand::dto::BrandTranslationInput {
                        locale: "en".to_string(),
                        name,
                        description: draft.description.clone(),
                    }]
                });
                let updated = service
                    .update_brand(
                        tenant.id,
                        brand_id,
                        UpdateBrandInput {
                            slug: draft.slug,
                            logo_media_id: None,
                            banner_media_id: None,
                            website_url: draft.website_url.map(Some),
                            is_active: draft.is_active,
                            metadata: None,
                            translations,
                        },
                    )
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                Ok(BrandAdminCommandResult {
                    brand: Some(BrandAdminRecord {
                        id: updated.id.to_string(),
                        tenant_id: updated.tenant_id.to_string(),
                        slug: updated.slug,
                        name: updated.name,
                        description: updated.description,
                        website_url: updated.website_url,
                        logo_url: updated.logo_media_id.map(|id| id.to_string()),
                        metadata: updated.metadata,
                        is_active: updated.is_active,
                        sort_order: 0,
                        created_at: updated.created_at.to_rfc3339(),
                        updated_at: updated.updated_at.to_rfc3339(),
                        translations: Vec::new(),
                    }),
                    success: true,
                })
            }
            BrandAdminCommand::Delete { id } => {
                let brand_id = parse_uuid(id.as_str(), "brand_id")?;
                service
                    .delete_brand(tenant.id, brand_id)
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                Ok(BrandAdminCommandResult {
                    brand: None,
                    success: true,
                })
            }
            BrandAdminCommand::SetTranslation {
                id,
                locale,
                name,
                description,
            } => {
                let brand_id = parse_uuid(id.as_str(), "brand_id")?;
                let updated = service
                    .update_brand(
                        tenant.id,
                        brand_id,
                        UpdateBrandInput {
                            slug: None,
                            logo_media_id: None,
                            banner_media_id: None,
                            website_url: None,
                            is_active: None,
                            metadata: None,
                            translations: Some(vec![rustok_brand::dto::BrandTranslationInput {
                                locale,
                                name,
                                description,
                            }]),
                        },
                    )
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

                Ok(BrandAdminCommandResult {
                    brand: Some(BrandAdminRecord {
                        id: updated.id.to_string(),
                        tenant_id: updated.tenant_id.to_string(),
                        slug: updated.slug,
                        name: updated.name,
                        description: updated.description,
                        website_url: updated.website_url,
                        logo_url: updated.logo_media_id.map(|id| id.to_string()),
                        metadata: updated.metadata,
                        is_active: updated.is_active,
                        sort_order: 0,
                        created_at: updated.created_at.to_rfc3339(),
                        updated_at: updated.updated_at.to_rfc3339(),
                        translations: Vec::new(),
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
            "brand commands require the `ssr` feature",
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
            "Permission denied: brand tenant mismatch",
        ));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new(format!("Invalid {field}")))
}
