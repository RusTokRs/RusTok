use url::Url;
use uuid::Uuid;

use rustok_api::TenantContext;
use rustok_content::{resolve_by_locale_with_fallback, CanonicalUrlService};

use crate::dto::{
    SeoAlternateLink, SeoDocument, SeoPageContext, SeoRedirectDecision, SeoRouteContext,
    SeoTargetKind,
};
use crate::{SeoError, SeoResult};

use super::robots::{apply_robots, build_document, merge_open_graph, robots_from_directives};
use super::{DirectTarget, LoadedMeta, SeoService, TargetState};

impl SeoService {
    pub async fn resolve_page_context(
        &self,
        tenant: &TenantContext,
        locale: &str,
        route: &str,
    ) -> SeoResult<Option<SeoPageContext>> {
        self.resolve_page_context_for_channel(tenant, locale, route, None)
            .await
    }

    pub async fn resolve_page_context_for_channel(
        &self,
        tenant: &TenantContext,
        locale: &str,
        route: &str,
        channel_slug: Option<&str>,
    ) -> SeoResult<Option<SeoPageContext>> {
        if !self.is_enabled(tenant.id).await? {
            return Ok(None);
        }

        let locale = super::normalize_effective_locale(locale, tenant.default_locale.as_str())?;
        let route = super::normalize_route(route)?;
        self.resolve_page_context_inner(tenant, locale.as_str(), route.as_str(), channel_slug)
            .await
    }

    async fn resolve_page_context_inner(
        &self,
        tenant: &TenantContext,
        locale: &str,
        route: &str,
        channel_slug: Option<&str>,
    ) -> SeoResult<Option<SeoPageContext>> {
        let canonical_service = CanonicalUrlService::new(self.db.clone());
        if let Some(resolved) = canonical_service
            .resolve_route(tenant.id, locale, route)
            .await
            .map_err(|err| SeoError::validation(err.to_string()))?
        {
            if let Some(kind) = SeoTargetKind::from_str(resolved.target_kind.as_str()) {
                if let Some(mut context) = self
                    .load_target_page_context(
                        tenant,
                        kind,
                        resolved.target_id,
                        Some(locale.to_string()),
                        Some(resolved.canonical_url.clone()),
                        channel_slug,
                    )
                    .await?
                {
                    if resolved.redirect_required {
                        context.route.redirect = Some(SeoRedirectDecision {
                            target_url: locale_prefixed_path(
                                locale,
                                resolved.canonical_url.as_str(),
                            ),
                            status_code: 308,
                        });
                    }
                    return Ok(Some(context));
                }
            }
        }

        if let Some(redirect) = self.match_redirect(tenant.id, route).await? {
            if redirect.target_url == route {
                return Err(SeoError::validation("redirect loop detected"));
            }

            if redirect.target_url.starts_with('/') {
                if let Some(mut context) = self
                    .resolve_redirect_target_once(
                        tenant,
                        locale,
                        redirect.target_url.as_str(),
                        channel_slug,
                    )
                    .await?
                {
                    context.route.redirect = Some(SeoRedirectDecision {
                        target_url: if redirect.target_url.starts_with("http") {
                            redirect.target_url.clone()
                        } else {
                            locale_prefixed_path(locale, redirect.target_url.as_str())
                        },
                        status_code: redirect.status_code,
                    });
                    return Ok(Some(context));
                }
            }

            return Ok(Some(SeoPageContext {
                route: SeoRouteContext {
                    target_kind: None,
                    target_id: None,
                    requested_locale: Some(locale.to_string()),
                    effective_locale: locale.to_string(),
                    canonical_url: route.to_string(),
                    redirect: Some(SeoRedirectDecision {
                        target_url: redirect.target_url,
                        status_code: redirect.status_code,
                    }),
                    alternates: Vec::new(),
                },
                document: SeoDocument {
                    title: String::new(),
                    description: None,
                    robots: robots_from_directives(&[
                        "noindex".to_string(),
                        "nofollow".to_string(),
                    ]),
                    open_graph: None,
                    twitter: None,
                    verification: None,
                    pagination: None,
                    structured_data_blocks: Vec::new(),
                    meta_tags: Vec::new(),
                    link_tags: Vec::new(),
                },
            }));
        }

        let state = match parse_supported_route(route)? {
            Some(DirectTarget::Page { slug }) => {
                self.load_page_by_slug(tenant, locale, slug.as_str())
                    .await?
            }
            Some(DirectTarget::Product { handle }) => {
                self.load_product_by_handle(tenant, locale, handle.as_str())
                    .await?
            }
            Some(DirectTarget::BlogPost { slug }) => {
                self.load_blog_post_by_slug(tenant, locale, slug.as_str())
                    .await?
            }
            Some(DirectTarget::ForumCategory { id }) => {
                self.load_forum_category_by_id(tenant, locale, id).await?
            }
            Some(DirectTarget::ForumTopic { id }) => {
                self.load_public_forum_topic_by_id(tenant, locale, id, channel_slug)
                    .await?
            }
            None => None,
        };

        let Some(state) = state else {
            return Ok(None);
        };
        let explicit = self
            .load_explicit_meta(tenant.id, state.target_kind, state.target_id)
            .await?;
        self.merge_page_context(tenant, state, explicit)
            .await
            .map(Some)
    }

    async fn resolve_redirect_target_once(
        &self,
        tenant: &TenantContext,
        locale: &str,
        route: &str,
        channel_slug: Option<&str>,
    ) -> SeoResult<Option<SeoPageContext>> {
        let canonical_service = CanonicalUrlService::new(self.db.clone());
        if let Some(resolved) = canonical_service
            .resolve_route(tenant.id, locale, route)
            .await
            .map_err(|err| SeoError::validation(err.to_string()))?
        {
            if let Some(kind) = SeoTargetKind::from_str(resolved.target_kind.as_str()) {
                return self
                    .load_target_page_context(
                        tenant,
                        kind,
                        resolved.target_id,
                        Some(locale.to_string()),
                        Some(resolved.canonical_url),
                        channel_slug,
                    )
                    .await;
            }
        }

        let state = match parse_supported_route(route)? {
            Some(DirectTarget::Page { slug }) => {
                self.load_page_by_slug(tenant, locale, slug.as_str())
                    .await?
            }
            Some(DirectTarget::Product { handle }) => {
                self.load_product_by_handle(tenant, locale, handle.as_str())
                    .await?
            }
            Some(DirectTarget::BlogPost { slug }) => {
                self.load_blog_post_by_slug(tenant, locale, slug.as_str())
                    .await?
            }
            Some(DirectTarget::ForumCategory { id }) => {
                self.load_forum_category_by_id(tenant, locale, id).await?
            }
            Some(DirectTarget::ForumTopic { id }) => {
                self.load_public_forum_topic_by_id(tenant, locale, id, channel_slug)
                    .await?
            }
            None => None,
        };
        let Some(state) = state else {
            return Ok(None);
        };
        let explicit = self
            .load_explicit_meta(tenant.id, state.target_kind, state.target_id)
            .await?;
        self.merge_page_context(tenant, state, explicit)
            .await
            .map(Some)
    }

    async fn load_target_page_context(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetKind,
        target_id: Uuid,
        requested_locale: Option<String>,
        canonical_override: Option<String>,
        channel_slug: Option<&str>,
    ) -> SeoResult<Option<SeoPageContext>> {
        let Some(state) = self
            .load_route_target_state(
                tenant,
                target_kind,
                target_id,
                requested_locale
                    .as_deref()
                    .unwrap_or(tenant.default_locale.as_str()),
                channel_slug,
            )
            .await?
        else {
            return Ok(None);
        };
        let explicit = self
            .load_explicit_meta(tenant.id, target_kind, target_id)
            .await?;
        let mut context = self.merge_page_context(tenant, state, explicit).await?;
        context.route.requested_locale = requested_locale;
        if let Some(canonical_override) = canonical_override {
            context.route.canonical_url = locale_prefixed_path(
                context.route.effective_locale.as_str(),
                canonical_override.as_str(),
            );
        }
        Ok(Some(context))
    }

    async fn merge_page_context(
        &self,
        tenant: &TenantContext,
        state: TargetState,
        explicit: Option<LoadedMeta>,
    ) -> SeoResult<SeoPageContext> {
        let settings = self.load_settings(tenant.id).await?;
        let requested_locale = state.requested_locale.clone();

        if let Some(explicit) = explicit {
            let translation = resolve_by_locale_with_fallback(
                explicit.translations.as_slice(),
                state.effective_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                |item| item.locale.as_str(),
            );
            let effective_translation = translation.item.cloned();
            let title = effective_translation
                .as_ref()
                .and_then(|item| super::trimmed_option(item.title.clone()))
                .unwrap_or_else(|| state.title.clone());
            let description = effective_translation
                .as_ref()
                .and_then(|item| super::trimmed_option(item.description.clone()))
                .or(state.description.clone());
            let effective_locale = translation.effective_locale;
            let canonical_url = explicit
                .meta
                .canonical_url
                .clone()
                .filter(|value| !value.trim().is_empty())
                .map(|value| canonical_url_for_locale(effective_locale.as_str(), value.as_str()))
                .unwrap_or_else(|| {
                    locale_prefixed_path(effective_locale.as_str(), state.canonical_path.as_str())
                });
            let open_graph = merge_open_graph(
                &state.open_graph,
                effective_translation
                    .as_ref()
                    .and_then(|item| super::trimmed_option(item.og_title.clone())),
                effective_translation
                    .as_ref()
                    .and_then(|item| super::trimmed_option(item.og_description.clone())),
                effective_translation
                    .as_ref()
                    .and_then(|item| super::trimmed_option(item.og_image.clone())),
                canonical_url.as_str(),
                effective_locale.as_str(),
            );

            return Ok(SeoPageContext {
                route: SeoRouteContext {
                    target_kind: Some(state.target_kind),
                    target_id: Some(state.target_id),
                    requested_locale,
                    effective_locale: effective_locale.clone(),
                    canonical_url: canonical_url.clone(),
                    redirect: None,
                    alternates: with_x_default(
                        state.alternates,
                        settings.x_default_locale.as_deref(),
                        tenant.default_locale.as_str(),
                    ),
                },
                document: build_document(
                    title,
                    description,
                    apply_robots(
                        explicit.meta.no_index,
                        explicit.meta.no_follow,
                        settings.default_robots.as_slice(),
                    ),
                    Some(open_graph),
                    explicit
                        .meta
                        .structured_data
                        .clone()
                        .unwrap_or(state.structured_data),
                    effective_translation
                        .as_ref()
                        .and_then(|item| super::trimmed_option(item.keywords.clone())),
                    canonical_url.as_str(),
                    effective_locale.as_str(),
                ),
            });
        }

        let canonical_url = locale_prefixed_path(
            state.effective_locale.as_str(),
            state.canonical_path.as_str(),
        );
        Ok(SeoPageContext {
            route: SeoRouteContext {
                target_kind: Some(state.target_kind),
                target_id: Some(state.target_id),
                requested_locale,
                effective_locale: state.effective_locale.clone(),
                canonical_url: canonical_url.clone(),
                redirect: None,
                alternates: with_x_default(
                    state.alternates,
                    settings.x_default_locale.as_deref(),
                    tenant.default_locale.as_str(),
                ),
            },
            document: build_document(
                state.title,
                state.description,
                robots_from_directives(settings.default_robots.as_slice()),
                Some(state.open_graph),
                state.structured_data,
                None,
                canonical_url.as_str(),
                state.effective_locale.as_str(),
            ),
        })
    }
}

pub(super) fn parse_supported_route(route: &str) -> SeoResult<Option<DirectTarget>> {
    let parsed = Url::parse(format!("https://rustok.local{route}").as_str())
        .map_err(|err| SeoError::validation(err.to_string()))?;
    let mut segments = parsed
        .path_segments()
        .map(|items| items.filter(|item| !item.is_empty()).collect::<Vec<_>>())
        .unwrap_or_default();
    if segments.len() > 2
        && segments
            .first()
            .and_then(|item| rustok_core::normalize_locale_tag(item))
            .is_some()
        && segments.get(1) == Some(&"modules")
    {
        segments.remove(0);
    }

    if segments.as_slice() == ["modules", "pages"] {
        return Ok(parsed
            .query_pairs()
            .find(|(key, _)| key == "slug")
            .map(|(_, value)| DirectTarget::Page {
                slug: value.to_string(),
            }));
    }
    if segments.as_slice() == ["modules", "blog"] {
        return Ok(parsed
            .query_pairs()
            .find(|(key, _)| key == "slug")
            .map(|(_, value)| DirectTarget::BlogPost {
                slug: value.to_string(),
            }));
    }
    if segments.as_slice() == ["modules", "product"] {
        return Ok(parsed
            .query_pairs()
            .find(|(key, _)| key == "handle")
            .map(|(_, value)| DirectTarget::Product {
                handle: value.to_string(),
            }));
    }
    if segments.as_slice() == ["modules", "forum"] {
        let category_id = parse_uuid_query(&parsed, "category")?;
        let topic_id = parse_uuid_query(&parsed, "topic")?;
        if let Some(topic_id) = topic_id {
            return Ok(Some(DirectTarget::ForumTopic { id: topic_id }));
        }
        if let Some(category_id) = category_id {
            return Ok(Some(DirectTarget::ForumCategory { id: category_id }));
        }
    }

    Ok(None)
}

fn parse_uuid_query(parsed: &Url, key: &str) -> SeoResult<Option<Uuid>> {
    let Some((_, value)) = parsed.query_pairs().find(|(query_key, _)| query_key == key) else {
        return Ok(None);
    };
    let raw = value.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    Uuid::parse_str(raw)
        .map(Some)
        .map_err(|_| SeoError::validation(format!("invalid `{key}` query parameter")))
}

#[cfg(test)]
mod tests {
    use super::{parse_supported_route, DirectTarget};
    use crate::migrations as seo_migrations;
    use crate::SeoService;
    use rustok_api::TenantContext;
    use rustok_core::{MemoryTransport, SecurityContext};
    use rustok_forum::{
        migrations as forum_migrations, CategoryService, CreateCategoryInput, CreateTopicInput,
        TopicService,
    };
    use rustok_outbox::TransactionalEventBus;
    use rustok_taxonomy::migrations as taxonomy_migrations;
    use rustok_tenant::entities::tenant_module;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::{
        ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    };
    use sea_orm_migration::SchemaManager;
    use std::sync::Arc;
    use uuid::Uuid;

    async fn test_db() -> DatabaseConnection {
        let db_url = format!(
            "sqlite:file:seo_routing_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut opts = ConnectOptions::new(db_url);
        opts.max_connections(5)
            .min_connections(1)
            .sqlx_logging(false);
        Database::connect(opts)
            .await
            .expect("failed to connect seo routing sqlite db")
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

    async fn seed_meta_tables(db: &DatabaseConnection) {
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE meta (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                target_type TEXT NOT NULL,
                target_id TEXT NOT NULL,
                no_index INTEGER NOT NULL,
                no_follow INTEGER NOT NULL,
                canonical_url TEXT NULL,
                structured_data TEXT NULL
            )"
            .to_string(),
        ))
        .await
        .expect("create meta table");
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE meta_translations (
                id TEXT PRIMARY KEY,
                meta_id TEXT NOT NULL,
                locale TEXT NOT NULL,
                title TEXT NULL,
                description TEXT NULL,
                keywords TEXT NULL,
                og_title TEXT NULL,
                og_description TEXT NULL,
                og_image TEXT NULL
            )"
            .to_string(),
        ))
        .await
        .expect("create meta_translations table");
    }

    async fn seed_content_routing_tables(db: &DatabaseConnection) {
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE content_canonical_urls (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                target_kind TEXT NOT NULL,
                target_id TEXT NOT NULL,
                locale TEXT NOT NULL,
                canonical_url TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )"
            .to_string(),
        ))
        .await
        .expect("create content_canonical_urls table");
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE content_url_aliases (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                target_kind TEXT NOT NULL,
                target_id TEXT NOT NULL,
                locale TEXT NOT NULL,
                alias_url TEXT NOT NULL,
                canonical_url TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )"
            .to_string(),
        ))
        .await
        .expect("create content_url_aliases table");
    }

    async fn enable_seo_module(db: &DatabaseConnection, tenant_id: Uuid) {
        let now = chrono::Utc::now();
        tenant_module::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            module_slug: Set("seo".to_string()),
            enabled: Set(true),
            settings: Set(serde_json::json!({}).into()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await
        .expect("insert seo module row");
    }

    async fn run_forum_migrations(db: &DatabaseConnection) {
        let manager = SchemaManager::new(db);
        for migration in forum_migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("forum migration should apply");
        }
    }

    async fn run_seo_migrations(db: &DatabaseConnection) {
        let manager = SchemaManager::new(db);
        for migration in seo_migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("seo migration should apply");
        }
    }

    async fn run_taxonomy_migrations(db: &DatabaseConnection) {
        let manager = SchemaManager::new(db);
        for migration in taxonomy_migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("taxonomy migration should apply");
        }
    }

    fn tenant_context(tenant_id: Uuid) -> TenantContext {
        TenantContext {
            id: tenant_id,
            name: "SEO Forum Tenant".to_string(),
            slug: "seo-forum".to_string(),
            domain: Some("forum.example.com".to_string()),
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    #[test]
    fn parse_supported_route_recognizes_forum_category_route() {
        let category_id = Uuid::new_v4();
        let parsed =
            parse_supported_route(format!("/modules/forum?category={category_id}").as_str())
                .expect("forum category route should parse");
        match parsed {
            Some(DirectTarget::ForumCategory { id }) => assert_eq!(id, category_id),
            other => panic!("unexpected parsed route: {other:?}"),
        }
    }

    #[test]
    fn parse_supported_route_recognizes_forum_topic_route() {
        let topic_id = Uuid::new_v4();
        let parsed = parse_supported_route(
            format!(
                "/modules/forum?category={}&topic={topic_id}",
                Uuid::new_v4()
            )
            .as_str(),
        )
        .expect("forum topic route should parse");
        match parsed {
            Some(DirectTarget::ForumTopic { id }) => assert_eq!(id, topic_id),
            other => panic!("unexpected parsed route: {other:?}"),
        }
    }

    #[test]
    fn parse_supported_route_rejects_invalid_forum_uuid_query() {
        let error = parse_supported_route("/modules/forum?topic=not-a-uuid")
            .expect_err("invalid topic query should fail");
        assert!(error
            .to_string()
            .contains("invalid `topic` query parameter"));
    }

    #[test]
    fn parse_supported_route_keeps_modules_prefix_without_locale() {
        let parsed =
            parse_supported_route("/modules/pages?slug=home").expect("page route should parse");
        match parsed {
            Some(DirectTarget::Page { slug }) => assert_eq!(slug, "home"),
            other => panic!("unexpected parsed route: {other:?}"),
        }
    }

    #[tokio::test]
    async fn resolve_page_context_supports_forum_direct_routes() {
        let db = test_db().await;
        seed_tenant_modules_table(&db).await;
        seed_meta_tables(&db).await;
        seed_content_routing_tables(&db).await;
        run_seo_migrations(&db).await;
        run_taxonomy_migrations(&db).await;
        run_forum_migrations(&db).await;
        let tenant_id = Uuid::new_v4();
        enable_seo_module(&db, tenant_id).await;
        let tenant = tenant_context(tenant_id);
        let transport = Arc::new(MemoryTransport::new());
        let _receiver = transport.subscribe();
        let event_bus = TransactionalEventBus::new(transport);
        let security = SecurityContext::system();

        let category = CategoryService::new(db.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "General".to_string(),
                    slug: "general".to_string(),
                    description: Some("Public category".to_string()),
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await
            .expect("forum category should be created");
        CategoryService::new(db.clone())
            .get_with_locale_fallback(
                tenant_id,
                SecurityContext::system(),
                category.id,
                "en",
                Some("en"),
            )
            .await
            .expect("forum category should be readable after create");

        let topic = TopicService::new(db.clone(), event_bus.clone())
            .create(
                tenant_id,
                security,
                CreateTopicInput {
                    locale: "en".to_string(),
                    category_id: category.id,
                    title: "Welcome thread".to_string(),
                    slug: Some("welcome-thread".to_string()),
                    body: "Introduce yourself to the community.".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    channel_slugs: None,
                },
            )
            .await
            .expect("forum topic should be created");

        let service = SeoService::new(db.clone(), event_bus);
        assert!(
            service
                .is_enabled(tenant.id)
                .await
                .expect("seo module enabled lookup should succeed"),
            "seo module should be enabled for tenant"
        );
        let category_meta = service
            .seo_meta(
                &tenant,
                crate::SeoTargetKind::ForumCategory,
                category.id,
                Some("en"),
            )
            .await
            .expect("forum category fallback meta should resolve");
        assert!(
            category_meta.is_some(),
            "forum category target loader should resolve"
        );

        let category_context = service
            .resolve_page_context(
                &tenant,
                "en",
                format!("/modules/forum?category={}", category.id).as_str(),
            )
            .await
            .expect("forum category SEO route should resolve")
            .expect("forum category SEO context should exist");
        assert_eq!(
            category_context.route.target_kind,
            Some(crate::SeoTargetKind::ForumCategory)
        );
        assert_eq!(category_context.document.title, "General");

        let topic_context = service
            .resolve_page_context(
                &tenant,
                "en",
                format!("/modules/forum?category={}&topic={}", category.id, topic.id).as_str(),
            )
            .await
            .expect("forum topic SEO route should resolve")
            .expect("forum topic SEO context should exist");
        assert_eq!(
            topic_context.route.target_kind,
            Some(crate::SeoTargetKind::ForumTopic)
        );
        assert_eq!(topic_context.document.title, "Welcome thread");
        assert!(topic_context.route.canonical_url.ends_with(
            format!("/modules/forum?category={}&topic={}", category.id, topic.id).as_str()
        ));
    }

    #[tokio::test]
    async fn resolve_page_context_for_channel_supports_restricted_forum_topics() {
        let db = test_db().await;
        seed_tenant_modules_table(&db).await;
        seed_meta_tables(&db).await;
        seed_content_routing_tables(&db).await;
        run_seo_migrations(&db).await;
        run_taxonomy_migrations(&db).await;
        run_forum_migrations(&db).await;
        let tenant_id = Uuid::new_v4();
        enable_seo_module(&db, tenant_id).await;
        let tenant = tenant_context(tenant_id);
        let transport = Arc::new(MemoryTransport::new());
        let _receiver = transport.subscribe();
        let event_bus = TransactionalEventBus::new(transport);
        let security = SecurityContext::system();

        let category = CategoryService::new(db.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "Support".to_string(),
                    slug: "support".to_string(),
                    description: Some("Restricted support".to_string()),
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await
            .expect("forum category should be created");

        let topic = TopicService::new(db.clone(), event_bus.clone())
            .create(
                tenant_id,
                security,
                CreateTopicInput {
                    locale: "en".to_string(),
                    category_id: category.id,
                    title: "Mobile release notes".to_string(),
                    slug: Some("mobile-release-notes".to_string()),
                    body: "Visible only for the mobile channel.".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    channel_slugs: Some(vec!["mobile".to_string()]),
                },
            )
            .await
            .expect("restricted forum topic should be created");

        let service = SeoService::new(db.clone(), event_bus);
        let route = format!("/modules/forum?category={}&topic={}", category.id, topic.id);

        let without_channel = service
            .resolve_page_context(&tenant, "en", route.as_str())
            .await
            .expect("forum SEO route without channel should resolve");
        assert!(
            without_channel.is_none(),
            "restricted forum topic should stay hidden without request channel",
        );

        let with_channel = service
            .resolve_page_context_for_channel(&tenant, "en", route.as_str(), Some(" MOBILE "))
            .await
            .expect("forum SEO route with channel should resolve")
            .expect("restricted forum topic should resolve for matching channel");
        assert_eq!(
            with_channel.route.target_kind,
            Some(crate::SeoTargetKind::ForumTopic)
        );
        assert_eq!(with_channel.document.title, "Mobile release notes");
    }
}

pub(super) fn locale_prefixed_path(locale: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    if locale.trim().is_empty() {
        path
    } else if path == "/" {
        format!("/{locale}")
    } else {
        format!("/{locale}{path}")
    }
}

pub(super) fn canonical_url_for_locale(locale: &str, canonical_url: &str) -> String {
    if canonical_url.starts_with("http://") || canonical_url.starts_with("https://") {
        canonical_url.to_string()
    } else {
        locale_prefixed_path(locale, canonical_url)
    }
}

pub(super) fn with_x_default(
    mut alternates: Vec<SeoAlternateLink>,
    x_default_locale: Option<&str>,
    tenant_default_locale: &str,
) -> Vec<SeoAlternateLink> {
    let x_default_locale = x_default_locale
        .and_then(rustok_core::normalize_locale_tag)
        .unwrap_or_else(|| tenant_default_locale.to_string());
    for alternate in &mut alternates {
        alternate.x_default = alternate.locale == x_default_locale;
    }
    if let Some(href) = alternates
        .iter()
        .find(|item| item.locale == x_default_locale)
        .map(|item| item.href.clone())
    {
        alternates.push(SeoAlternateLink {
            locale: "x-default".to_string(),
            href,
            x_default: true,
        });
    }
    alternates
}
