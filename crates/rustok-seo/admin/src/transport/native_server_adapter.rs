use leptos::prelude::*;

#[cfg(feature = "ssr")]
use rustok_seo::SeoBulkApplyMode;
#[cfg(feature = "ssr")]
use rustok_seo::{SeoApplicationServices, SeoSettingsService};
#[cfg(feature = "ssr")]
use rustok_seo::SeoTargetCapabilityKind;
use rustok_seo::{
    SeoBulkApplyInput, SeoBulkExportInput, SeoBulkImportInput, SeoBulkJobRecord, SeoBulkJobStatus,
    SeoBulkListInput, SeoBulkPage, SeoBulkSelectionInput, SeoBulkSelectionPreviewRecord,
    SeoDiagnosticsSummaryRecord, SeoIndexDeliveryStatusRecord, SeoIndexRepairReplayInput,
    SeoIndexRepairReplayResultRecord, SeoModuleSettings, SeoRedirectInput, SeoRedirectRecord,
    SeoRobotsPreviewRecord, SeoSitemapStatusRecord, SeoTargetRegistryEntry,
};

#[cfg(feature = "ssr")]
use rustok_core::ModuleRuntimeExtensions;
#[cfg(feature = "ssr")]
use rustok_tenant::entities::tenant_module;
#[cfg(feature = "ssr")]
use sea_orm::prelude::Uuid;
#[cfg(feature = "ssr")]
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

#[cfg(feature = "ssr")]
const MODULE_SLUG: &str = "seo";

#[cfg(feature = "ssr")]
fn require_permission(
    auth: &rustok_api::AuthContext,
    required: &[rustok_api::Permission],
    message: &str,
) -> Result<(), ServerFnError> {
    if rustok_api::has_any_effective_permission(&auth.permissions, required) {
        Ok(())
    } else {
        Err(ServerFnError::new(message))
    }
}

#[cfg(feature = "ssr")]
async fn persist_seo_settings(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    input: SeoModuleSettings,
) -> Result<SeoModuleSettings, ServerFnError> {
    let settings = SeoSettingsService::normalize_settings(input);
    let Some(model) = tenant_module::Entity::find()
        .filter(tenant_module::Column::TenantId.eq(tenant_id))
        .filter(tenant_module::Column::ModuleSlug.eq(MODULE_SLUG))
        .one(db)
        .await
        .map_err(|err| ServerFnError::new(err.to_string()))?
    else {
        return Err(ServerFnError::new(
            "Module `seo` must be enabled for this tenant before saving defaults",
        ));
    };

    if !model.enabled {
        return Err(ServerFnError::new(
            "Module `seo` must be enabled for this tenant before saving defaults",
        ));
    }

    let mut active: tenant_module::ActiveModel = model.into();
    active.settings =
        Set(serde_json::to_value(&settings).map_err(|err| ServerFnError::new(err.to_string()))?);
    active
        .update(db)
        .await
        .map_err(|err| ServerFnError::new(err.to_string()))?;

    Ok(settings)
}

#[cfg(feature = "ssr")]
pub(super) async fn seo_service_from_context() -> Result<
    (
        SeoApplicationServices,
        rustok_api::AuthContext,
        rustok_api::TenantContext,
    ),
    ServerFnError,
> {
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;
    use rustok_outbox::TransactionalEventBus;

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let auth = leptos_axum::extract::<rustok_api::AuthContext>()
        .await
        .map_err(ServerFnError::new)?;
    let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
        .await
        .map_err(ServerFnError::new)?;

    Ok((
        {
            let event_bus = runtime_ctx
                .shared_get::<TransactionalEventBus>()
                .ok_or_else(|| {
                    ServerFnError::new(
                        "SEO native transport requires TransactionalEventBus in host runtime context",
                    )
                })?;
            let extensions = runtime_ctx
                .shared_get::<std::sync::Arc<ModuleRuntimeExtensions>>()
                .ok_or_else(|| {
                    ServerFnError::new(
                        "SEO runtime extensions are not initialized; host bootstrap must insert ModuleRuntimeExtensions",
                    )
                })?;
            SeoApplicationServices::from_runtime_extensions(runtime_ctx.db_clone(), event_bus, &extensions)
                .map_err(|err| ServerFnError::new(err.to_string()))?
        },
        auth,
        tenant,
    ))
}

#[server(prefix = "/api/fn", endpoint = "seo/redirects")]
pub(super) async fn seo_redirects_native() -> Result<Vec<SeoRedirectRecord>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_READ],
            "seo:read required",
        )?;

        service
            .redirects().list_redirects(tenant.id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "seo/redirects requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/upsert-redirect")]
pub(super) async fn seo_upsert_redirect_native(
    input: SeoRedirectInput,
) -> Result<SeoRedirectRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_UPDATE],
            "seo:update required",
        )?;

        service
            .redirects().upsert_redirect(&tenant, input)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "seo/upsert-redirect requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/sitemap-status")]
pub(super) async fn seo_sitemap_status_native() -> Result<SeoSitemapStatusRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[
                rustok_api::Permission::SEO_READ,
                rustok_api::Permission::SEO_GENERATE,
            ],
            "seo:read or seo:generate required",
        )?;

        service
            .sitemaps().sitemap_status(&tenant)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "seo/sitemap-status requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/generate-sitemaps")]
pub(super) async fn seo_generate_sitemaps_native() -> Result<SeoSitemapStatusRecord, ServerFnError>
{
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_GENERATE],
            "seo:generate required",
        )?;

        service
            .sitemaps().generate_sitemaps(&tenant)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "seo/generate-sitemaps requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/settings")]
pub(super) async fn seo_settings_native() -> Result<SeoModuleSettings, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_READ],
            "seo:read required",
        )?;

        service
            .settings().load_settings(tenant.id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "seo/settings requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/save-settings")]
pub(super) async fn seo_save_settings_native(
    input: SeoModuleSettings,
) -> Result<SeoModuleSettings, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_UPDATE],
            "seo:update required",
        )?;

        let db = runtime_ctx.db_clone();
        persist_seo_settings(&db, tenant.id, input).await?;

        service
            .settings().load_settings(tenant.id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "seo/save-settings requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/robots-preview")]
pub(super) async fn seo_robots_preview_native() -> Result<SeoRobotsPreviewRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_READ],
            "seo:read required",
        )?;

        service
            .sitemaps().robots_preview(&tenant)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "seo/robots-preview requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/diagnostics")]
pub(super) async fn seo_diagnostics_native(
    locale: Option<String>,
) -> Result<SeoDiagnosticsSummaryRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[
                rustok_api::Permission::SEO_READ,
                rustok_api::Permission::SEO_MANAGE,
            ],
            "seo:read or seo:manage required",
        )?;

        service
            .operations().diagnostics_summary(&tenant, locale.as_deref())
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = locale;
        Err(ServerFnError::new(
            "seo/diagnostics requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/bulk-items")]
pub(super) async fn seo_bulk_items_native(
    input: SeoBulkListInput,
) -> Result<SeoBulkPage, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_MANAGE],
            "seo:manage required",
        )?;

        service
            .bulk().list_bulk_items(&tenant, input)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "seo/bulk-items requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/bulk-targets")]
pub(super) async fn seo_bulk_targets_native() -> Result<Vec<SeoTargetRegistryEntry>, ServerFnError>
{
    #[cfg(feature = "ssr")]
    {
        let (service, auth, _tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_MANAGE],
            "seo:manage required",
        )?;

        Ok(service.routing().target_registry_entries(Some(SeoTargetCapabilityKind::Bulk)))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "seo/bulk-targets requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/bulk-selection-preview")]
pub(super) async fn seo_bulk_selection_preview_native(
    input: SeoBulkSelectionInput,
) -> Result<SeoBulkSelectionPreviewRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_MANAGE],
            "seo:manage required",
        )?;

        service
            .bulk().preview_bulk_selection_count(&tenant, input)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "seo/bulk-selection-preview requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/bulk-jobs")]
pub(super) async fn seo_bulk_jobs_native(
    limit: Option<i32>,
    status: Option<SeoBulkJobStatus>,
) -> Result<Vec<SeoBulkJobRecord>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_MANAGE],
            "seo:manage required",
        )?;

        service
            .bulk().list_bulk_jobs(
                tenant.id,
                limit.unwrap_or(20).clamp(1, 100) as usize,
                status,
            )
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (limit, status);
        Err(ServerFnError::new(
            "seo/bulk-jobs requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/bulk-job")]
pub(super) async fn seo_bulk_job_native(
    job_id: String,
) -> Result<Option<SeoBulkJobRecord>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_MANAGE],
            "seo:manage required",
        )?;
        let job_id =
            Uuid::parse_str(job_id.as_str()).map_err(|err| ServerFnError::new(err.to_string()))?;

        service
            .bulk().bulk_job(tenant.id, job_id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = job_id;
        Err(ServerFnError::new(
            "seo/bulk-job requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn require_bulk_write_permissions(
    auth: &rustok_api::AuthContext,
    publish_after_write: bool,
) -> Result<(), ServerFnError> {
    require_permission(
        auth,
        &[rustok_api::Permission::SEO_MANAGE],
        "seo:manage required",
    )?;
    require_permission(
        auth,
        &[rustok_api::Permission::SEO_UPDATE],
        "seo:update required",
    )?;
    if publish_after_write {
        require_permission(
            auth,
            &[rustok_api::Permission::SEO_PUBLISH],
            "seo:publish required",
        )?;
    }
    Ok(())
}

#[server(prefix = "/api/fn", endpoint = "seo/queue-bulk-apply")]
pub(super) async fn seo_queue_bulk_apply_native(
    input: SeoBulkApplyInput,
) -> Result<SeoBulkJobRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        if input.apply_mode == SeoBulkApplyMode::PreviewOnly {
            require_permission(
                &auth,
                &[rustok_api::Permission::SEO_MANAGE],
                "seo:manage required",
            )?;
        } else {
            require_bulk_write_permissions(&auth, input.publish_after_write)?;
        }

        service
            .bulk().queue_bulk_apply(&tenant, Some(auth.user_id), input)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "seo/queue-bulk-apply requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/queue-bulk-import")]
pub(super) async fn seo_queue_bulk_import_native(
    input: SeoBulkImportInput,
) -> Result<SeoBulkJobRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_bulk_write_permissions(&auth, input.publish_after_write)?;

        service
            .bulk().queue_bulk_import(&tenant, Some(auth.user_id), input)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "seo/queue-bulk-import requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/queue-bulk-export")]
pub(super) async fn seo_queue_bulk_export_native(
    input: SeoBulkExportInput,
) -> Result<SeoBulkJobRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_MANAGE],
            "seo:manage required",
        )?;

        service
            .bulk().queue_bulk_export(&tenant, Some(auth.user_id), input)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "seo/queue-bulk-export requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/index-tracking")]
pub(super) async fn seo_index_tracking_native(
    target_type: Option<String>,
) -> Result<SeoIndexDeliveryStatusRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_MANAGE],
            "seo:manage required",
        )?;

        service
            .operations().index_delivery_status(tenant.id, target_type.as_deref())
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = target_type;
        Err(ServerFnError::new(
            "seo/index-tracking requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "seo/index-repair-replay")]
pub(super) async fn seo_index_repair_replay_native(
    input: SeoIndexRepairReplayInput,
) -> Result<SeoIndexRepairReplayResultRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_api::Permission::SEO_MANAGE],
            "seo:manage required",
        )?;

        service
            .operations().run_index_repair_replay(
                tenant.id,
                input.target_type.as_deref(),
                input.limit.clamp(1, 500) as usize,
                input.replay_historical,
            )
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "seo/index-repair-replay requires the `ssr` feature",
        ))
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::{MODULE_SLUG, persist_seo_settings, require_permission};
    use rustok_api::AuthContext;
    use rustok_api::Permission;
    use rustok_seo::{SeoIndexRepairReplayInput, SeoModuleSettings};
    use rustok_tenant::entities::tenant_module;
    use sea_orm::prelude::Uuid;
    use sea_orm::{
        ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
        DatabaseConnection, DbBackend, EntityTrait, QueryFilter, Statement,
    };
    use serde_json::json;

    async fn test_db() -> DatabaseConnection {
        let db_url = format!(
            "sqlite:file:seo_admin_api_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut opts = ConnectOptions::new(db_url);
        opts.max_connections(5)
            .min_connections(1)
            .sqlx_logging(false);
        Database::connect(opts)
            .await
            .expect("failed to connect seo admin sqlite db")
    }

    async fn seed_tenant_modules_table(db: &DatabaseConnection) {
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE tenant_modules (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                module_slug TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                settings TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )"
            .to_string(),
        ))
        .await
        .expect("create tenant_modules table");
    }

    async fn insert_tenant_module(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        enabled: bool,
        settings: serde_json::Value,
    ) {
        let now = chrono::Utc::now();
        tenant_module::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            module_slug: Set(MODULE_SLUG.to_string()),
            enabled: Set(enabled),
            settings: Set(settings),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await
        .expect("insert tenant module");
    }

    fn auth_with_permissions(permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    #[test]
    fn require_permission_accepts_manage_for_update() {
        let auth = auth_with_permissions(vec![Permission::SEO_MANAGE]);

        let result = require_permission(&auth, &[Permission::SEO_UPDATE], "seo:update required");

        assert!(result.is_ok());
    }

    #[test]
    fn require_permission_rejects_missing_permission() {
        let auth = auth_with_permissions(vec![Permission::SEO_READ]);

        let result = require_permission(&auth, &[Permission::SEO_UPDATE], "seo:update required");

        assert!(
            result
                .expect_err("missing permission should fail")
                .to_string()
                .contains("seo:update required"),
            "missing permission error should mention seo:update required"
        );
    }

    #[tokio::test]
    async fn persist_seo_settings_rejects_missing_module_row() {
        let db = test_db().await;
        seed_tenant_modules_table(&db).await;

        let result = persist_seo_settings(&db, Uuid::new_v4(), SeoModuleSettings::default()).await;

        assert!(
            result
                .expect_err("missing seo module row should fail")
                .to_string()
                .contains("Module `seo` must be enabled for this tenant before saving defaults"),
            "missing module row error should mention enabled seo module requirement"
        );
    }

    #[tokio::test]
    async fn persist_seo_settings_rejects_disabled_module_row() {
        let db = test_db().await;
        seed_tenant_modules_table(&db).await;
        let tenant_id = Uuid::new_v4();
        insert_tenant_module(&db, tenant_id, false, json!({})).await;

        let result = persist_seo_settings(&db, tenant_id, SeoModuleSettings::default()).await;

        assert!(
            result
                .expect_err("disabled seo module row should fail")
                .to_string()
                .contains("Module `seo` must be enabled for this tenant before saving defaults"),
            "disabled module row error should mention enabled seo module requirement"
        );
    }

    #[tokio::test]
    async fn persist_seo_settings_stores_normalized_payload() {
        let db = test_db().await;
        seed_tenant_modules_table(&db).await;
        let tenant_id = Uuid::new_v4();
        insert_tenant_module(&db, tenant_id, true, json!({})).await;

        let stored = persist_seo_settings(
            &db,
            tenant_id,
            SeoModuleSettings {
                default_robots: vec![
                    " Index ".to_string(),
                    "FOLLOW".to_string(),
                    "index".to_string(),
                ],
                sitemap_enabled: false,
                allowed_redirect_hosts: vec![
                    " Example.com ".to_string(),
                    "cdn.example.com".to_string(),
                    "example.com".to_string(),
                ],
                allowed_canonical_hosts: vec![
                    " Blog.Example.com ".to_string(),
                    "blog.example.com".to_string(),
                ],
                x_default_locale: Some(" EN-us ".to_string()),
                ..SeoModuleSettings::default()
            },
        )
        .await
        .expect("save normalized settings");

        assert_eq!(stored.default_robots, vec!["index", "follow"]);
        assert!(!stored.sitemap_enabled);
        assert_eq!(
            stored.allowed_redirect_hosts,
            vec!["example.com", "cdn.example.com"]
        );
        assert_eq!(stored.allowed_canonical_hosts, vec!["blog.example.com"]);
        assert_eq!(stored.x_default_locale.as_deref(), Some("en-US"));

        let persisted = tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .filter(tenant_module::Column::ModuleSlug.eq(MODULE_SLUG))
            .one(&db)
            .await
            .expect("load tenant module row")
            .expect("seo module row");
        let persisted_settings = serde_json::from_value::<SeoModuleSettings>(persisted.settings)
            .expect("deserialize persisted settings");

        assert_eq!(persisted_settings.default_robots, vec!["index", "follow"]);
        assert!(!persisted_settings.sitemap_enabled);
        assert_eq!(
            persisted_settings.allowed_redirect_hosts,
            vec!["example.com", "cdn.example.com"]
        );
        assert_eq!(
            persisted_settings.allowed_canonical_hosts,
            vec!["blog.example.com"]
        );
        assert_eq!(
            persisted_settings.x_default_locale.as_deref(),
            Some("en-US")
        );
    }

    #[test]
    fn normalize_index_target_type_accepts_supported_values() {
        assert_eq!(
            super::super::normalize_index_target_type(Some(" content ".to_string()))
                .expect("content target"),
            Some("content".to_string())
        );
        assert_eq!(
            super::super::normalize_index_target_type(Some("PRODUCT".to_string()))
                .expect("product target"),
            Some("product".to_string())
        );
        assert_eq!(
            super::super::normalize_index_target_type(Some("   ".to_string()))
                .expect("empty target"),
            None
        );
    }

    #[test]
    fn normalize_index_target_type_rejects_unknown_values() {
        let err = super::super::normalize_index_target_type(Some("forum".to_string()))
            .expect_err("unsupported target type must fail");
        assert_eq!(err, "Index target type must be `content` or `product`");
    }

    #[test]
    fn normalize_index_repair_input_clamps_limit_and_normalizes_target_type() {
        let input = super::super::normalize_index_repair_replay_input(SeoIndexRepairReplayInput {
            target_type: Some(" PRODUCT ".to_string()),
            limit: 700,
            replay_historical: true,
        })
        .expect("input should normalize");

        assert_eq!(input.target_type.as_deref(), Some("product"));
        assert_eq!(input.limit, 500);
        assert!(input.replay_historical);
    }
}
