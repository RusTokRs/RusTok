use std::collections::{HashMap, HashSet};

use async_graphql::{Context, FieldError, Object, Result};
use chrono::{Duration, Utc};
use rustok_core::{ModuleRegistry, Permission};
use rustok_telemetry::metrics;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DbBackend, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Statement,
};
use semver::{Version, VersionReq};
use std::time::Instant;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::common::{encode_cursor, PageInfo, PaginationInput};
use crate::graphql::errors::GraphQLError;
#[cfg(feature = "mod-content")]
use crate::graphql::types::ResolvedCanonicalRoute;
use crate::graphql::types::{
    ActivityItem, ActivityUser, BuildJob, DashboardStats, InstalledModule, MarketplaceModule,
    MarketplaceModuleVersion, ModuleRegistryItem, ModuleSettingField, ReleaseInfo, Tenant,
    TenantModule, User, UserConnection, UserEdge, UsersFilter,
};
use crate::models::_entities::tenant_modules::Column as TenantModulesColumn;
use crate::models::_entities::tenant_modules::Entity as TenantModulesEntity;
use crate::models::_entities::users::Column as UsersColumn;
use crate::models::build::{Column as BuildColumn, Entity as BuildEntity};
use crate::models::release::{Column as ReleaseColumn, Entity as ReleaseEntity, ReleaseStatus};
use crate::models::users;
use crate::modules::ManifestManager;
use crate::services::build_service::BuildService;
use crate::services::marketplace_catalog::marketplace_catalog_from_context;
use crate::services::rbac_service::RbacService;
#[cfg(feature = "mod-content")]
use rustok_content::CanonicalUrlService;

fn calculate_percent_change(current: i64, previous: i64) -> f64 {
    if previous == 0 {
        if current == 0 {
            0.0
        } else {
            100.0
        }
    } else {
        ((current - previous) as f64 / previous as f64) * 100.0
    }
}

fn clamp_collection_limit(limit: Option<i32>) -> usize {
    limit.unwrap_or(100).clamp(1, 100) as usize
}

fn requested_collection_limit(limit: Option<i32>) -> Option<u64> {
    limit.map(|value| value.max(0) as u64)
}

#[cfg(feature = "mod-content")]
fn map_content_error(err: rustok_content::ContentError) -> FieldError {
    match err {
        rustok_content::ContentError::Validation(message)
        | rustok_content::ContentError::Forbidden(message) => FieldError::new(message),
        rustok_content::ContentError::NodeNotFound(_)
        | rustok_content::ContentError::CategoryNotFound(_)
        | rustok_content::ContentError::TranslationNotFound { .. }
        | rustok_content::ContentError::DuplicateSlug { .. }
        | rustok_content::ContentError::ConcurrentModification { .. } => {
            FieldError::new(err.to_string())
        }
        rustok_content::ContentError::Database(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
        rustok_content::ContentError::Core(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
        rustok_content::ContentError::Rich(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct OrderStatsSnapshot {
    total_orders: i64,
    total_revenue: i64,
    current_orders: i64,
    previous_orders: i64,
    current_revenue: i64,
    previous_revenue: i64,
}

#[derive(Debug, Clone, Copy, Default)]
struct PeriodCountSnapshot {
    total_count: i64,
    current_count: i64,
    previous_count: i64,
}

async fn load_period_count_snapshot(
    db: &sea_orm::DatabaseConnection,
    table: &str,
    tenant_id: uuid::Uuid,
    current_period_start: chrono::DateTime<Utc>,
    previous_period_start: chrono::DateTime<Utc>,
    extra_filter_sql: Option<&str>,
    extra_value: Option<&str>,
) -> std::result::Result<PeriodCountSnapshot, sea_orm::DbErr> {
    let backend = db.get_database_backend();
    let filter_sql = extra_filter_sql.unwrap_or("");

    let statement = match backend {
        DbBackend::Sqlite => {
            let sql = format!(
                r#"
                SELECT
                    CAST(COUNT(*) AS INTEGER) AS total_count,
                    CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_count,
                    CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_count
                FROM {table}
                WHERE tenant_id = ?1{filter_sql}
                "#
            );

            let mut values = vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ];
            if let Some(extra_value) = extra_value {
                values.push(extra_value.into());
            }

            Statement::from_sql_and_values(backend, sql, values)
        }
        _ => {
            let sql = format!(
                r#"
                SELECT
                    COUNT(*)::bigint AS total_count,
                    COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_count,
                    COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_count
                FROM {table}
                WHERE tenant_id = $1{filter_sql}
                "#
            );

            let mut values = vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ];
            if let Some(extra_value) = extra_value {
                values.push(extra_value.into());
            }

            Statement::from_sql_and_values(backend, sql, values)
        }
    };

    let Some(row) = db.query_one(statement).await? else {
        return Ok(PeriodCountSnapshot::default());
    };

    Ok(PeriodCountSnapshot {
        total_count: row.try_get("", "total_count")?,
        current_count: row.try_get("", "current_count")?,
        previous_count: row.try_get("", "previous_count")?,
    })
}

async fn load_order_stats_snapshot(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    current_period_start: chrono::DateTime<Utc>,
    previous_period_start: chrono::DateTime<Utc>,
) -> std::result::Result<OrderStatsSnapshot, sea_orm::DbErr> {
    let backend = db.get_database_backend();
    let tenant_id = tenant_id.to_string();

    let statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            backend,
            r#"
            SELECT
                CAST(COUNT(*) AS INTEGER) AS total_orders,
                CAST(COALESCE(SUM(COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)), 0) AS INTEGER) AS total_revenue,
                CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_orders,
                CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_orders,
                CAST(COALESCE(SUM(CASE
                    WHEN created_at >= ?2 THEN COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)
                    ELSE 0
                END), 0) AS INTEGER) AS current_revenue,
                CAST(COALESCE(SUM(CASE
                    WHEN created_at >= ?3 AND created_at < ?2 THEN COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)
                    ELSE 0
                END), 0) AS INTEGER) AS previous_revenue
            FROM sys_events
            WHERE event_type = 'order.placed'
              AND (
                  json_extract(payload, '$.tenant_id') = ?1
                  OR json_extract(payload, '$.event.tenant_id') = ?1
              )
            "#,
            vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ],
        ),
        _ => Statement::from_sql_and_values(
            backend,
            r#"
            SELECT
                COUNT(*)::bigint AS total_orders,
                COALESCE(SUM(COALESCE((payload->'event'->'data'->>'total')::bigint, 0)), 0)::bigint AS total_revenue,
                COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_orders,
                COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_orders,
                COALESCE(SUM(CASE
                    WHEN created_at >= $2 THEN COALESCE((payload->'event'->'data'->>'total')::bigint, 0)
                    ELSE 0
                END), 0)::bigint AS current_revenue,
                COALESCE(SUM(CASE
                    WHEN created_at >= $3 AND created_at < $2 THEN COALESCE((payload->'event'->'data'->>'total')::bigint, 0)
                    ELSE 0
                END), 0)::bigint AS previous_revenue
            FROM sys_events
            WHERE event_type = 'order.placed'
              AND (
                  payload->>'tenant_id' = $1
                  OR payload->'event'->>'tenant_id' = $1
              )
            "#,
            vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ],
        ),
    };

    let Some(row) = db.query_one(statement).await? else {
        return Ok(OrderStatsSnapshot::default());
    };

    Ok(OrderStatsSnapshot {
        total_orders: row.try_get("", "total_orders")?,
        total_revenue: row.try_get("", "total_revenue")?,
        current_orders: row.try_get("", "current_orders")?,
        previous_orders: row.try_get("", "previous_orders")?,
        current_revenue: row.try_get("", "current_revenue")?,
        previous_revenue: row.try_get("", "previous_revenue")?,
    })
}

fn humanize_slug(slug: &str) -> String {
    slug.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn fallback_module_category(slug: &str) -> &'static str {
    match slug {
        "content" | "blog" | "forum" | "pages" => "content",
        "commerce" => "commerce",
        "tenant" | "rbac" | "index" | "outbox" => "platform",
        _ => "extensions",
    }
}

fn normalize_version_req(raw: &str, is_max: bool) -> String {
    let trimmed = raw.trim();
    let wildcard = trimmed.replace(".x", ".*").replace(".X", ".*");
    let has_operator = wildcard.contains('<')
        || wildcard.contains('>')
        || wildcard.contains('=')
        || wildcard.contains('~')
        || wildcard.contains('^')
        || wildcard.contains('*')
        || wildcard.contains(',');

    if has_operator {
        return wildcard;
    }

    if is_max {
        format!("<= {wildcard}")
    } else {
        format!(">= {wildcard}")
    }
}

fn current_platform_version() -> Option<Version> {
    Version::parse(env!("CARGO_PKG_VERSION")).ok()
}

fn is_catalog_module_compatible(entry: &crate::modules::CatalogManifestModule) -> bool {
    let Some(platform_version) = current_platform_version() else {
        return true;
    };

    let min_ok = entry
        .rustok_min_version
        .as_deref()
        .and_then(|raw| VersionReq::parse(&normalize_version_req(raw, false)).ok())
        .is_none_or(|req| req.matches(&platform_version));
    let max_ok = entry
        .rustok_max_version
        .as_deref()
        .and_then(|raw| VersionReq::parse(&normalize_version_req(raw, true)).ok())
        .is_none_or(|req| req.matches(&platform_version));

    min_ok && max_ok
}

fn marketplace_module_from_catalog_entry(
    entry: crate::modules::CatalogManifestModule,
    registry: &ModuleRegistry,
    installed_modules: &[crate::modules::InstalledManifestModule],
) -> MarketplaceModule {
    let compatible = is_catalog_module_compatible(&entry);
    let signature_present = entry.signature.is_some();
    let runtime_module = registry.get(&entry.slug);
    let installed_module = installed_modules
        .iter()
        .find(|module| module.slug == entry.slug);
    let latest_version = runtime_module
        .map(|module| module.version().to_string())
        .or_else(|| entry.version.clone())
        .unwrap_or_else(|| "workspace".to_string());
    let installed_version = installed_module.and_then(|module| module.version.clone());
    let dependencies = runtime_module
        .map(|module| {
            module
                .dependencies()
                .iter()
                .map(|dependency| dependency.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| entry.depends_on.clone());
    let versions = if entry.versions.is_empty() {
        vec![MarketplaceModuleVersion {
            version: latest_version.clone(),
            changelog: None,
            yanked: false,
            published_at: None,
            checksum_sha256: entry.checksum_sha256.clone(),
            signature_present,
        }]
    } else {
        entry
            .versions
            .iter()
            .map(|version| MarketplaceModuleVersion {
                version: version.version.clone(),
                changelog: version.changelog.clone(),
                yanked: version.yanked,
                published_at: version.published_at.clone(),
                checksum_sha256: version.checksum_sha256.clone(),
                signature_present: version.signature.is_some(),
            })
            .collect()
    };

    MarketplaceModule {
        slug: entry.slug.clone(),
        name: runtime_module
            .map(|module| module.name().to_string())
            .or_else(|| entry.name.clone())
            .unwrap_or_else(|| humanize_slug(&entry.slug)),
        latest_version: latest_version.clone(),
        description: runtime_module
            .map(|module| module.description().to_string())
            .or_else(|| entry.description.clone())
            .unwrap_or_else(|| {
                format!(
                    "{} module from {} source",
                    humanize_slug(&entry.slug),
                    entry.source
                )
            }),
        source: entry.source.clone(),
        kind: if entry.required || registry.is_core(&entry.slug) {
            "core".to_string()
        } else {
            "optional".to_string()
        },
        category: entry
            .category
            .clone()
            .unwrap_or_else(|| fallback_module_category(&entry.slug).to_string()),
        tags: entry.tags.clone(),
        icon_url: entry.icon_url.clone(),
        banner_url: entry.banner_url.clone(),
        screenshots: entry.screenshots.clone(),
        crate_name: entry.crate_name,
        dependencies,
        ownership: entry.ownership,
        trust_level: entry.trust_level,
        rustok_min_version: entry.rustok_min_version,
        rustok_max_version: entry.rustok_max_version,
        publisher: entry.publisher,
        checksum_sha256: entry.checksum_sha256,
        signature_present,
        versions,
        compatible,
        recommended_admin_surfaces: entry.recommended_admin_surfaces,
        showcase_admin_surfaces: entry.showcase_admin_surfaces,
        settings_schema: settings_schema_fields(&entry.settings_schema),
        installed: installed_module.is_some(),
        installed_version: installed_version.clone(),
        update_available: installed_version
            .as_ref()
            .is_some_and(|version| version != &latest_version),
    }
}

fn settings_schema_fields(
    schema: &HashMap<String, crate::modules::ModuleSettingSpec>,
) -> Vec<ModuleSettingField> {
    let mut keys = schema.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    keys.into_iter()
        .filter_map(|key| {
            schema
                .get(&key)
                .map(|spec| ModuleSettingField::from_spec(key, spec))
        })
        .collect()
}

fn marketplace_modules_from_catalog(
    entries: Vec<crate::modules::CatalogManifestModule>,
    registry: &ModuleRegistry,
    installed_modules: &[crate::modules::InstalledManifestModule],
) -> Vec<MarketplaceModule> {
    entries
        .into_iter()
        .map(|entry| marketplace_module_from_catalog_entry(entry, registry, installed_modules))
        .collect()
}

fn trust_level_matches(module: &MarketplaceModule, trust_level: Option<&str>) -> bool {
    trust_level.is_none_or(|trust_level| module.trust_level.eq_ignore_ascii_case(trust_level))
}

fn source_matches(module: &MarketplaceModule, source: Option<&str>) -> bool {
    source.is_none_or(|source| module.source.eq_ignore_ascii_case(source))
}

async fn ensure_modules_read_permission(ctx: &Context<'_>) -> Result<()> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
    let tenant = ctx.data::<TenantContext>()?;

    let can_read_modules = RbacService::has_any_permission(
        &app_ctx.db,
        &tenant.id,
        &auth.user_id,
        &[
            Permission::MODULES_READ,
            Permission::MODULES_LIST,
            Permission::MODULES_MANAGE,
        ],
    )
    .await
    .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

    if !can_read_modules {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: modules:read, modules:list, or modules:manage required",
        ));
    }

    Ok(())
}

async fn load_marketplace_catalog(
    app_ctx: &loco_rs::app::AppContext,
    manifest: &crate::modules::ModulesManifest,
    registry: &ModuleRegistry,
) -> Result<Vec<crate::modules::CatalogManifestModule>> {
    marketplace_catalog_from_context(app_ctx)
        .list_modules(manifest, registry)
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))
}

#[derive(Default)]
pub struct RootQuery;

#[Object]
impl RootQuery {
    async fn health(&self) -> &str {
        "GraphQL is working!"
    }

    async fn api_version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    async fn current_tenant(&self, ctx: &Context<'_>) -> Result<Tenant> {
        let tenant = ctx.data::<TenantContext>()?;
        Ok(Tenant {
            id: tenant.id,
            name: tenant.name.clone(),
            slug: tenant.slug.clone(),
        })
    }

    #[cfg(feature = "mod-content")]
    async fn resolve_canonical_route(
        &self,
        ctx: &Context<'_>,
        route: String,
        locale: String,
    ) -> Result<Option<ResolvedCanonicalRoute>> {
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let lookup_started_at = Instant::now();
        let service = CanonicalUrlService::new(app_ctx.db.clone());
        let resolved = service
            .resolve_route(tenant.id, locale.as_str(), route.as_str())
            .await
            .map_err(map_content_error)?;

        metrics::record_read_path_query(
            "graphql",
            "root.resolve_canonical_route",
            "canonical_lookup",
            lookup_started_at.elapsed().as_secs_f64(),
            resolved.is_some() as u64,
        );

        Ok(resolved.map(ResolvedCanonicalRoute::from))
    }

    async fn enabled_modules(&self, ctx: &Context<'_>, limit: Option<i32>) -> Result<Vec<String>> {
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);
        let modules = TenantModulesEntity::find()
            .filter(TenantModulesColumn::TenantId.eq(tenant.id))
            .filter(TenantModulesColumn::Enabled.eq(true))
            .order_by_asc(TenantModulesColumn::ModuleSlug)
            .limit(limit as u64)
            .all(&app_ctx.db)
            .await
            .map_err(|err| err.to_string())?;

        let modules = modules
            .into_iter()
            .map(|module| module.module_slug)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.enabled_modules",
            requested_limit,
            limit as u64,
            modules.len(),
        );

        Ok(modules)
    }

    async fn module_registry(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<ModuleRegistryItem>> {
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);
        let manifest = ManifestManager::load()
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        let catalog_by_slug: HashMap<String, crate::modules::CatalogManifestModule> =
            load_marketplace_catalog(app_ctx, &manifest, registry)
                .await?
                .into_iter()
                .map(|module| (module.slug.clone(), module))
                .collect();
        let enabled_modules = TenantModulesEntity::find_enabled(&app_ctx.db, tenant.id)
            .await
            .map_err(|err| err.to_string())?;
        let enabled_set: HashSet<String> = enabled_modules.into_iter().collect();

        let modules = registry
            .list()
            .into_iter()
            .take(limit)
            .map(|module| {
                let catalog_entry = catalog_by_slug.get(module.slug());

                ModuleRegistryItem {
                    module_slug: module.slug().to_string(),
                    name: module.name().to_string(),
                    description: module.description().to_string(),
                    version: module.version().to_string(),
                    kind: if registry.is_core(module.slug()) {
                        "core".to_string()
                    } else {
                        "optional".to_string()
                    },
                    enabled: registry.is_core(module.slug()) || enabled_set.contains(module.slug()),
                    dependencies: module
                        .dependencies()
                        .iter()
                        .map(|dependency| dependency.to_string())
                        .collect(),
                    ownership: catalog_entry
                        .map(|entry| entry.ownership.clone())
                        .unwrap_or_else(|| "third_party".to_string()),
                    trust_level: catalog_entry
                        .map(|entry| entry.trust_level.clone())
                        .unwrap_or_else(|| "unverified".to_string()),
                    recommended_admin_surfaces: catalog_entry
                        .map(|entry| entry.recommended_admin_surfaces.clone())
                        .unwrap_or_default(),
                    showcase_admin_surfaces: catalog_entry
                        .map(|entry| entry.showcase_admin_surfaces.clone())
                        .unwrap_or_default(),
                    settings_schema: catalog_entry
                        .map(|entry| settings_schema_fields(&entry.settings_schema))
                        .unwrap_or_default(),
                }
            })
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.module_registry",
            requested_limit,
            limit as u64,
            modules.len(),
        );

        Ok(modules)
    }

    async fn tenant_modules(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<TenantModule>> {
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);
        let modules = TenantModulesEntity::find()
            .filter(TenantModulesColumn::TenantId.eq(tenant.id))
            .order_by_asc(TenantModulesColumn::ModuleSlug)
            .limit(limit as u64)
            .all(&app_ctx.db)
            .await
            .map_err(|err| err.to_string())?;

        let modules = modules
            .into_iter()
            .map(|module| TenantModule {
                module_slug: module.module_slug,
                enabled: module.enabled,
                settings: module.settings.to_string(),
            })
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.tenant_modules",
            requested_limit,
            limit as u64,
            modules.len(),
        );

        Ok(modules)
    }

    async fn installed_modules(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<InstalledModule>> {
        ensure_modules_read_permission(ctx).await?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);

        let manifest = ManifestManager::load()
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let modules = ManifestManager::installed_modules(&manifest)
            .iter()
            .take(limit)
            .map(InstalledModule::from)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.installed_modules",
            requested_limit,
            limit as u64,
            modules.len(),
        );

        Ok(modules)
    }

    async fn marketplace(
        &self,
        ctx: &Context<'_>,
        search: Option<String>,
        category: Option<String>,
        source: Option<String>,
        trust_level: Option<String>,
        only_compatible: Option<bool>,
        installed_only: Option<bool>,
        limit: Option<i32>,
    ) -> Result<Vec<MarketplaceModule>> {
        ensure_modules_read_permission(ctx).await?;

        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);
        let manifest = ManifestManager::load()
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        let installed_modules = ManifestManager::installed_modules(&manifest);
        let search = search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let category = category
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let trust_level = trust_level
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let source = source
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let only_compatible = only_compatible.unwrap_or(true);
        let installed_only = installed_only.unwrap_or(false);

        let modules = marketplace_modules_from_catalog(
            load_marketplace_catalog(app_ctx, &manifest, registry).await?,
            registry,
            &installed_modules,
        )
        .into_iter()
        .filter(|module| module.kind == "optional")
        .filter(|module| !only_compatible || module.compatible || module.installed)
        .filter(|module| !installed_only || module.installed)
        .filter(|module| trust_level_matches(module, trust_level))
        .filter(|module| source_matches(module, source))
        .filter(|module| {
            category
                .as_ref()
                .is_none_or(|category| module.category.eq_ignore_ascii_case(category))
        })
        .filter(|module| {
            search.is_none_or(|search| {
                let search = search.to_lowercase();
                module.slug.to_lowercase().contains(&search)
                    || module.name.to_lowercase().contains(&search)
                    || module.description.to_lowercase().contains(&search)
                    || module.crate_name.to_lowercase().contains(&search)
            })
        })
        .take(limit)
        .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.marketplace",
            requested_limit,
            limit as u64,
            modules.len(),
        );

        Ok(modules)
    }

    async fn marketplace_module(
        &self,
        ctx: &Context<'_>,
        slug: String,
    ) -> Result<Option<MarketplaceModule>> {
        ensure_modules_read_permission(ctx).await?;

        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;
        let manifest = ManifestManager::load()
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        let installed_modules = ManifestManager::installed_modules(&manifest);
        let slug = slug.trim().to_lowercase();

        Ok(marketplace_modules_from_catalog(
            load_marketplace_catalog(app_ctx, &manifest, registry).await?,
            registry,
            &installed_modules,
        )
        .into_iter()
        .find(|module| module.slug.eq_ignore_ascii_case(&slug)))
    }

    async fn active_build(&self, ctx: &Context<'_>) -> Result<Option<BuildJob>> {
        ensure_modules_read_permission(ctx).await?;

        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let build = BuildService::new(app_ctx.db.clone())
            .active_build()
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(build.as_ref().map(BuildJob::from_model))
    }

    async fn build_history(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 20)] limit: i32,
        #[graphql(default = 0)] offset: i32,
    ) -> Result<Vec<BuildJob>> {
        ensure_modules_read_permission(ctx).await?;

        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let requested_limit = limit.max(0) as u64;
        let limit = limit.clamp(1, 100) as u64;
        let offset = offset.max(0) as u64;

        let builds = BuildEntity::find()
            .order_by_desc(BuildColumn::CreatedAt)
            .offset(offset)
            .limit(limit)
            .all(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let builds = builds.iter().map(BuildJob::from_model).collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.build_history",
            Some(requested_limit),
            limit,
            builds.len(),
        );

        Ok(builds)
    }

    async fn active_release(&self, ctx: &Context<'_>) -> Result<Option<ReleaseInfo>> {
        ensure_modules_read_permission(ctx).await?;

        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let release = ReleaseEntity::find()
            .filter(ReleaseColumn::Status.eq(ReleaseStatus::Active))
            .order_by_desc(ReleaseColumn::UpdatedAt)
            .one(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(release.as_ref().map(ReleaseInfo::from_model))
    }

    async fn release_history(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 20)] limit: i32,
        #[graphql(default = 0)] offset: i32,
    ) -> Result<Vec<ReleaseInfo>> {
        ensure_modules_read_permission(ctx).await?;

        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let requested_limit = limit.max(0) as u64;
        let limit = limit.clamp(1, 100) as u64;
        let offset = offset.max(0) as u64;

        let releases = ReleaseEntity::find()
            .order_by_desc(ReleaseColumn::CreatedAt)
            .offset(offset)
            .limit(limit)
            .all(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let releases = releases
            .iter()
            .map(ReleaseInfo::from_model)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.release_history",
            Some(requested_limit),
            limit,
            releases.len(),
        );

        Ok(releases)
    }

    async fn me(&self, ctx: &Context<'_>) -> Result<Option<User>> {
        let auth = match ctx.data_opt::<AuthContext>() {
            Some(auth) => auth,
            None => return Ok(None),
        };
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let user = users::Entity::find()
            .filter(UsersColumn::Id.eq(auth.user_id))
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .one(&app_ctx.db)
            .await
            .map_err(|err| err.to_string())?;

        Ok(user.as_ref().map(User::from))
    }

    async fn user(&self, ctx: &Context<'_>, id: uuid::Uuid) -> Result<Option<User>> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;

        let can_read_users = RbacService::has_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &rustok_core::Permission::USERS_READ,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_read_users {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: users:read required",
            ));
        }

        let user = users::Entity::find_by_id(id)
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .one(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(user.as_ref().map(User::from))
    }

    async fn users(
        &self,
        ctx: &Context<'_>,
        #[graphql(default)] pagination: PaginationInput,
        filter: Option<UsersFilter>,
        search: Option<String>,
    ) -> Result<UserConnection> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;

        let can_list_users = RbacService::has_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &rustok_core::Permission::USERS_LIST,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_list_users {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: users:list required",
            ));
        }

        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let mut query = users::Entity::find().filter(UsersColumn::TenantId.eq(tenant.id));

        if let Some(filter) = filter {
            if let Some(role) = filter.role {
                let role: rustok_core::UserRole = role.into();
                let user_ids = RbacService::get_user_ids_for_role(&app_ctx.db, &tenant.id, role)
                    .await
                    .map_err(|err| {
                        <FieldError as GraphQLError>::internal_error(&err.to_string())
                    })?;
                query = query.filter(UsersColumn::Id.is_in(user_ids));
            }

            if let Some(status) = filter.status {
                let status: rustok_core::UserStatus = status.into();
                query = query.filter(UsersColumn::Status.eq(status.to_string()));
            }
        }

        if let Some(search) = search {
            let search = search.trim();
            if !search.is_empty() {
                let condition = Condition::any()
                    .add(UsersColumn::Email.contains(search))
                    .add(UsersColumn::Name.contains(search));
                query = query.filter(condition);
            }
        }
        let count_started_at = Instant::now();
        let total = query
            .clone()
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;
        metrics::record_read_path_query(
            "graphql",
            "root.users",
            "count",
            count_started_at.elapsed().as_secs_f64(),
            total.max(0) as u64,
        );

        let page_started_at = Instant::now();
        let users = query
            .offset(offset as u64)
            .limit(limit as u64)
            .all(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        metrics::record_read_path_query(
            "graphql",
            "root.users",
            "users_page",
            page_started_at.elapsed().as_secs_f64(),
            users.len() as u64,
        );

        let edges = users
            .iter()
            .enumerate()
            .map(|(index, user)| UserEdge {
                node: User::from(user),
                cursor: encode_cursor(offset + index as i64),
            })
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.users",
            Some(requested_limit),
            limit as u64,
            edges.len(),
        );

        Ok(UserConnection {
            edges,
            page_info: PageInfo::new(total, offset, limit),
        })
    }

    async fn dashboard_stats(&self, ctx: &Context<'_>) -> Result<DashboardStats> {
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let now = Utc::now();
        let current_period_start = now - Duration::days(30);
        let previous_period_start = current_period_start - Duration::days(30);

        let user_stats_started_at = Instant::now();
        let user_stats = load_period_count_snapshot(
            &app_ctx.db,
            "users",
            tenant.id,
            current_period_start,
            previous_period_start,
            None,
            None,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        metrics::record_read_path_query(
            "graphql",
            "root.dashboard_stats",
            "users_snapshot",
            user_stats_started_at.elapsed().as_secs_f64(),
            user_stats.total_count.max(0) as u64,
        );

        let post_stats_started_at = Instant::now();
        let post_stats = load_period_count_snapshot(
            &app_ctx.db,
            "nodes",
            tenant.id,
            current_period_start,
            previous_period_start,
            Some(match app_ctx.db.get_database_backend() {
                DbBackend::Sqlite => " AND kind = ?4",
                _ => " AND kind = $4",
            }),
            Some("post"),
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        metrics::record_read_path_query(
            "graphql",
            "root.dashboard_stats",
            "posts_snapshot",
            post_stats_started_at.elapsed().as_secs_f64(),
            post_stats.total_count.max(0) as u64,
        );

        let order_stats_started_at = Instant::now();
        let order_stats = load_order_stats_snapshot(
            &app_ctx.db,
            tenant.id,
            current_period_start,
            previous_period_start,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        metrics::record_read_path_query(
            "graphql",
            "root.dashboard_stats",
            "orders_snapshot",
            order_stats_started_at.elapsed().as_secs_f64(),
            order_stats.total_orders.max(0) as u64,
        );

        Ok(DashboardStats {
            total_users: user_stats.total_count,
            total_posts: post_stats.total_count,
            total_orders: order_stats.total_orders,
            total_revenue: order_stats.total_revenue,
            users_change: calculate_percent_change(
                user_stats.current_count,
                user_stats.previous_count,
            ),
            posts_change: calculate_percent_change(
                post_stats.current_count,
                post_stats.previous_count,
            ),
            orders_change: calculate_percent_change(
                order_stats.current_orders,
                order_stats.previous_orders,
            ),
            revenue_change: calculate_percent_change(
                order_stats.current_revenue,
                order_stats.previous_revenue,
            ),
        })
    }

    async fn recent_activity(
        &self,
        ctx: &Context<'_>,
        #[graphql(default)] limit: i64,
    ) -> Result<Vec<ActivityItem>> {
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let requested_limit = limit.max(0) as u64;
        let limit = limit.clamp(1, 50);

        let recent_users_started_at = Instant::now();
        let recent_users = users::Entity::find()
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .order_by_desc(UsersColumn::CreatedAt)
            .limit(limit as u64)
            .all(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        metrics::record_read_path_query(
            "graphql",
            "root.recent_activity",
            "recent_users",
            recent_users_started_at.elapsed().as_secs_f64(),
            recent_users.len() as u64,
        );

        let activities = recent_users
            .into_iter()
            .map(|user| ActivityItem {
                id: user.id.to_string(),
                r#type: "user.created".to_string(),
                description: format!("New user {} joined", user.email),
                timestamp: user.created_at.to_rfc3339(),
                user: Some(ActivityUser {
                    id: user.id.to_string(),
                    name: user.name,
                }),
            })
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.recent_activity",
            Some(requested_limit),
            limit as u64,
            activities.len(),
        );

        Ok(activities)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_catalog_module_compatible, marketplace_module_from_catalog_entry, normalize_version_req,
        source_matches, trust_level_matches,
    };
    use crate::graphql::types::MarketplaceModule;
    use crate::modules::{CatalogManifestModule, InstalledManifestModule};
    use rustok_core::ModuleRegistry;
    use std::collections::HashMap;

    fn catalog_module(min: Option<&str>, max: Option<&str>) -> CatalogManifestModule {
        CatalogManifestModule {
            slug: "seo".to_string(),
            source: "registry".to_string(),
            crate_name: "rustok-seo".to_string(),
            name: None,
            category: None,
            tags: Vec::new(),
            version: Some("1.2.0".to_string()),
            description: None,
            git: None,
            rev: None,
            path: None,
            required: false,
            depends_on: Vec::new(),
            ownership: "third_party".to_string(),
            trust_level: "unverified".to_string(),
            rustok_min_version: min.map(str::to_string),
            rustok_max_version: max.map(str::to_string),
            publisher: None,
            checksum_sha256: None,
            signature: None,
            versions: Vec::new(),
            recommended_admin_surfaces: Vec::new(),
            showcase_admin_surfaces: Vec::new(),
            settings_schema: HashMap::new(),
        }
    }

    #[test]
    fn normalize_version_req_adds_bounds_for_plain_versions() {
        assert_eq!(normalize_version_req("0.5.0", false), ">= 0.5.0");
        assert_eq!(normalize_version_req("1.0.0", true), "<= 1.0.0");
        assert_eq!(normalize_version_req("1.x", true), "1.*");
    }

    #[test]
    fn compatibility_accepts_unbounded_catalog_entry() {
        assert!(is_catalog_module_compatible(&catalog_module(None, None)));
    }

    #[test]
    fn compatibility_rejects_entry_above_current_platform_max() {
        assert!(!is_catalog_module_compatible(&catalog_module(
            None,
            Some("0.0.1")
        )));
    }

    #[test]
    fn trust_level_filter_matches_case_insensitively() {
        let module = MarketplaceModule {
            slug: "seo".to_string(),
            name: "SEO".to_string(),
            latest_version: "1.2.0".to_string(),
            description: "SEO tools".to_string(),
            source: "registry".to_string(),
            kind: "optional".to_string(),
            category: "extensions".to_string(),
            tags: Vec::new(),
            icon_url: None,
            banner_url: None,
            screenshots: Vec::new(),
            crate_name: "rustok-seo".to_string(),
            dependencies: Vec::new(),
            ownership: "third_party".to_string(),
            trust_level: "verified".to_string(),
            rustok_min_version: None,
            rustok_max_version: None,
            publisher: None,
            checksum_sha256: None,
            signature_present: false,
            versions: Vec::new(),
            compatible: true,
            recommended_admin_surfaces: Vec::new(),
            showcase_admin_surfaces: Vec::new(),
            settings_schema: Vec::new(),
            installed: false,
            installed_version: None,
            update_available: false,
        };

        assert!(trust_level_matches(&module, Some("VERIFIED")));
        assert!(!trust_level_matches(&module, Some("community")));
    }

    #[test]
    fn source_filter_matches_case_insensitively() {
        let module = MarketplaceModule {
            slug: "seo".to_string(),
            name: "SEO".to_string(),
            latest_version: "1.2.0".to_string(),
            description: "SEO tools".to_string(),
            source: "registry".to_string(),
            kind: "optional".to_string(),
            category: "extensions".to_string(),
            tags: Vec::new(),
            icon_url: None,
            banner_url: None,
            screenshots: Vec::new(),
            crate_name: "rustok-seo".to_string(),
            dependencies: Vec::new(),
            ownership: "third_party".to_string(),
            trust_level: "verified".to_string(),
            rustok_min_version: None,
            rustok_max_version: None,
            publisher: None,
            checksum_sha256: None,
            signature_present: false,
            versions: Vec::new(),
            compatible: true,
            recommended_admin_surfaces: Vec::new(),
            showcase_admin_surfaces: Vec::new(),
            settings_schema: Vec::new(),
            installed: false,
            installed_version: None,
            update_available: false,
        };

        assert!(source_matches(&module, Some("REGISTRY")));
        assert!(!source_matches(&module, Some("path")));
    }

    #[test]
    fn marketplace_mapping_uses_catalog_name_and_description_without_runtime_module() {
        let mut entry = catalog_module(None, None);
        entry.name = Some("SEO Toolkit".to_string());
        entry.description = Some("Search optimization bundle".to_string());

        let module = marketplace_module_from_catalog_entry(
            entry,
            &ModuleRegistry::new(),
            &Vec::<InstalledManifestModule>::new(),
        );

        assert_eq!(module.name, "SEO Toolkit");
        assert_eq!(module.description, "Search optimization bundle");
    }

    #[test]
    fn marketplace_mapping_prefers_manifest_category_when_present() {
        let mut entry = catalog_module(None, None);
        entry.slug = "search".to_string();
        entry.category = Some("search".to_string());

        let module = marketplace_module_from_catalog_entry(
            entry,
            &ModuleRegistry::new(),
            &Vec::<InstalledManifestModule>::new(),
        );

        assert_eq!(module.category, "search");
    }

    #[test]
    fn marketplace_mapping_preserves_manifest_tags() {
        let mut entry = catalog_module(None, None);
        entry.tags = vec!["discovery".to_string(), "search".to_string()];

        let module = marketplace_module_from_catalog_entry(
            entry,
            &ModuleRegistry::new(),
            &Vec::<InstalledManifestModule>::new(),
        );

        assert_eq!(module.tags, vec!["discovery", "search"]);
    }

    #[test]
    fn marketplace_mapping_preserves_visual_metadata() {
        let mut entry = catalog_module(None, None);
        entry.icon_url = Some("https://cdn.example.test/modules/seo/icon.svg".to_string());
        entry.banner_url = Some("https://cdn.example.test/modules/seo/banner.png".to_string());
        entry.screenshots = vec![
            "https://cdn.example.test/modules/seo/screenshot-1.png".to_string(),
            "https://cdn.example.test/modules/seo/screenshot-2.png".to_string(),
        ];

        let module = marketplace_module_from_catalog_entry(
            entry,
            &ModuleRegistry::new(),
            &Vec::<InstalledManifestModule>::new(),
        );

        assert_eq!(
            module.icon_url.as_deref(),
            Some("https://cdn.example.test/modules/seo/icon.svg")
        );
        assert_eq!(
            module.banner_url.as_deref(),
            Some("https://cdn.example.test/modules/seo/banner.png")
        );
        assert_eq!(
            module.screenshots,
            vec![
                "https://cdn.example.test/modules/seo/screenshot-1.png",
                "https://cdn.example.test/modules/seo/screenshot-2.png",
            ]
        );
    }

    #[test]
    fn marketplace_mapping_falls_back_to_legacy_slug_category_when_manifest_category_missing() {
        let mut entry = catalog_module(None, None);
        entry.slug = "search".to_string();

        let module = marketplace_module_from_catalog_entry(
            entry,
            &ModuleRegistry::new(),
            &Vec::<InstalledManifestModule>::new(),
        );

        assert_eq!(module.category, "extensions");
    }
}
