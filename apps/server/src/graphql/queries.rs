use std::collections::{HashMap, HashSet};

use async_graphql::{Context, FieldError, Object, Result};
use chrono::{Duration, Utc};
use rustok_content::entities::node::{Column as NodesColumn, Entity as NodesEntity};
use rustok_core::{Action, ModuleRegistry, Permission, Resource};
use rustok_outbox::entity::{Column as SysEventsColumn, Entity as SysEventsEntity};
use sea_orm::{
    sea_query::Expr, ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use semver::{Version, VersionReq};

use crate::context::{AuthContext, TenantContext};
use crate::graphql::common::{encode_cursor, PageInfo, PaginationInput};
use crate::graphql::errors::GraphQLError;
use crate::graphql::types::{
    ActivityItem, ActivityUser, BuildJob, DashboardStats, InstalledModule, MarketplaceModule,
    MarketplaceModuleVersion, ModuleRegistryItem, ReleaseInfo, Tenant, TenantModule, User,
    UserConnection, UserEdge, UsersFilter,
};
use crate::models::_entities::tenant_modules::Column as TenantModulesColumn;
use crate::models::_entities::tenant_modules::Entity as TenantModulesEntity;
use crate::models::_entities::users::Column as UsersColumn;
use crate::models::build::{Column as BuildColumn, Entity as BuildEntity};
use crate::models::release::{Column as ReleaseColumn, Entity as ReleaseEntity, ReleaseStatus};
use crate::models::users;
use crate::modules::ManifestManager;
use crate::services::auth::AuthService;
use crate::services::build_service::BuildService;
use crate::services::marketplace_catalog::marketplace_catalog_from_context;

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

fn parse_order_total(payload: &serde_json::Value) -> Option<i64> {
    payload
        .get("event")
        .and_then(|event| event.get("data"))
        .and_then(|data| data.get("total"))
        .and_then(serde_json::Value::as_i64)
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

fn module_category(slug: &str) -> &'static str {
    match slug {
        "content" | "blog" | "forum" | "pages" => "content",
        "commerce" => "commerce",
        "alloy" => "automation",
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
            .unwrap_or_else(|| humanize_slug(&entry.slug)),
        latest_version: latest_version.clone(),
        description: runtime_module
            .map(|module| module.description().to_string())
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
        category: module_category(&entry.slug).to_string(),
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
        installed: installed_module.is_some(),
        installed_version: installed_version.clone(),
        update_available: installed_version
            .as_ref()
            .is_some_and(|version| version != &latest_version),
    }
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
    let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
    let tenant = ctx.data::<TenantContext>()?;

    let can_read_modules = AuthService::has_any_permission(
        &app_ctx.db,
        &tenant.id,
        &auth.user_id,
        &[
            Permission::new(Resource::Modules, Action::Read),
            Permission::new(Resource::Modules, Action::List),
            Permission::new(Resource::Modules, Action::Manage),
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
    app_ctx: &loco_rs::prelude::AppContext,
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

    async fn enabled_modules(&self, ctx: &Context<'_>) -> Result<Vec<String>> {
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let modules = TenantModulesEntity::find_enabled(&app_ctx.db, tenant.id)
            .await
            .map_err(|err| err.to_string())?;

        Ok(modules)
    }

    async fn module_registry(&self, ctx: &Context<'_>) -> Result<Vec<ModuleRegistryItem>> {
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;
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

        Ok(registry
            .list()
            .into_iter()
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
                }
            })
            .collect())
    }

    async fn tenant_modules(&self, ctx: &Context<'_>) -> Result<Vec<TenantModule>> {
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let modules = TenantModulesEntity::find()
            .filter(TenantModulesColumn::TenantId.eq(tenant.id))
            .all(&app_ctx.db)
            .await
            .map_err(|err| err.to_string())?;

        Ok(modules
            .into_iter()
            .map(|module| TenantModule {
                module_slug: module.module_slug,
                enabled: module.enabled,
                settings: module.settings.to_string(),
            })
            .collect())
    }

    async fn installed_modules(&self, ctx: &Context<'_>) -> Result<Vec<InstalledModule>> {
        ensure_modules_read_permission(ctx).await?;

        let manifest = ManifestManager::load()
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(ManifestManager::installed_modules(&manifest)
            .iter()
            .map(InstalledModule::from)
            .collect())
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
    ) -> Result<Vec<MarketplaceModule>> {
        ensure_modules_read_permission(ctx).await?;

        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;
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

        Ok(marketplace_modules_from_catalog(
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
        .collect())
    }

    async fn marketplace_module(
        &self,
        ctx: &Context<'_>,
        slug: String,
    ) -> Result<Option<MarketplaceModule>> {
        ensure_modules_read_permission(ctx).await?;

        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
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

        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
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

        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let limit = limit.clamp(1, 100) as u64;
        let offset = offset.max(0) as u64;

        let builds = BuildEntity::find()
            .order_by_desc(BuildColumn::CreatedAt)
            .offset(offset)
            .limit(limit)
            .all(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(builds.iter().map(BuildJob::from_model).collect())
    }

    async fn active_release(&self, ctx: &Context<'_>) -> Result<Option<ReleaseInfo>> {
        ensure_modules_read_permission(ctx).await?;

        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
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

        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let limit = limit.clamp(1, 100) as u64;
        let offset = offset.max(0) as u64;

        let releases = ReleaseEntity::find()
            .order_by_desc(ReleaseColumn::CreatedAt)
            .offset(offset)
            .limit(limit)
            .all(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(releases.iter().map(ReleaseInfo::from_model).collect())
    }

    async fn me(&self, ctx: &Context<'_>) -> Result<Option<User>> {
        let auth = match ctx.data_opt::<AuthContext>() {
            Some(auth) => auth,
            None => return Ok(None),
        };
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
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
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        let can_read_users = AuthService::has_permission(
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
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        let can_list_users = AuthService::has_permission(
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

        let (offset, limit) = pagination.normalize()?;
        let mut query = users::Entity::find().filter(UsersColumn::TenantId.eq(tenant.id));

        if let Some(filter) = filter {
            if let Some(role) = filter.role {
                let role: rustok_core::UserRole = role.into();
                query = query.filter(UsersColumn::Role.eq(role.to_string()));
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
        let total = query
            .clone()
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;
        let users = query
            .offset(offset as u64)
            .limit(limit as u64)
            .all(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let edges = users
            .iter()
            .enumerate()
            .map(|(index, user)| UserEdge {
                node: User::from(user),
                cursor: encode_cursor(offset + index as i64),
            })
            .collect();

        Ok(UserConnection {
            edges,
            page_info: PageInfo::new(total, offset, limit),
        })
    }

    async fn dashboard_stats(&self, ctx: &Context<'_>) -> Result<DashboardStats> {
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let now = Utc::now();
        let current_period_start = now - Duration::days(30);
        let previous_period_start = current_period_start - Duration::days(30);

        let total_users = users::Entity::find()
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;

        let total_posts = NodesEntity::find()
            .filter(NodesColumn::TenantId.eq(tenant.id))
            .filter(NodesColumn::Kind.eq("post"))
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;

        let current_users = users::Entity::find()
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .filter(UsersColumn::CreatedAt.gte(current_period_start))
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;

        let previous_users = users::Entity::find()
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .filter(UsersColumn::CreatedAt.gte(previous_period_start))
            .filter(UsersColumn::CreatedAt.lt(current_period_start))
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;

        let current_posts = NodesEntity::find()
            .filter(NodesColumn::TenantId.eq(tenant.id))
            .filter(NodesColumn::Kind.eq("post"))
            .filter(NodesColumn::CreatedAt.gte(current_period_start))
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;

        let previous_posts = NodesEntity::find()
            .filter(NodesColumn::TenantId.eq(tenant.id))
            .filter(NodesColumn::Kind.eq("post"))
            .filter(NodesColumn::CreatedAt.gte(previous_period_start))
            .filter(NodesColumn::CreatedAt.lt(current_period_start))
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;

        let tenant_id_str = tenant.id.to_string();

        let order_events = SysEventsEntity::find()
            .filter(SysEventsColumn::EventType.eq("order.placed"))
            .filter(
                Condition::any()
                    .add(Expr::cust_with_values(
                        "payload->>'tenant_id' = $1",
                        [tenant_id_str.clone()],
                    ))
                    .add(Expr::cust_with_values(
                        "payload->'event'->>'tenant_id' = $1",
                        [tenant_id_str],
                    )),
            )
            .all(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let mut total_orders = 0i64;
        let mut total_revenue = 0i64;
        let mut current_orders = 0i64;
        let mut previous_orders = 0i64;
        let mut current_revenue = 0i64;
        let mut previous_revenue = 0i64;

        for event in order_events {
            let order_total = parse_order_total(&event.payload).unwrap_or(0);
            total_orders += 1;
            total_revenue += order_total;

            let created_at = event.created_at;
            if created_at >= current_period_start {
                current_orders += 1;
                current_revenue += order_total;
            } else if created_at >= previous_period_start {
                previous_orders += 1;
                previous_revenue += order_total;
            }
        }

        Ok(DashboardStats {
            total_users,
            total_posts,
            total_orders,
            total_revenue,
            users_change: calculate_percent_change(current_users, previous_users),
            posts_change: calculate_percent_change(current_posts, previous_posts),
            orders_change: calculate_percent_change(current_orders, previous_orders),
            revenue_change: calculate_percent_change(current_revenue, previous_revenue),
        })
    }

    async fn recent_activity(
        &self,
        ctx: &Context<'_>,
        #[graphql(default)] limit: i64,
    ) -> Result<Vec<ActivityItem>> {
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let limit = limit.clamp(1, 50);

        let recent_users = users::Entity::find()
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .order_by_desc(UsersColumn::CreatedAt)
            .limit(limit as u64)
            .all(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

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
            .collect();

        Ok(activities)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_catalog_module_compatible, normalize_version_req, source_matches, trust_level_matches,
    };
    use crate::graphql::types::MarketplaceModule;
    use crate::modules::CatalogManifestModule;

    fn catalog_module(min: Option<&str>, max: Option<&str>) -> CatalogManifestModule {
        CatalogManifestModule {
            slug: "seo".to_string(),
            source: "registry".to_string(),
            crate_name: "rustok-seo".to_string(),
            version: Some("1.2.0".to_string()),
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
            installed: false,
            installed_version: None,
            update_available: false,
        };

        assert!(source_matches(&module, Some("REGISTRY")));
        assert!(!source_matches(&module, Some("path")));
    }
}
