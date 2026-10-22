use async_graphql::{Context, ErrorExtensions, Object, Result};
use rustok_api::{
    AuthContext, RequestContext, TenantContext,
    graphql::{require_module_enabled, resolve_graphql_locale},
};
use rustok_channel::ChannelService;
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use rustok_profiles::{GqlProfileSummary, ProfileSummaryAudience};
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use uuid::Uuid;

use crate::services::is_post_visible_for_channel;
use crate::{BlogError, CategoryService, PostService, TagService};

use super::runtime_data::BlogGraphqlRuntimeData;
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
        if id.is_nil() {
            return Err(
                <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::bad_user_input(
                    "Post ID must not be nil",
                ),
            );
        }
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_blog_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = query_tenant_id(ctx, tenant, tenant_id)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = PostService::new(db.clone(), event_bus.clone());
        let post = match service
            .get_post_with_locale_fallback(
                tenant_id,
                request_security_context(ctx),
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
            Err(BlogError::Forbidden(_)) if is_public_request(ctx) => return Ok(None),
            Err(err) => return Err(crate::error::public::to_graphql_error(err)),
        };

        if is_public_request(ctx)
            && (post.status != crate::BlogPostStatus::Published
                || !is_post_visible_for_request(
                    &post.channel_slugs,
                    public_channel_slug(ctx).as_deref(),
                    false,
                ))
        {
            return Ok(None);
        }

        let author_profiles = load_author_profiles_map(
            ctx,
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
        let tenant_id = query_tenant_id(ctx, tenant, tenant_id)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = PostService::new(db.clone(), event_bus.clone());
        let post = match service
            .get_post_by_slug_with_locale_fallback(
                tenant_id,
                request_security_context(ctx),
                &locale,
                &slug,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(post) => post,
            Err(BlogError::PostNotFound(_))
            | Err(BlogError::Content(rustok_content::ContentError::NodeNotFound(_))) => {
                return Ok(None);
            }
            // A public reader must not learn that a post exists but is not
            // readable for them: the slug lookup answers with "no post".
            Err(BlogError::Forbidden(_)) if is_public_request(ctx) => return Ok(None),
            Err(err) => return Err(crate::error::public::to_graphql_error(err)),
        };

        if let Some(post) = post.filter(|post| {
            is_post_visible_for_request(
                &post.channel_slugs,
                public_channel_slug(ctx).as_deref(),
                !is_public_request(ctx),
            )
        }) {
            let author_profiles = load_author_profiles_map(
                ctx,
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
        let tenant_id = query_tenant_id(ctx, tenant, tenant_id)?;

        let filter = filter.unwrap_or(PostsFilter {
            status: None,
            category_id: None,
            tag: None,
            author_id: None,
            locale: None,
            after: None,
            per_page: Some(20),
            sort_by: None,
            sort_order: None,
        });

        if is_public_request(ctx) {
            return Err(async_graphql::Error::new(
                "Public blog listings use publicBlogPosts with cursor pagination",
            ));
        }

        let requested_limit = filter.per_page;
        let locale = resolve_graphql_locale(ctx, filter.locale.as_deref());
        let effective_limit = filter.per_page.unwrap_or(20).clamp(1, 100);

        let service = PostService::new(db.clone(), event_bus.clone());
        let list_started_at = Instant::now();
        let result = service
            .list_posts_with_locale_fallback(
                tenant_id,
                request_security_context(ctx),
                crate::PostListQuery {
                    status: filter.status.map(Into::into),
                    category_id: filter.category_id,
                    tag: filter.tag.clone(),
                    author_id: filter.author_id,
                    locale: Some(locale.clone()),
                    after: filter.after.clone(),
                    // The clamped limit is the executed limit: the reported
                    // budget metric and the query must not disagree.
                    per_page: Some(effective_limit as u32),
                    sort_by: Some(filter.sort_by.map(Into::into).unwrap_or_default()),
                    sort_order: Some(filter.sort_order.map(Into::into).unwrap_or_default()),
                },
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(crate::error::public::to_graphql_error)?;
        metrics::record_read_path_query(
            "graphql",
            "blog.posts",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            result.items.len() as u64,
        );

        let author_profiles = load_author_profiles_map(
            ctx,
            tenant_id,
            result.items.iter().map(|item| Some(item.author_id)),
            locale.as_str(),
            tenant.default_locale.as_str(),
        )
        .await?;
        let items = result
            .items
            .into_iter()
            .map(|item| {
                let author_profile = author_profiles.get(&item.author_id).cloned();
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

        Ok(GqlPostList {
            items,
            next_cursor: result.next_cursor,
        })
    }

    /// Public published-post list with keyset pagination. Returns no total count.
    async fn public_blog_posts(
        &self,
        ctx: &Context<'_>,
        filter: Option<PublicPostsFilter>,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPublicPostList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_blog_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = query_tenant_id(ctx, tenant, tenant_id)?;

        let filter = filter.unwrap_or_default();
        let locale = resolve_graphql_locale(ctx, filter.locale.as_deref());
        let after = filter
            .after
            .as_deref()
            .map(crate::PublishedPostCursor::decode)
            .transpose()
            .map_err(crate::error::public::to_graphql_error)?;
        let query = crate::PublicPostsPageQuery {
            category_id: filter.category_id,
            tag: filter.tag,
            author_id: filter.author_id,
            locale: Some(locale.clone()),
            per_page: filter.per_page.map(|value| value as u32),
            after,
        };

        let service = PostService::new(db.clone(), event_bus.clone());
        let page = service
            .list_public_visible_keyset(
                tenant_id,
                query,
                Some(tenant.default_locale.as_str()),
                public_channel_slug(ctx).as_deref(),
            )
            .await
            .map_err(crate::error::public::to_graphql_error)?;

        let author_profiles = load_author_profiles_map(
            ctx,
            tenant_id,
            page.items.iter().map(|item| Some(item.author_id)),
            locale.as_str(),
            tenant.default_locale.as_str(),
        )
        .await?;
        let items = page
            .items
            .into_iter()
            .map(|item| {
                let author_profile = author_profiles.get(&item.author_id).cloned();
                map_post_list_item(item, author_profile)
            })
            .collect::<Vec<_>>();

        Ok(GqlPublicPostList {
            items,
            next_cursor: page.next_cursor.map(|cursor| cursor.encode()),
        })
    }

    async fn blog_category(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        locale: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlBlogCategory>> {
        if id.is_nil() {
            return Err(
                <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::bad_user_input(
                    "Category ID must not be nil",
                ),
            );
        }
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_blog_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = query_tenant_id(ctx, tenant, tenant_id)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = CategoryService::new(db.clone(), event_bus.clone());
        let category = service
            .get(tenant_id, request_security_context(ctx), id, &locale)
            .await;

        match category {
            Ok(cat) => Ok(Some(cat.into())),
            Err(BlogError::CategoryNotFound(_)) => Ok(None),
            Err(BlogError::Forbidden(_)) if is_public_request(ctx) => Ok(None),
            Err(err) => Err(crate::error::public::to_graphql_error(err)),
        }
    }

    async fn blog_categories(
        &self,
        ctx: &Context<'_>,
        filter: Option<BlogCategoriesFilter>,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlBlogCategoryList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_blog_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = query_tenant_id(ctx, tenant, tenant_id)?;

        let filter = filter.unwrap_or_default();
        let locale = resolve_graphql_locale(ctx, filter.locale.as_deref());
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(100).clamp(1, 100);

        let service = CategoryService::new(db.clone(), event_bus.clone());
        let (items, total) = service
            .list(
                tenant_id,
                request_security_context(ctx),
                crate::dto::ListCategoriesFilter {
                    locale: Some(locale),
                    page,
                    per_page,
                },
            )
            .await
            .map_err(crate::error::public::to_graphql_error)?;

        Ok(GqlBlogCategoryList {
            items: items.into_iter().map(Into::into).collect(),
            total,
        })
    }

    async fn blog_tags(
        &self,
        ctx: &Context<'_>,
        filter: Option<BlogTagsFilter>,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlBlogTagList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_blog_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = query_tenant_id(ctx, tenant, tenant_id)?;

        let filter = filter.unwrap_or_default();
        let locale = resolve_graphql_locale(ctx, filter.locale.as_deref());
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(50).clamp(1, 100);

        let service = TagService::new(db.clone());
        let (items, total) = service
            .list_tags(
                tenant_id,
                request_security_context(ctx),
                crate::dto::ListTagsFilter {
                    locale: Some(locale),
                    page,
                    per_page,
                },
            )
            .await
            .map_err(crate::error::public::to_graphql_error)?;

        Ok(GqlBlogTagList {
            items: items.into_iter().map(Into::into).collect(),
            total,
        })
    }

    async fn post_by_preview_token(
        &self,
        ctx: &Context<'_>,
        token: String,
        locale: Option<String>,
    ) -> Result<Option<GqlPost>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = crate::services::PreviewTokenService::new(db.clone(), event_bus.clone());
        let post = service
            .get_post_by_token(&token, &locale, Some(tenant.default_locale.as_str()))
            .await
            .map_err(crate::error::public::to_graphql_error)?;

        match post {
            Some(post) => Ok(Some(map_post(post, None))),
            None => Ok(None),
        }
    }
}

fn query_tenant_id(
    ctx: &Context<'_>,
    tenant: &TenantContext,
    requested: Option<Uuid>,
) -> Result<Uuid> {
    if tenant.id.is_nil() {
        return Err(
            <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::bad_user_input(
                "Tenant ID must not be nil",
            ),
        );
    }
    if let Some(requested) = requested {
        if requested.is_nil() {
            return Err(
                <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::bad_user_input(
                    "Requested tenant ID must not be nil",
                ),
            );
        }
        if requested != tenant.id {
            return Err(
                <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::permission_denied(
                    "Blog queries must use the current tenant",
                ),
            );
        }
    }
    if let Some(auth) = ctx.data_opt::<AuthContext>() {
        if auth.tenant_id.is_nil() {
            return Err(
                <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::bad_user_input(
                    "Authenticated tenant ID must not be nil",
                ),
            );
        }
        if auth.tenant_id != tenant.id {
            return Err(
                <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::permission_denied(
                    "Authenticated actor is not bound to the current tenant",
                ),
            );
        }
    }
    Ok(tenant.id)
}

fn request_security_context(ctx: &Context<'_>) -> SecurityContext {
    ctx.data_opt::<AuthContext>()
        .map(security_context_from_auth)
        .unwrap_or_else(SecurityContext::public_read)
}

fn security_context_from_auth(auth: &AuthContext) -> SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn is_public_request(ctx: &Context<'_>) -> bool {
    ctx.data_opt::<AuthContext>().is_none()
}

pub(super) fn public_channel_slug(ctx: &Context<'_>) -> Option<String> {
    ctx.data_opt::<RequestContext>()
        .and_then(|request_context| request_context.channel_slug.clone())
        .map(|slug| slug.trim().to_ascii_lowercase())
        .filter(|slug| !slug.is_empty())
}

fn is_post_visible_for_request(
    channel_slugs: &[String],
    public_channel_slug: Option<&str>,
    is_authenticated: bool,
) -> bool {
    is_authenticated || is_post_visible_for_channel(channel_slugs, public_channel_slug)
}

fn map_post(post: crate::PostResponse, author_profile: Option<GqlProfileSummary>) -> GqlPost {
    let mut gql: GqlPost = post.into();
    gql.author_profile = author_profile;
    gql
}

fn map_post_list_item(
    item: crate::PostSummary,
    author_profile: Option<GqlProfileSummary>,
) -> GqlPostListItem {
    let mut gql: GqlPostListItem = item.into();
    gql.author_profile = author_profile;
    gql
}

async fn load_author_profiles_map<I>(
    ctx: &Context<'_>,
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

    let Some(runtime_data) = ctx.data_opt::<BlogGraphqlRuntimeData>() else {
        tracing::warn!(
            tenant_id = %tenant_id,
            author_count = user_ids.len(),
            "Blog author profile enrichment runtime data is unavailable; returning posts without profile presentation"
        );
        return Ok(HashMap::new());
    };
    let Some(reader) = runtime_data.profile_summary_reader() else {
        tracing::warn!(
            tenant_id = %tenant_id,
            author_count = user_ids.len(),
            "Blog author profile provider is unavailable; returning posts without profile presentation"
        );
        return Ok(HashMap::new());
    };

    let audience = ctx
        .data_opt::<ProfileSummaryAudience>()
        .copied()
        .unwrap_or(ProfileSummaryAudience::Anonymous);

    let profiles = match reader
        .find_profile_summaries(
            tenant_id,
            &user_ids,
            Some(requested_locale),
            Some(tenant_default_locale),
            audience,
        )
        .await
    {
        Ok(profiles) => profiles,
        Err(error) => {
            tracing::warn!(
                tenant_id = %tenant_id,
                author_count = user_ids.len(),
                error_code = error.code(),
                "Blog author profile provider is unavailable; returning posts without profile presentation"
            );
            return Ok(HashMap::new());
        }
    };

    Ok(profiles
        .into_iter()
        .map(|(user_id, summary)| (user_id, summary.into()))
        .collect())
}

async fn require_public_blog_channel_enabled(ctx: &Context<'_>) -> Result<()> {
    let db = ctx.data::<DatabaseConnection>()?;
    let tenant = ctx.data::<TenantContext>()?;
    ensure_public_blog_channel_enabled(
        db,
        tenant.id,
        ctx.data_opt::<RequestContext>(),
        ctx.data_opt::<AuthContext>().is_some(),
    )
    .await
}

pub(super) async fn ensure_authenticated_blog_channel_enabled(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    request_context: Option<&RequestContext>,
) -> Result<()> {
    let Some(request_context) = request_context else {
        return Err(
            <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::internal_error(
                "Blog comment creation requires current channel context",
            ),
        );
    };
    let Some(channel_id) = request_context.channel_id else {
        return Err(
            <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::internal_error(
                "Blog comment creation requires current channel",
            ),
        );
    };

    let enabled = ChannelService::new(db.clone())
        .is_module_enabled_for_tenant(tenant_id, channel_id, MODULE_SLUG)
        .await
        .map_err(|_| {
            <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::internal_error(
                "Unable to verify Blog channel availability",
            )
        })?;

    if enabled {
        Ok(())
    } else {
        Err(
            async_graphql::Error::new("Blog is not available for the current channel")
                .extend_with(|_, ext| ext.set("code", "MODULE_NOT_ENABLED")),
        )
    }
}

pub(super) async fn ensure_public_blog_channel_enabled(
    db: &DatabaseConnection,
    tenant_id: Uuid,
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
        .is_module_enabled_for_tenant(tenant_id, channel_id, MODULE_SLUG)
        .await
        .map_err(|_| {
            <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::internal_error(
                "Unable to verify Blog channel availability",
            )
        })?;

    if enabled {
        return Ok(());
    }

    Err(
        async_graphql::Error::new("Blog is not available for the current channel")
            .extend_with(|_, ext| ext.set("code", "MODULE_NOT_ENABLED")),
    )
}

#[cfg(test)]
mod tests;
