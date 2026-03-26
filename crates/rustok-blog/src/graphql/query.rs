use async_graphql::{dataloader::DataLoader, Context, ErrorExtensions, Object, Result};
use rustok_api::{
    graphql::{require_module_enabled, resolve_graphql_locale},
    AuthContext, RequestContext, TenantContext,
};
use rustok_channel::ChannelService;
use rustok_content::NodeService;
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use rustok_profiles::{
    graphql::GqlProfileSummary, ProfileService, ProfileSummaryLoader, ProfileSummaryLoaderKey,
    ProfilesReader,
};
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use uuid::Uuid;

use crate::services::is_post_visible_for_channel;
use crate::{BlogError, PostService};

use super::types::*;

const MODULE_SLUG: &str = "blog";

#[derive(Default)]
pub struct BlogQuery;

#[Object]
impl BlogQuery {
    async fn post(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        locale: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlPost>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_blog_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = PostService::new(db.clone(), event_bus.clone());
        let post = match service
            .get_post_with_locale_fallback(
                tenant_id,
                id,
                &locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(post) => post,
            Err(BlogError::PostNotFound(_))
            | Err(BlogError::Content(rustok_content::ContentError::NodeNotFound(_))) => {
                return Ok(None);
            }
            Err(err) => return Err(async_graphql::Error::new(err.to_string())),
        };

        if is_public_request(ctx)
            && (post.status != crate::BlogPostStatus::Published
                || !is_post_visible_for_request(
                    &post.metadata,
                    public_channel_slug(ctx).as_deref(),
                    false,
                ))
        {
            return Ok(None);
        }

        let author_profiles = load_author_profiles_map(
            ctx,
            db,
            tenant_id,
            [Some(post.author_id)],
            locale.as_str(),
            tenant.default_locale.as_str(),
        )
        .await?;

        let author_profile = author_profiles.get(&post.author_id).cloned();
        Ok(Some(map_post(post, author_profile)))
    }

    async fn post_by_slug(
        &self,
        ctx: &Context<'_>,
        slug: String,
        locale: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlPost>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_blog_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = PostService::new(db.clone(), event_bus.clone());
        let post = service
            .get_post_by_slug_with_locale_fallback(
                tenant_id,
                &locale,
                &slug,
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        if let Some(post) = post.filter(|post| {
            is_post_visible_for_request(
                &post.metadata,
                public_channel_slug(ctx).as_deref(),
                !is_public_request(ctx),
            )
        }) {
            let author_profiles = load_author_profiles_map(
                ctx,
                db,
                tenant_id,
                [Some(post.author_id)],
                locale.as_str(),
                tenant.default_locale.as_str(),
            )
            .await?;

            let author_profile = author_profiles.get(&post.author_id).cloned();
            return Ok(Some(map_post(post, author_profile)));
        }

        Ok(None)
    }

    async fn posts(
        &self,
        ctx: &Context<'_>,
        filter: Option<PostsFilter>,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPostList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_blog_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let filter = filter.unwrap_or(PostsFilter {
            status: None,
            author_id: None,
            locale: None,
            page: Some(1),
            per_page: Some(20),
        });

        if is_public_request(ctx) {
            return list_public_visible_posts(
                ctx,
                db,
                event_bus,
                tenant.id,
                tenant.default_locale.as_str(),
                filter,
                public_channel_slug(ctx).as_deref(),
            )
            .await;
        }

        let requested_limit = filter.per_page;
        let locale = resolve_graphql_locale(ctx, filter.locale.as_deref());
        let effective_limit = filter.per_page.unwrap_or(20).clamp(1, 100);

        let domain_filter = rustok_content::dto::ListNodesFilter {
            kind: Some("post".to_string()),
            status: filter.status.map(Into::into),
            parent_id: None,
            author_id: filter.author_id,
            locale: Some(locale.clone()),
            category_id: None,
            page: filter.page.unwrap_or(1),
            per_page: filter.per_page.unwrap_or(20),
            include_deleted: false,
        };

        let service = NodeService::new(db.clone(), event_bus.clone());
        let list_started_at = Instant::now();
        let (items, total): (Vec<rustok_content::dto::NodeListItem>, u64) = service
            .list_nodes_with_locale_fallback(
                tenant_id,
                auth_context_to_security(ctx),
                domain_filter,
                Some(tenant.default_locale.as_str()),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "blog.posts",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );

        let author_profiles = load_author_profiles_map(
            ctx,
            db,
            tenant_id,
            items.iter().map(|item| item.author_id),
            locale.as_str(),
            tenant.default_locale.as_str(),
        )
        .await?;
        let items = items
            .into_iter()
            .map(|item| {
                let author_profile = item
                    .author_id
                    .and_then(|author_id| author_profiles.get(&author_id).cloned());
                map_post_list_item(item, author_profile)
            })
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "blog.posts",
            requested_limit,
            effective_limit,
            items.len(),
        );

        Ok(GqlPostList { items, total })
    }
}

fn auth_context_to_security(ctx: &Context<'_>) -> SecurityContext {
    ctx.data::<AuthContext>()
        .map(|auth| auth.security_context())
        .unwrap_or_else(|_| SecurityContext::system())
}

fn is_public_request(ctx: &Context<'_>) -> bool {
    ctx.data_opt::<AuthContext>().is_none()
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

fn is_post_visible_for_request(
    metadata: &serde_json::Value,
    public_channel_slug: Option<&str>,
    is_authenticated: bool,
) -> bool {
    is_authenticated || is_post_visible_for_channel(metadata, public_channel_slug)
}

async fn list_public_visible_posts(
    ctx: &Context<'_>,
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    default_locale: &str,
    filter: PostsFilter,
    public_channel_slug: Option<&str>,
) -> Result<GqlPostList> {
    let locale = resolve_graphql_locale_fallback(filter.locale.as_deref(), default_locale);
    let requested_page = filter.page.unwrap_or(1).max(1);
    let requested_per_page = filter.per_page.unwrap_or(20).clamp(1, 100);
    let batch_size = requested_per_page.max(100);
    let service = NodeService::new(db.clone(), event_bus.clone());

    let mut current_page = 1u64;
    let mut visible_items = Vec::new();

    loop {
        let (items, total) = service
            .list_nodes_with_locale_fallback(
                tenant_id,
                SecurityContext::system(),
                rustok_content::dto::ListNodesFilter {
                    kind: Some("post".to_string()),
                    status: Some(rustok_content::entities::node::ContentStatus::Published),
                    parent_id: None,
                    author_id: filter.author_id,
                    category_id: None,
                    locale: Some(locale.clone()),
                    page: current_page,
                    per_page: batch_size,
                    include_deleted: false,
                },
                Some(default_locale),
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        if items.is_empty() {
            break;
        }

        visible_items.extend(
            items
                .into_iter()
                .filter(|item| is_post_visible_for_channel(&item.metadata, public_channel_slug)),
        );

        if current_page.saturating_mul(batch_size) >= total {
            break;
        }
        current_page += 1;
    }

    let visible_total = visible_items.len() as u64;
    let offset = requested_page
        .saturating_sub(1)
        .saturating_mul(requested_per_page) as usize;
    let page_items = visible_items
        .into_iter()
        .skip(offset)
        .take(requested_per_page as usize)
        .collect::<Vec<_>>();
    let author_profiles = load_author_profiles_map(
        ctx,
        db,
        tenant_id,
        page_items.iter().map(|item| item.author_id),
        locale.as_str(),
        default_locale,
    )
    .await?;
    let items = page_items
        .into_iter()
        .map(|item| {
            let author_profile = item
                .author_id
                .and_then(|author_id| author_profiles.get(&author_id).cloned());
            map_post_list_item(item, author_profile)
        })
        .collect::<Vec<_>>();

    Ok(GqlPostList {
        items,
        total: visible_total,
    })
}

fn resolve_graphql_locale_fallback(requested: Option<&str>, fallback: &str) -> String {
    requested
        .map(str::trim)
        .filter(|locale| !locale.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| fallback.to_string())
}

fn map_post(post: crate::PostResponse, author_profile: Option<GqlProfileSummary>) -> GqlPost {
    let mut gql: GqlPost = post.into();
    gql.author_profile = author_profile;
    gql
}

fn map_post_list_item(
    item: rustok_content::dto::NodeListItem,
    author_profile: Option<GqlProfileSummary>,
) -> GqlPostListItem {
    let mut gql: GqlPostListItem = item.into();
    gql.author_profile = author_profile;
    gql
}

async fn load_author_profiles_map<I>(
    ctx: &Context<'_>,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    author_ids: I,
    requested_locale: &str,
    tenant_default_locale: &str,
) -> Result<HashMap<Uuid, GqlProfileSummary>>
where
    I: IntoIterator<Item = Option<Uuid>>,
{
    let user_ids = author_ids
        .into_iter()
        .flatten()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if user_ids.is_empty() {
        return Ok(HashMap::new());
    }

    if let Some(loader) = ctx.data_opt::<DataLoader<ProfileSummaryLoader>>() {
        let keys = user_ids
            .iter()
            .map(|user_id| ProfileSummaryLoaderKey {
                tenant_id,
                user_id: *user_id,
                requested_locale: Some(requested_locale.to_string()),
                tenant_default_locale: Some(tenant_default_locale.to_string()),
            })
            .collect::<Vec<_>>();
        let profiles = loader.load_many(keys).await?;
        return Ok(profiles
            .into_iter()
            .map(|(key, summary)| (key.user_id, summary.into()))
            .collect());
    }

    let profiles = ProfileService::new(db.clone())
        .find_profile_summaries(
            tenant_id,
            &user_ids,
            Some(requested_locale),
            Some(tenant_default_locale),
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

    Ok(profiles
        .into_iter()
        .map(|(user_id, summary)| (user_id, summary.into()))
        .collect())
}

async fn require_public_blog_channel_enabled(ctx: &Context<'_>) -> Result<()> {
    let db = ctx.data::<DatabaseConnection>()?;
    ensure_public_blog_channel_enabled(
        db,
        ctx.data_opt::<RequestContext>(),
        ctx.data_opt::<AuthContext>().is_some(),
    )
    .await
}

async fn ensure_public_blog_channel_enabled(
    db: &DatabaseConnection,
    request_context: Option<&RequestContext>,
    is_authenticated: bool,
) -> Result<()> {
    if is_authenticated {
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
    use super::{ensure_public_blog_channel_enabled, is_post_visible_for_request};
    use rustok_api::{context::ChannelResolutionSource, RequestContext};
    use rustok_channel::{migrations, BindChannelModuleInput, ChannelService, CreateChannelInput};
    use rustok_test_utils::setup_test_db;
    use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
    use sea_orm_migration::SchemaManager;
    use uuid::Uuid;

    async fn setup_channel_db() -> DatabaseConnection {
        let db = setup_test_db().await;
        db.execute(Statement::from_string(
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
            migration
                .up(&manager)
                .await
                .expect("channel migration should apply");
        }
        db
    }

    async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) {
        db.execute(Statement::from_sql_and_values(
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
        }
    }

    #[tokio::test]
    async fn public_request_rejects_disabled_blog_channel_binding() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-blog").await;
        let service = ChannelService::new(db.clone());
        let channel = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "blog-web".to_string(),
                name: "Blog Web".to_string(),
                settings: None,
            })
            .await
            .expect("channel should be created");
        service
            .bind_module(
                channel.id,
                BindChannelModuleInput {
                    module_slug: "blog".to_string(),
                    is_enabled: false,
                    settings: None,
                },
            )
            .await
            .expect("binding should be saved");

        let result = ensure_public_blog_channel_enabled(
            &db,
            Some(&request_context(channel.id, "blog-web")),
            false,
        )
        .await;

        assert!(
            result.is_err(),
            "public blog read-path must be gated by channel binding"
        );
    }

    #[tokio::test]
    async fn authenticated_request_bypasses_blog_channel_module_gate() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-blog").await;
        let service = ChannelService::new(db.clone());
        let channel = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "blog-web".to_string(),
                name: "Blog Web".to_string(),
                settings: None,
            })
            .await
            .expect("channel should be created");
        service
            .bind_module(
                channel.id,
                BindChannelModuleInput {
                    module_slug: "blog".to_string(),
                    is_enabled: false,
                    settings: None,
                },
            )
            .await
            .expect("binding should be saved");

        let result = ensure_public_blog_channel_enabled(
            &db,
            Some(&request_context(channel.id, "blog-web")),
            true,
        )
        .await;

        assert!(
            result.is_ok(),
            "authenticated/admin blog flows must not be blocked"
        );
    }

    #[tokio::test]
    async fn public_request_allows_blog_channel_without_explicit_binding() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-blog").await;
        let service = ChannelService::new(db.clone());
        let channel = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "blog-web".to_string(),
                name: "Blog Web".to_string(),
                settings: None,
            })
            .await
            .expect("channel should be created");

        let result = ensure_public_blog_channel_enabled(
            &db,
            Some(&request_context(channel.id, "blog-web")),
            false,
        )
        .await;

        assert!(
            result.is_ok(),
            "missing blog binding should keep module enabled by default in v0"
        );
    }

    #[test]
    fn authenticated_request_bypasses_post_channel_allowlist() {
        let metadata = serde_json::json!({
            "channel_visibility": {
                "allowed_channel_slugs": ["web"]
            }
        });

        assert!(is_post_visible_for_request(&metadata, None, true));
        assert!(!is_post_visible_for_request(&metadata, None, false));
    }

    #[tokio::test]
    async fn disabled_binding_error_reports_resolution_source() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-blog").await;
        let service = ChannelService::new(db.clone());
        let channel = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "blog-web".to_string(),
                name: "Blog Web".to_string(),
                settings: None,
            })
            .await
            .expect("channel should be created");
        service
            .bind_module(
                channel.id,
                BindChannelModuleInput {
                    module_slug: "blog".to_string(),
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
            channel_slug: Some("blog-web".to_string()),
            channel_resolution_source: Some(ChannelResolutionSource::Query),
            locale: "en".to_string(),
        };

        let error = ensure_public_blog_channel_enabled(&db, Some(&request_context), false)
            .await
            .expect_err("disabled binding should be reported");

        assert!(
            error.message.contains("resolved via query"),
            "error must expose resolution source for diagnostics"
        );
    }
}
