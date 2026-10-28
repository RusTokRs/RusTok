//! Public RSS 2.0 transport owned by Blog. The storefront host only mounts
//! the HTTP route and maps failures to an HTTP status.
use rustok_api::{AuthContext, HostRuntimeContext, RequestContext, TenantContext};
use rustok_blog::{PostService, PublicPostsPageQuery};
use rustok_outbox::TransactionalEventBus;
use rustok_tenant::TenantService;

use super::configured_tenant_slug;

/// Publish only already-visible Blog posts. A tenant or channel must never be
/// inferred from an untrusted host name or query parameter.
pub async fn render_rss_feed(
    runtime: &HostRuntimeContext,
    locale: &str,
    tenant_context: Option<&TenantContext>,
    request_context: Option<&RequestContext>,
    auth_context: Option<&AuthContext>,
) -> Result<String, &'static str> {
    let (tenant_id, fallback_locale) = if let Some(tenant) = tenant_context {
        (tenant.id, tenant.default_locale.clone())
    } else {
        let tenant_slug = configured_tenant_slug().ok_or("Blog tenant is not configured")?;
        let tenant = TenantService::new(runtime.db_clone())
            .get_tenant_by_slug(&tenant_slug)
            .await
            .map_err(|_| "Blog tenant cannot be resolved")?;
        (tenant.id, tenant.default_locale)
    };

    if request_context.is_some_and(|context| context.tenant_id != tenant_id)
        || auth_context.is_some_and(|auth| auth.tenant_id != tenant_id)
    {
        return Err("Blog tenant context mismatch");
    }

    match rustok_api::is_tenant_module_enabled(runtime.db(), tenant_id, "blog").await {
        Ok(true) => {}
        Ok(false) => return Err("Blog is not enabled for this tenant"),
        Err(_) => return Err("Blog module state is unavailable"),
    }

    if auth_context.is_none() {
        if let Some(channel_id) = request_context.and_then(|context| context.channel_id) {
            let enabled = rustok_channel::ChannelService::new(runtime.db_clone())
                .is_module_enabled_for_tenant(tenant_id, channel_id, "blog")
                .await
                .map_err(|_| "Blog channel state is unavailable")?;
            if !enabled {
                return Err("Blog is not enabled for this channel");
            }
        }
    }

    let origin = std::env::var("NEXT_PUBLIC_STOREFRONT_URL")
        .or_else(|_| std::env::var("NEXT_PUBLIC_SITE_URL"))
        .map_err(|_| "Public storefront URL is not configured")?;
    let origin = url::Url::parse(&origin).map_err(|_| "Invalid public storefront URL")?;
    if !matches!(origin.scheme(), "http" | "https")
        || origin.host_str().is_none()
        || !origin.username().is_empty()
        || origin.password().is_some()
        || origin.query().is_some()
        || origin.fragment().is_some()
    {
        return Err("Invalid public storefront URL");
    }
    let root = origin.origin().ascii_serialization();
    let root = url::Url::parse(&root).map_err(|_| "Invalid public storefront origin")?;
    let locale = locale.trim();
    let posts_url = root
        .join(&format!("/{locale}/modules/blog"))
        .map_err(|_| "Invalid Blog post route")?;
    let feed_url = root
        .join(&format!("/{locale}/blog/feed.xml"))
        .map_err(|_| "Invalid Blog feed route")?;

    let event_bus = runtime
        .shared_get::<TransactionalEventBus>()
        .ok_or("Blog event runtime is unavailable")?;
    let channel_slug = request_context.and_then(|context| context.channel_slug.as_deref());
    let page = PostService::new(runtime.db_clone(), event_bus)
        .list_public_visible_keyset(
            tenant_id,
            PublicPostsPageQuery {
                category_id: None,
                tag: None,
                author_id: None,
                locale: Some(locale.to_string()),
                per_page: Some(25),
                after: None,
            },
            Some(fallback_locale.as_str()),
            channel_slug,
        )
        .await
        .map_err(|_| "Blog feed cannot be loaded")?;

    let is_ru = locale.starts_with("ru");
    let title = if is_ru { "Блог RusToK" } else { "RusToK Blog" };
    let description = if is_ru {
        "Последние статьи и публикации RusToK"
    } else {
        "Latest stories and articles from RusToK"
    };
    let items = page.items.into_iter().map(|post| {
        let mut post_url = posts_url.clone();
        post_url.query_pairs_mut().append_pair("slug", &post.slug);
        let post_url = post_url.as_str();
        let date = post.published_at.map(|date| date.to_rfc2822()).unwrap_or_default();
        let tags = post.tags.iter()
            .map(|tag| format!("      <category>{}</category>\n", cdata(tag)))
            .collect::<String>();
        let category = post.category_name
            .map(|name| format!("      <category>{}</category>\n", cdata(&name)))
            .unwrap_or_default();
        format!(
            "    <item>\n      <title>{}</title>\n      <link>{post_url}</link>\n      <guid isPermaLink=\"true\">{post_url}</guid>\n      <pubDate>{date}</pubDate>\n      <description>{}</description>\n{category}{tags}    </item>\n",
            cdata(&post.title),
            cdata(post.excerpt.as_deref().unwrap_or(&post.title)),
        )
    }).collect::<String>();

    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n  <channel>\n    <title>{}</title>\n    <link>{posts_url}</link>\n    <description>{}</description>\n    <language>{locale}</language>\n    <atom:link href=\"{feed_url}\" rel=\"self\" type=\"application/rss+xml\"/>\n{items}  </channel>\n</rss>",
        cdata(title),
        cdata(description),
    ))
}

fn cdata(value: &str) -> String {
    format!("<![CDATA[{}]]>", value.replace("]]>", "]]]]><![CDATA[>"))
}
