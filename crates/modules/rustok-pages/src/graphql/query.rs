use async_graphql::{Context, ErrorExtensions, Object, Result};
use rustok_api::{
    AuthContext, RequestContext, TenantContext, graphql::require_module_enabled,
    graphql::resolve_graphql_locale,
};
use rustok_channel::ChannelService;
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::time::Instant;
use uuid::Uuid;

use crate::cache_invalidation::{PublishedLandingRepresentation, published_landing_cache_variant};
use crate::services::page::is_page_visible_for_channel;
use crate::services::rbac::can_read_non_public_pages;
use crate::{
    PageBuilderArtifactService, PageCacheScope, PageService, PagesCacheReadRuntime, page_cache_key,
};

use super::runtime_data::PagesGraphqlRuntimeData;
use super::types::*;

const MODULE_SLUG: &str = "pages";

#[derive(Default)]
pub struct PagesQuery;

#[Object(name = "PagesDomainQuery")]
impl PagesQuery {
    async fn page(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        locale: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlPage>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_pages_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let security = request_security_context(ctx);
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = query_tenant_id(
            tenant,
            ctx.data_opt::<AuthContext>().map(|auth| auth.tenant_id),
            tenant_id,
        )?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = PageService::new(db.clone(), event_bus.clone());
        let page = match service
            .get_with_locale_fallback(
                tenant_id,
                security,
                id,
                &locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(page) => page,
            Err(crate::PagesError::PageNotFound(_)) => return Ok(None),
            // To a visitor an unpublished page does not exist; a permission error would
            // confirm that the id is taken.
            Err(crate::PagesError::Forbidden(_)) if is_public_request(ctx) => return Ok(None),
            Err(err) => return Err(async_graphql::Error::new(err.to_string())),
        };

        if is_public_request(ctx)
            && (page.status != rustok_content::entities::node::ContentStatus::Published
                || !is_page_visible_for_request(
                    &page.channel_slugs,
                    public_channel_slug(ctx).as_deref(),
                    false,
                ))
        {
            return Ok(None);
        }

        Ok(Some(page.into()))
    }

    async fn page_by_slug(
        &self,
        ctx: &Context<'_>,
        locale: Option<String>,
        slug: String,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlPage>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_pages_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let security = request_security_context(ctx);
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = query_tenant_id(
            tenant,
            ctx.data_opt::<AuthContext>().map(|auth| auth.tenant_id),
            tenant_id,
        )?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .get_by_slug_with_locale_fallback(
                tenant_id,
                security,
                &locale,
                &slug,
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        let public_channel_slug = public_channel_slug(ctx);
        Ok(page
            .filter(|page| {
                is_page_visible_for_request(
                    &page.channel_slugs,
                    public_channel_slug.as_deref(),
                    !is_public_request(ctx),
                )
            })
            .map(Into::into))
    }

    async fn pages(
        &self,
        ctx: &Context<'_>,
        filter: Option<ListGqlPagesFilter>,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPageList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_pages_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let security = request_security_context(ctx);
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = query_tenant_id(
            tenant,
            ctx.data_opt::<AuthContext>().map(|auth| auth.tenant_id),
            tenant_id,
        )?;

        let filter = filter.unwrap_or(ListGqlPagesFilter {
            locale: None,
            template: None,
            page: Some(1),
            per_page: Some(20),
        });

        if is_public_request(ctx) {
            return list_public_visible_pages(
                db,
                event_bus,
                tenant_id,
                filter,
                tenant.default_locale.as_str(),
                public_channel_slug(ctx).as_deref(),
            )
            .await;
        }

        let requested_limit = filter.per_page;
        let locale = resolve_graphql_locale(ctx, filter.locale.as_deref());

        let service = PageService::new(db.clone(), event_bus.clone());
        let list_started_at = Instant::now();
        let (items, total) = service
            .list(
                tenant_id,
                security,
                crate::ListPagesFilter {
                    status: None,
                    template: filter.template,
                    locale: Some(locale),
                    page: filter.page.unwrap_or(1),
                    per_page: filter.per_page.unwrap_or(20),
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        metrics::record_read_path_query(
            "graphql",
            "pages.pages",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );
        let items = items.into_iter().map(Into::into).collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "pages.pages",
            requested_limit,
            filter.per_page.unwrap_or(20).min(100),
            items.len(),
        );

        Ok(GqlPageList { items, total })
    }

    /// Append-only body-revision journal metadata for one page locale, newest first.
    ///
    /// Editor state: the service rejects readers that may not read the document of the page
    /// (neither save nor publish authority over it).
    async fn page_body_revisions(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        locale: String,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<GqlPageBodyRevision>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let security = request_security_context(ctx);
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = query_tenant_id(
            tenant,
            ctx.data_opt::<AuthContext>().map(|auth| auth.tenant_id),
            tenant_id,
        )?;

        PageService::new(db.clone(), event_bus.clone())
            .body_revision_history(tenant_id, security, id, &locale)
            .await
            .map(|revisions| revisions.into_iter().map(Into::into).collect())
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }

    /// Pages whose working or current bodies depend on a shared site symbol.
    /// Includes transitive dependencies through other symbol definitions.
    async fn site_symbol_usage(
        &self,
        ctx: &Context<'_>,
        symbol_id: String,
        locale: String,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<Uuid>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let security = request_security_context(ctx);
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = query_tenant_id(
            tenant,
            ctx.data_opt::<AuthContext>().map(|auth| auth.tenant_id),
            tenant_id,
        )?;
        PageService::new(db.clone(), event_bus.clone())
            .site_symbol_usage(tenant_id, security, &locale, &symbol_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }

    /// Most recent scheduled publication job of one page, whatever its state.
    ///
    /// `null` means the page was never scheduled.
    async fn page_publish_schedule(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlPagePublishSchedule>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let security = request_security_context(ctx);
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = query_tenant_id(
            tenant,
            ctx.data_opt::<AuthContext>().map(|auth| auth.tenant_id),
            tenant_id,
        )?;

        PageService::new(db.clone(), event_bus.clone())
            .page_publish_schedule(tenant_id, security, id)
            .await
            .map(|schedule| schedule.map(Into::into))
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }
}

/// Resolves `GqlPage.landing`: the published landing a visitor of the request channel gets.
///
/// Page status, channel visibility, the locale binding and artifact integrity are decided by
/// the artifact owner in one transaction; nothing is derived from the page body.
pub(super) async fn published_landing(
    ctx: &Context<'_>,
    page: &GqlPage,
) -> Result<Option<GqlPublishedLanding>> {
    let Some(locale) = page
        .effective_locale
        .as_deref()
        .or(page.requested_locale.as_deref())
    else {
        return Ok(None);
    };
    let db = ctx.data::<DatabaseConnection>()?;
    let tenant = ctx.data::<TenantContext>()?;
    load_published_landing(
        db,
        ctx.data_opt::<PagesGraphqlRuntimeData>()
            .and_then(PagesGraphqlRuntimeData::cache),
        tenant.id,
        page.id,
        locale,
        tenant.default_locale.as_str(),
        public_channel_slug(ctx).as_deref(),
    )
    .await
}

/// Reads the published landing through the generation-aware Pages cache.
///
/// The landing is the same for every reader of one tenant, page, locale and channel, so the
/// entry carries no principal. Its key rotates with every page event (update, publish,
/// unpublish, delete), not only with the events that replace the artifact; see
/// [`published_landing_cache_variant`].
///
/// Only a resolved landing is stored. "No landing" is cheap to recompute and must never
/// outlive a publish. Every cache failure degrades to the owner read.
async fn load_published_landing(
    db: &DatabaseConnection,
    cache: Option<&PagesCacheReadRuntime>,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    fallback_locale: &str,
    channel_slug: Option<&str>,
) -> Result<Option<GqlPublishedLanding>> {
    let cache_key = match cache {
        Some(cache) => published_landing_cache_key(
            cache,
            tenant_id,
            page_id,
            locale,
            fallback_locale,
            channel_slug,
        )
        .await
        .map(|key| (cache, key)),
        None => None,
    };

    if let Some((cache, key)) = cache_key.as_ref() {
        match cache.get_json::<GqlPublishedLanding>(key).await {
            Ok(Some(landing)) => {
                tracing::debug!(%tenant_id, %page_id, "Pages landing cache hit");
                return Ok(Some(landing));
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    %error,
                    %tenant_id,
                    %page_id,
                    "Pages landing cache read failed; loading the published artifact"
                );
            }
        }
    }

    let landing = PageBuilderArtifactService::new(db.clone())
        .load_public_bound_artifact_with_fallback(
            tenant_id,
            page_id,
            locale,
            Some(fallback_locale),
            channel_slug,
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?
        .map(GqlPublishedLanding::from);

    if let (Some((cache, key)), Some(landing)) = (cache_key, landing.as_ref())
        && let Err(error) = cache.put_json(key, landing).await
    {
        tracing::warn!(
            %error,
            %tenant_id,
            %page_id,
            "Pages landing cache fill failed"
        );
    }
    Ok(landing)
}

async fn published_landing_cache_key(
    cache: &PagesCacheReadRuntime,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    fallback_locale: &str,
    channel_slug: Option<&str>,
) -> Option<String> {
    let generations = match cache.generation_snapshot(tenant_id).await {
        Ok(generations) => generations,
        Err(error) => {
            tracing::warn!(
                %error,
                %tenant_id,
                %page_id,
                "Pages landing generation read failed; bypassing cache"
            );
            return None;
        }
    };
    let variant = published_landing_cache_variant(
        PublishedLandingRepresentation::Landing,
        generations,
        locale,
        fallback_locale,
        channel_slug,
    );
    match page_cache_key(
        PageCacheScope::Artifact,
        tenant_id,
        page_id,
        generations.artifact,
        variant.as_str(),
    ) {
        Ok(key) => Some(key),
        Err(error) => {
            tracing::warn!(
                %error,
                %tenant_id,
                %page_id,
                "Pages landing cache key rejected"
            );
            None
        }
    }
}

fn query_tenant_id(
    tenant: &TenantContext,
    authenticated_tenant: Option<Uuid>,
    requested: Option<Uuid>,
) -> Result<Uuid> {
    if authenticated_tenant.is_some_and(|authenticated_tenant| authenticated_tenant != tenant.id)
        || requested.is_some_and(|requested| requested != tenant.id)
    {
        return Err(async_graphql::Error::new(
            "Pages reads must use the current tenant",
        ));
    }
    Ok(tenant.id)
}

fn request_security_context(ctx: &Context<'_>) -> SecurityContext {
    ctx.data_opt::<AuthContext>()
        .map(|auth| {
            rustok_core::security_context_from_access_token(
                auth.user_id,
                &auth.grant_type,
                &auth.permissions,
            )
        })
        .unwrap_or_else(SecurityContext::public_read)
}

/// A request is a visitor request unless its principal authors pages.
///
/// Signing in does not make a reader an author: a customer token carries `pages:read` and
/// `pages:list`, exactly what an anonymous reader holds. Such a request gets the published
/// pages of its channel and nothing else, like an anonymous one.
fn is_public_request(ctx: &Context<'_>) -> bool {
    !can_read_non_public_pages(&request_security_context(ctx))
}

fn public_channel_slug(ctx: &Context<'_>) -> Option<String> {
    ctx.data_opt::<RequestContext>()
        .and_then(|request_context| request_context.channel_slug.clone())
        .map(|slug| slug.trim().to_ascii_lowercase())
        .filter(|slug| !slug.is_empty())
}

fn request_channel_label(request_context: &RequestContext) -> &str {
    request_context.channel_slug.as_deref().unwrap_or("current")
}

fn request_channel_resolution_source(request_context: &RequestContext) -> &str {
    request_context
        .channel_resolution_source
        .as_ref()
        .map(|source| source.as_str())
        .unwrap_or("unknown")
}

fn is_page_visible_for_request(
    channel_slugs: &[String],
    public_channel_slug: Option<&str>,
    is_author: bool,
) -> bool {
    is_author || is_page_visible_for_channel(channel_slugs, public_channel_slug)
}

async fn list_public_visible_pages(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    filter: ListGqlPagesFilter,
    default_locale: &str,
    public_channel_slug: Option<&str>,
) -> Result<GqlPageList> {
    let locale = resolve_graphql_locale_fallback(filter.locale.as_deref(), default_locale);
    let service = PageService::new(db.clone(), event_bus.clone());
    let (items, total) = service
        .list_public_visible_with_locale_fallback(
            tenant_id,
            crate::ListPagesFilter {
                status: Some(rustok_content::entities::node::ContentStatus::Published),
                template: filter.template.clone(),
                locale: Some(locale),
                page: filter.page.unwrap_or(1).max(1),
                per_page: filter.per_page.unwrap_or(20).clamp(1, 100),
            },
            Some(default_locale),
            public_channel_slug,
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
    let items = items.into_iter().map(Into::into).collect();

    Ok(GqlPageList { items, total })
}

fn resolve_graphql_locale_fallback(requested: Option<&str>, fallback: &str) -> String {
    requested
        .map(str::trim)
        .filter(|locale| !locale.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| fallback.to_string())
}

async fn require_public_pages_channel_enabled(ctx: &Context<'_>) -> Result<()> {
    let db = ctx.data::<DatabaseConnection>()?;
    ensure_public_pages_channel_enabled(
        db,
        ctx.data_opt::<RequestContext>(),
        !is_public_request(ctx),
    )
    .await
}

async fn ensure_public_pages_channel_enabled(
    db: &DatabaseConnection,
    request_context: Option<&RequestContext>,
    is_author: bool,
) -> Result<()> {
    if is_author {
        return Ok(());
    }

    let Some(request_context) = request_context else {
        return Ok(());
    };
    let Some(channel_id) = request_context.channel_id else {
        return Ok(());
    };

    let enabled = ChannelService::new(db.clone())
        .is_module_enabled(channel_id, MODULE_SLUG)
        .await
        .map_err(|error| {
            async_graphql::Error::new(format!("Channel module check failed: {error}"))
                .extend_with(|_, ext| ext.set("code", "INTERNAL_SERVER_ERROR"))
        })?;

    if enabled {
        return Ok(());
    }

    let channel_label = request_channel_label(request_context);
    let resolution_source = request_channel_resolution_source(request_context);
    Err(async_graphql::Error::new(format!(
        "Module '{MODULE_SLUG}' is not enabled for channel '{channel_label}' (resolved via {resolution_source})"
    ))
    .extend_with(|_, ext| ext.set("code", "MODULE_NOT_ENABLED")))
}

#[cfg(test)]
mod tests {
    use super::{ensure_public_pages_channel_enabled, is_page_visible_for_request};
    use crate::services::page::is_page_visible_for_channel;
    use rustok_api::{RequestContext, context::ChannelResolutionSource};
    use rustok_channel::{BindChannelModuleInput, ChannelService, CreateChannelInput, migrations};
    use rustok_test_utils::setup_test_db;
    use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
    use sea_orm_migration::SchemaManager;
    use uuid::Uuid;

    async fn setup_channel_db() -> DatabaseConnection {
        let db = setup_test_db().await;
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE tenants (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL UNIQUE,
                domain TEXT NULL UNIQUE,
                settings TEXT NOT NULL DEFAULT '{}',
                default_locale TEXT NOT NULL DEFAULT 'en',
                is_active BOOLEAN NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        ))
        .await
        .expect("tenants table should exist for channel foreign keys");
        let manager = SchemaManager::new(&db);
        for migration in migrations::migrations() {
            match migration.up(&manager).await {
                Ok(_) => {}
                Err(error)
                    if db.get_database_backend() == sea_orm::DatabaseBackend::Sqlite
                        && error.to_string().contains("require PostgreSQL") => {}
                Err(error) => panic!("channel migration should apply: {error:?}"),
            }
        }
        db
    }

    async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) {
        db.execute_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO tenants (id, name, slug, settings, default_locale, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [
                tenant_id.into(),
                format!("{slug} tenant").into(),
                slug.to_string().into(),
                "{}".to_string().into(),
                "en".to_string().into(),
                true.into(),
            ],
        ))
        .await
        .expect("tenant should be inserted");
    }

    fn request_context(channel_id: Uuid, channel_slug: &str) -> RequestContext {
        RequestContext {
            tenant_id: Uuid::new_v4(),
            user_id: None,
            channel_id: Some(channel_id),
            channel_slug: Some(channel_slug.to_string()),
            channel_resolution_source: Some(ChannelResolutionSource::Host),
            locale: "en".to_string(),
            correlation_id: "test-correlation-id".to_string(),
        }
    }

    #[tokio::test]
    async fn public_request_rejects_disabled_pages_channel_binding() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-web").await;
        let service = ChannelService::new(db.clone());
        let channel = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "web".to_string(),
                name: "Web".to_string(),
                settings: None,
            })
            .await
            .expect("channel should be created");
        service
            .bind_module(
                channel.id,
                BindChannelModuleInput {
                    module_slug: "pages".to_string(),
                    is_enabled: false,
                    settings: None,
                },
            )
            .await
            .expect("binding should be saved");

        let result = ensure_public_pages_channel_enabled(
            &db,
            Some(&request_context(channel.id, "web")),
            false,
        )
        .await;

        assert!(
            result.is_err(),
            "public read-path must be gated by channel binding"
        );
    }

    #[tokio::test]
    async fn page_author_bypasses_channel_module_gate() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-web").await;
        let service = ChannelService::new(db.clone());
        let channel = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "web".to_string(),
                name: "Web".to_string(),
                settings: None,
            })
            .await
            .expect("channel should be created");
        service
            .bind_module(
                channel.id,
                BindChannelModuleInput {
                    module_slug: "pages".to_string(),
                    is_enabled: false,
                    settings: None,
                },
            )
            .await
            .expect("binding should be saved");

        let result = ensure_public_pages_channel_enabled(
            &db,
            Some(&request_context(channel.id, "web")),
            true,
        )
        .await;

        assert!(result.is_ok(), "page authoring flows must not be blocked");
    }

    #[tokio::test]
    async fn public_request_allows_channel_without_explicit_pages_binding() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-web").await;
        let service = ChannelService::new(db.clone());
        let channel = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "web".to_string(),
                name: "Web".to_string(),
                settings: None,
            })
            .await
            .expect("channel should be created");

        let result = ensure_public_pages_channel_enabled(
            &db,
            Some(&request_context(channel.id, "web")),
            false,
        )
        .await;

        assert!(
            result.is_ok(),
            "missing binding should keep module enabled by default in v0"
        );
    }

    #[test]
    fn public_channel_slug_is_only_used_for_unauthenticated_requests() {
        let request_context = RequestContext {
            tenant_id: Uuid::new_v4(),
            user_id: None,
            channel_id: Some(Uuid::new_v4()),
            channel_slug: Some(" Web ".to_string()),
            channel_resolution_source: Some(ChannelResolutionSource::Query),
            locale: "en".to_string(),
            correlation_id: "test-correlation-id".to_string(),
        };

        let unauthenticated = request_context
            .channel_slug
            .as_ref()
            .map(|slug| slug.trim().to_ascii_lowercase());
        let visible = is_page_visible_for_channel(&["web".to_string()], unauthenticated.as_deref());
        assert!(visible);
        assert!(is_page_visible_for_channel(&[], unauthenticated.as_deref()));
    }

    #[test]
    fn page_author_bypasses_page_channel_allowlist() {
        let channel_slugs = vec!["web".to_string()];

        assert!(is_page_visible_for_request(&channel_slugs, None, true));
        assert!(!is_page_visible_for_request(&channel_slugs, None, false));
    }

    #[tokio::test]
    async fn disabled_binding_error_reports_resolution_source() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-web").await;
        let service = ChannelService::new(db.clone());
        let channel = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "web".to_string(),
                name: "Web".to_string(),
                settings: None,
            })
            .await
            .expect("channel should be created");
        service
            .bind_module(
                channel.id,
                BindChannelModuleInput {
                    module_slug: "pages".to_string(),
                    is_enabled: false,
                    settings: None,
                },
            )
            .await
            .expect("binding should be saved");

        let request_context = RequestContext {
            tenant_id,
            user_id: None,
            channel_id: Some(channel.id),
            channel_slug: Some("web".to_string()),
            channel_resolution_source: Some(ChannelResolutionSource::Host),
            locale: "en".to_string(),
            correlation_id: "test-correlation-id".to_string(),
        };

        let error = ensure_public_pages_channel_enabled(&db, Some(&request_context), false)
            .await
            .expect_err("disabled binding should be reported");

        assert!(
            error.message.contains("resolved via host"),
            "error must expose resolution source for diagnostics"
        );
    }
}
