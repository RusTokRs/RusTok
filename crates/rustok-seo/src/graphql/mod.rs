use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_api::{
    graphql::{require_module_enabled, resolve_graphql_locale, GraphQLError},
    has_any_effective_permission, AuthContext, RequestContext, TenantContext,
};
use rustok_core::Permission;
use rustok_outbox::TransactionalEventBus;

use crate::{
    SeoError, SeoMetaInput, SeoMetaRecord, SeoPageContext, SeoRedirectInput, SeoRedirectRecord,
    SeoRevisionRecord, SeoService, SeoSitemapStatusRecord, SeoTargetKind,
};

const MODULE_SLUG: &str = "seo";

#[derive(Default)]
pub struct SeoQuery;

#[derive(Default)]
pub struct SeoMutation;

#[Object]
impl SeoQuery {
    async fn seo_page_context(
        &self,
        ctx: &Context<'_>,
        route: String,
        locale: Option<String>,
    ) -> Result<Option<SeoPageContext>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let channel_slug = ctx
            .data_opt::<RequestContext>()
            .and_then(|request| request.channel_slug.as_deref());
        SeoService::new(db.clone(), event_bus.clone())
            .resolve_page_context_for_channel(tenant, locale.as_str(), route.as_str(), channel_slug)
            .await
            .map_err(map_seo_error)
    }

    async fn seo_meta(
        &self,
        ctx: &Context<'_>,
        target_kind: SeoTargetKind,
        target_id: Uuid,
        locale: Option<String>,
    ) -> Result<Option<SeoMetaRecord>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_seo_permission(ctx, &[Permission::SEO_READ], "seo:read required")?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        SeoService::new(db.clone(), event_bus.clone())
            .seo_meta(tenant, target_kind, target_id, locale.as_deref())
            .await
            .map_err(map_seo_error)
    }

    async fn seo_redirects(&self, ctx: &Context<'_>) -> Result<Vec<SeoRedirectRecord>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_seo_permission(ctx, &[Permission::SEO_READ], "seo:read required")?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        SeoService::new(db.clone(), event_bus.clone())
            .list_redirects(tenant.id)
            .await
            .map_err(map_seo_error)
    }

    async fn seo_sitemap_status(&self, ctx: &Context<'_>) -> Result<SeoSitemapStatusRecord> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_seo_permission(
            ctx,
            &[Permission::SEO_READ, Permission::SEO_GENERATE],
            "seo:read or seo:generate required",
        )?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        SeoService::new(db.clone(), event_bus.clone())
            .sitemap_status(tenant)
            .await
            .map_err(map_seo_error)
    }
}

#[Object]
impl SeoMutation {
    async fn upsert_seo_meta(
        &self,
        ctx: &Context<'_>,
        input: SeoMetaInput,
    ) -> Result<SeoMetaRecord> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_seo_permission(ctx, &[Permission::SEO_UPDATE], "seo:update required")?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        SeoService::new(db.clone(), event_bus.clone())
            .upsert_meta(tenant, input)
            .await
            .map_err(map_seo_error)
    }

    async fn publish_seo_revision(
        &self,
        ctx: &Context<'_>,
        target_kind: SeoTargetKind,
        target_id: Uuid,
        note: Option<String>,
    ) -> Result<SeoRevisionRecord> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_seo_permission(ctx, &[Permission::SEO_PUBLISH], "seo:publish required")?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        SeoService::new(db.clone(), event_bus.clone())
            .publish_revision(tenant, target_kind, target_id, note)
            .await
            .map_err(map_seo_error)
    }

    async fn rollback_seo_revision(
        &self,
        ctx: &Context<'_>,
        target_kind: SeoTargetKind,
        target_id: Uuid,
        revision: i32,
    ) -> Result<SeoMetaRecord> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_seo_permission(ctx, &[Permission::SEO_PUBLISH], "seo:publish required")?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        SeoService::new(db.clone(), event_bus.clone())
            .rollback_revision(tenant, target_kind, target_id, revision)
            .await
            .map_err(map_seo_error)
    }

    async fn upsert_seo_redirect(
        &self,
        ctx: &Context<'_>,
        input: SeoRedirectInput,
    ) -> Result<SeoRedirectRecord> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_seo_permission(ctx, &[Permission::SEO_UPDATE], "seo:update required")?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        SeoService::new(db.clone(), event_bus.clone())
            .upsert_redirect(tenant, input)
            .await
            .map_err(map_seo_error)
    }

    async fn generate_seo_sitemaps(&self, ctx: &Context<'_>) -> Result<SeoSitemapStatusRecord> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_seo_permission(ctx, &[Permission::SEO_GENERATE], "seo:generate required")?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        SeoService::new(db.clone(), event_bus.clone())
            .generate_sitemaps(tenant)
            .await
            .map_err(map_seo_error)
    }
}

fn require_seo_permission(
    ctx: &Context<'_>,
    permissions: &[Permission],
    message: &str,
) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(<FieldError as GraphQLError>::permission_denied(message));
    }
    Ok(auth)
}

fn map_seo_error(error: SeoError) -> async_graphql::Error {
    match error {
        SeoError::Validation(message) => <FieldError as GraphQLError>::bad_user_input(&message),
        SeoError::NotFound => <FieldError as GraphQLError>::not_found("SEO record not found"),
        SeoError::PermissionDenied => {
            <FieldError as GraphQLError>::permission_denied("Permission denied")
        }
        SeoError::Database(error) => {
            <FieldError as GraphQLError>::internal_error(&error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SeoQuery;
    use async_graphql::{EmptyMutation, EmptySubscription, Request, Schema};
    use rustok_api::{AuthContext, RequestContext, TenantContext};
    use rustok_core::{MemoryTransport, Permission};
    use rustok_forum::{
        migrations as forum_migrations, CategoryService, CreateCategoryInput, CreateTopicInput,
        TopicService,
    };
    use rustok_outbox::TransactionalEventBus;
    use sea_orm::{
        ActiveModelTrait, ActiveValue::Set, ConnectOptions, ConnectionTrait, Database,
        DatabaseConnection, DbBackend, Statement,
    };
    use sea_orm_migration::SchemaManager;
    use serde_json::json;
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::{
        entities::seo_redirect, migrations as seo_migrations, SeoModuleSettings, SeoService,
    };
    use rustok_taxonomy::migrations as taxonomy_migrations;

    async fn test_db() -> DatabaseConnection {
        let db_url = format!(
            "sqlite:file:seo_graphql_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut opts = ConnectOptions::new(db_url);
        opts.max_connections(5)
            .min_connections(1)
            .sqlx_logging(false);
        Database::connect(opts)
            .await
            .expect("failed to connect seo graphql sqlite db")
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

    async fn seed_seo_redirects_table(db: &DatabaseConnection) {
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE seo_redirects (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                match_type TEXT NOT NULL,
                source_pattern TEXT NOT NULL,
                target_url TEXT NOT NULL,
                status_code INTEGER NOT NULL,
                expires_at TEXT NULL,
                is_active INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )"
            .to_string(),
        ))
        .await
        .expect("create seo_redirects table");
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

    async fn run_forum_migrations(db: &DatabaseConnection) {
        let manager = SchemaManager::new(db);
        for migration in forum_migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("forum migration should apply");
        }
    }

    async fn insert_enabled_seo_module(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        settings: serde_json::Value,
    ) {
        let now = chrono::Utc::now();
        rustok_tenant::entities::tenant_module::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            module_slug: Set("seo".to_string()),
            enabled: Set(true),
            settings: Set(settings.into()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await
        .expect("insert seo tenant module");
    }

    async fn insert_redirect(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        source_pattern: &str,
        target_url: &str,
        status_code: i32,
    ) {
        let now = chrono::Utc::now();
        seo_redirect::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            match_type: Set("exact".to_string()),
            source_pattern: Set(source_pattern.to_string()),
            target_url: Set(target_url.to_string()),
            status_code: Set(status_code),
            expires_at: Set(None),
            is_active: Set(true),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await
        .expect("insert seo redirect");
    }

    fn tenant_context(tenant_id: Uuid) -> TenantContext {
        TenantContext {
            id: tenant_id,
            name: "SEO Tenant".to_string(),
            slug: "seo-tenant".to_string(),
            domain: Some("seo.example.com".to_string()),
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn request_context(
        tenant_id: Uuid,
        locale: &str,
        channel_slug: Option<&str>,
    ) -> RequestContext {
        RequestContext {
            tenant_id,
            user_id: None,
            channel_id: None,
            channel_slug: channel_slug.map(|value| value.to_string()),
            channel_resolution_source: None,
            locale: locale.to_string(),
        }
    }

    fn auth_context(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "session".to_string(),
        }
    }

    fn event_bus() -> TransactionalEventBus {
        TransactionalEventBus::new(Arc::new(MemoryTransport::new()))
    }

    #[tokio::test]
    async fn graphql_seo_page_context_matches_redirect_service_contract() {
        let db = test_db().await;
        seed_tenant_modules_table(&db).await;
        seed_seo_redirects_table(&db).await;
        seed_content_routing_tables(&db).await;
        let tenant_id = Uuid::new_v4();
        insert_enabled_seo_module(&db, tenant_id, json!({})).await;
        insert_redirect(&db, tenant_id, "/legacy", "https://example.com/new", 308).await;

        let tenant = tenant_context(tenant_id);
        let expected = SeoService::new_memory(db.clone())
            .resolve_page_context(&tenant, "en-US", "/legacy")
            .await
            .expect("service seo page context should resolve")
            .expect("redirect route should resolve");

        let schema = Schema::build(SeoQuery::default(), EmptyMutation, EmptySubscription)
            .data(db.clone())
            .data(event_bus())
            .data(tenant.clone())
            .data(request_context(tenant_id, "en-US", None))
            .finish();

        let response = schema
            .execute(Request::new(
                r#"
                    query {
                        seoPageContext(route: "/legacy") {
                            route {
                                effectiveLocale
                                canonicalUrl
                                redirect {
                                    targetUrl
                                    statusCode
                                }
                            }
                            document {
                                robots {
                                    index
                                    follow
                                }
                            }
                        }
                    }
                "#,
            ))
            .await;
        assert!(
            response.errors.is_empty(),
            "seoPageContext GraphQL query should not error: {:?}",
            response.errors
        );

        let data = response
            .data
            .into_json()
            .expect("graphql seo page context should serialize");
        assert_eq!(
            data["seoPageContext"]["route"]["effectiveLocale"],
            expected.route.effective_locale
        );
        assert_eq!(
            data["seoPageContext"]["route"]["canonicalUrl"],
            expected.route.canonical_url
        );
        assert_eq!(
            data["seoPageContext"]["route"]["redirect"]["targetUrl"],
            expected
                .route
                .redirect
                .as_ref()
                .expect("service redirect should exist")
                .target_url
        );
        assert_eq!(
            data["seoPageContext"]["route"]["redirect"]["statusCode"],
            expected
                .route
                .redirect
                .as_ref()
                .expect("service redirect should exist")
                .status_code
        );
        assert_eq!(
            data["seoPageContext"]["document"]["robots"]["index"],
            expected.document.robots.index
        );
        assert_eq!(
            data["seoPageContext"]["document"]["robots"]["follow"],
            expected.document.robots.follow
        );
    }

    #[tokio::test]
    async fn graphql_seo_sitemap_status_respects_disabled_settings_and_permissions() {
        let db = test_db().await;
        seed_tenant_modules_table(&db).await;
        let tenant_id = Uuid::new_v4();
        let disabled_settings = SeoModuleSettings {
            sitemap_enabled: false,
            ..SeoModuleSettings::default()
        };
        insert_enabled_seo_module(
            &db,
            tenant_id,
            serde_json::to_value(&disabled_settings).expect("settings should serialize"),
        )
        .await;

        let tenant = tenant_context(tenant_id);
        let expected = SeoService::new_memory(db.clone())
            .sitemap_status(&tenant)
            .await
            .expect("service sitemap status should resolve");

        let schema = Schema::build(SeoQuery::default(), EmptyMutation, EmptySubscription)
            .data(db.clone())
            .data(event_bus())
            .data(tenant.clone())
            .finish();

        let unauthenticated = schema
            .execute(Request::new(
                r#"
                    query {
                        seoSitemapStatus {
                            enabled
                        }
                    }
                "#,
            ))
            .await;
        assert_eq!(unauthenticated.errors.len(), 1);
        assert!(
            unauthenticated.errors[0]
                .message
                .contains("Authentication required"),
            "missing auth should return GraphQL unauthenticated error: {:?}",
            unauthenticated.errors
        );

        let authenticated = schema
            .execute(
                Request::new(
                    r#"
                        query {
                            seoSitemapStatus {
                                enabled
                                fileCount
                                status
                                latestJobId
                            }
                        }
                    "#,
                )
                .data(auth_context(tenant_id, vec![Permission::SEO_GENERATE])),
            )
            .await;
        assert!(
            authenticated.errors.is_empty(),
            "seoSitemapStatus GraphQL query should not error: {:?}",
            authenticated.errors
        );

        let data = authenticated
            .data
            .into_json()
            .expect("graphql seo sitemap status should serialize");
        assert_eq!(data["seoSitemapStatus"]["enabled"], expected.enabled);
        assert_eq!(data["seoSitemapStatus"]["fileCount"], expected.file_count);
        assert_eq!(data["seoSitemapStatus"]["status"], json!(expected.status));
        assert_eq!(
            data["seoSitemapStatus"]["latestJobId"],
            json!(expected.latest_job_id)
        );
    }

    #[tokio::test]
    async fn graphql_seo_page_context_uses_request_channel_for_restricted_forum_topics() {
        let db = test_db().await;
        seed_tenant_modules_table(&db).await;
        seed_meta_tables(&db).await;
        seed_content_routing_tables(&db).await;
        run_seo_migrations(&db).await;
        run_taxonomy_migrations(&db).await;
        run_forum_migrations(&db).await;
        let tenant_id = Uuid::new_v4();
        insert_enabled_seo_module(&db, tenant_id, json!({})).await;

        let tenant = tenant_context(tenant_id);
        let transport = Arc::new(MemoryTransport::new());
        let _receiver = transport.subscribe();
        let event_bus = TransactionalEventBus::new(transport);
        let security = rustok_core::SecurityContext::system();

        let category = CategoryService::new(db.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "Announcements".to_string(),
                    slug: "announcements".to_string(),
                    description: Some("Mobile-only announcements".to_string()),
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
                    title: "Mobile launch".to_string(),
                    slug: Some("mobile-launch".to_string()),
                    body: "Only the mobile channel should expose this topic.".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    metadata: json!({}),
                    tags: vec![],
                    channel_slugs: Some(vec!["mobile".to_string()]),
                },
            )
            .await
            .expect("restricted forum topic should be created");

        let schema = Schema::build(SeoQuery::default(), EmptyMutation, EmptySubscription)
            .data(db.clone())
            .data(event_bus.clone())
            .data(tenant.clone())
            .data(request_context(tenant_id, "en", Some("mobile")))
            .finish();

        let response = schema
            .execute(Request::new(format!(
                r#"
                    query {{
                        seoPageContext(route: "/modules/forum?category={}&topic={}") {{
                            route {{
                                targetKind
                            }}
                            document {{
                                title
                            }}
                        }}
                    }}
                "#,
                category.id, topic.id
            )))
            .await;
        assert!(
            response.errors.is_empty(),
            "restricted forum seoPageContext should not error: {:?}",
            response.errors
        );

        let data = response
            .data
            .into_json()
            .expect("graphql seo page context should serialize");
        assert_eq!(data["seoPageContext"]["route"]["targetKind"], "forum_topic");
        assert_eq!(data["seoPageContext"]["document"]["title"], "Mobile launch");

        let no_channel_schema =
            Schema::build(SeoQuery::default(), EmptyMutation, EmptySubscription)
                .data(db.clone())
                .data(event_bus)
                .data(tenant)
                .data(request_context(tenant_id, "en", None))
                .finish();

        let no_channel = no_channel_schema
            .execute(Request::new(format!(
                r#"
                    query {{
                        seoPageContext(route: "/modules/forum?category={}&topic={}") {{
                            document {{
                                title
                            }}
                        }}
                    }}
                "#,
                category.id, topic.id
            )))
            .await;
        assert!(
            no_channel.errors.is_empty(),
            "seoPageContext without channel should not error: {:?}",
            no_channel.errors
        );
        let no_channel_data = no_channel
            .data
            .into_json()
            .expect("graphql seo page context without channel should serialize");
        assert!(
            no_channel_data["seoPageContext"].is_null(),
            "restricted forum topic should stay hidden without channel slug",
        );
    }
}
