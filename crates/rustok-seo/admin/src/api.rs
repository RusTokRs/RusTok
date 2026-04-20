use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[cfg(feature = "ssr")]
use rustok_seo::SeoService;
use rustok_seo::{
    SeoModuleSettings, SeoRedirectInput, SeoRedirectRecord, SeoRobotsPreviewRecord,
    SeoSitemapStatusRecord,
};

#[cfg(feature = "ssr")]
use rustok_tenant::entities::tenant_module;
#[cfg(feature = "ssr")]
use sea_orm::prelude::Uuid;
#[cfg(feature = "ssr")]
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

#[cfg(feature = "ssr")]
const MODULE_SLUG: &str = "seo";

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

pub async fn fetch_redirects() -> Result<Vec<SeoRedirectRecord>, ApiError> {
    seo_redirects_native().await.map_err(Into::into)
}

pub async fn save_redirect(input: SeoRedirectInput) -> Result<SeoRedirectRecord, ApiError> {
    seo_upsert_redirect_native(input).await.map_err(Into::into)
}

pub async fn fetch_sitemap_status() -> Result<SeoSitemapStatusRecord, ApiError> {
    seo_sitemap_status_native().await.map_err(Into::into)
}

pub async fn generate_sitemaps() -> Result<SeoSitemapStatusRecord, ApiError> {
    seo_generate_sitemaps_native().await.map_err(Into::into)
}

pub async fn fetch_settings() -> Result<SeoModuleSettings, ApiError> {
    seo_settings_native().await.map_err(Into::into)
}

pub async fn save_settings(settings: SeoModuleSettings) -> Result<SeoModuleSettings, ApiError> {
    seo_save_settings_native(settings).await.map_err(Into::into)
}

pub async fn fetch_robots_preview() -> Result<SeoRobotsPreviewRecord, ApiError> {
    seo_robots_preview_native().await.map_err(Into::into)
}

#[cfg(feature = "ssr")]
fn require_permission(
    auth: &rustok_api::AuthContext,
    required: &[rustok_core::Permission],
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
    let settings = SeoService::normalize_settings(input);
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
async fn seo_service_from_context() -> Result<
    (
        SeoService,
        rustok_api::AuthContext,
        rustok_api::TenantContext,
    ),
    ServerFnError,
> {
    use leptos::prelude::expect_context;
    use loco_rs::app::AppContext;

    let app_ctx = expect_context::<AppContext>();
    let auth = leptos_axum::extract::<rustok_api::AuthContext>()
        .await
        .map_err(ServerFnError::new)?;
    let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
        .await
        .map_err(ServerFnError::new)?;

    Ok((
        SeoService::new(
            app_ctx.db.clone(),
            rustok_api::loco::transactional_event_bus_from_context(&app_ctx),
        ),
        auth,
        tenant,
    ))
}

#[server(prefix = "/api/fn", endpoint = "seo/redirects")]
async fn seo_redirects_native() -> Result<Vec<SeoRedirectRecord>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_core::Permission::SEO_READ],
            "seo:read required",
        )?;

        service
            .list_redirects(tenant.id)
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
async fn seo_upsert_redirect_native(
    input: SeoRedirectInput,
) -> Result<SeoRedirectRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_core::Permission::SEO_UPDATE],
            "seo:update required",
        )?;

        service
            .upsert_redirect(&tenant, input)
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
async fn seo_sitemap_status_native() -> Result<SeoSitemapStatusRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[
                rustok_core::Permission::SEO_READ,
                rustok_core::Permission::SEO_GENERATE,
            ],
            "seo:read or seo:generate required",
        )?;

        service
            .sitemap_status(&tenant)
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
async fn seo_generate_sitemaps_native() -> Result<SeoSitemapStatusRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_core::Permission::SEO_GENERATE],
            "seo:generate required",
        )?;

        service
            .generate_sitemaps(&tenant)
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
async fn seo_settings_native() -> Result<SeoModuleSettings, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_core::Permission::SEO_READ],
            "seo:read required",
        )?;

        service
            .load_settings(tenant.id)
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
async fn seo_save_settings_native(
    input: SeoModuleSettings,
) -> Result<SeoModuleSettings, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;

        let app_ctx = expect_context::<AppContext>();
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_core::Permission::SEO_UPDATE],
            "seo:update required",
        )?;

        persist_seo_settings(&app_ctx.db, tenant.id, input).await?;

        service
            .load_settings(tenant.id)
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
async fn seo_robots_preview_native() -> Result<SeoRobotsPreviewRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = seo_service_from_context().await?;
        require_permission(
            &auth,
            &[rustok_core::Permission::SEO_READ],
            "seo:read required",
        )?;

        service
            .robots_preview(&tenant)
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

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::{persist_seo_settings, require_permission, MODULE_SLUG};
    use rustok_api::AuthContext;
    use rustok_core::Permission;
    use rustok_seo::SeoModuleSettings;
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
            settings: Set(settings.into()),
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
        let persisted_settings =
            serde_json::from_value::<SeoModuleSettings>(persisted.settings.into())
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
}
