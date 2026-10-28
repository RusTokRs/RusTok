use super::{ensure_public_blog_channel_enabled, post_is_readable, security_context_from_auth};
use crate::BlogPostStatus;
use rustok_api::AuthContext;
use rustok_api::{RequestContext, context::ChannelResolutionSource};
use rustok_channel::{BindChannelModuleInput, ChannelService, CreateChannelInput, migrations};
use rustok_core::{SecurityActorKind, SecurityContext, UserRole};
use rustok_test_utils::setup_test_db;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[test]
fn graphql_query_security_preserves_service_principal_identity() {
    let auth = AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::nil(),
        tenant_id: Uuid::new_v4(),
        permissions: vec![rustok_api::Permission::BLOG_POSTS_READ],
        client_id: Some(Uuid::new_v4()),
        scopes: vec!["content:read".to_string()],
        grant_type: "client_credentials".to_string(),
    };

    let security = security_context_from_auth(&auth);
    assert_eq!(security.actor_kind, SecurityActorKind::Service);
    assert_eq!(security.user_id, None);
}
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
        if db.get_database_backend() == sea_orm::DatabaseBackend::Sqlite
            && (migration.name() == "m20260730_000010_add_channel_index_revision"
                || migration.name() == "m20260731_000011_add_channel_index_tombstones"
                || migration.name() == "m20260807_000012_add_channel_index_identity_generation")
        {
            continue;
        }
        migration
            .up(&manager)
            .await
            .expect("channel migration should apply");
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

fn request_context(tenant_id: Uuid, channel_id: Uuid, channel_slug: &str) -> RequestContext {
    RequestContext {
        tenant_id,
        user_id: None,
        channel_id: Some(channel_id),
        channel_slug: Some(channel_slug.to_string()),
        channel_resolution_source: Some(ChannelResolutionSource::Host),
        locale: "en".to_string(),
        correlation_id: "test-correlation-id".to_string(),
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
        tenant_id,
        Some(&request_context(tenant_id, channel.id, "blog-web")),
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
        tenant_id,
        Some(&request_context(tenant_id, channel.id, "blog-web")),
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
        tenant_id,
        Some(&request_context(tenant_id, channel.id, "blog-web")),
        false,
    )
    .await;

    assert!(
        result.is_ok(),
        "missing blog binding should keep module enabled by default in v0"
    );
}

/// Visibility follows the caller's authority, not the presence of a token.
///
/// A signed-in customer sees exactly the public view (published, and visible in
/// the current channel); an editor with tenant-wide `blog_posts:read` sees
/// non-public and other-channel posts too. The previous rule treated *any*
/// authentication as authorization, which gave every customer a different view
/// of the same post than an anonymous reader.
#[test]
fn channel_visibility_follows_authority_not_authentication() {
    let channel_slugs = vec!["web".to_string()];
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let editor = SecurityContext::from_permissions(
        UserRole::Manager,
        Some(Uuid::new_v4()),
        [rustok_api::Permission::BLOG_POSTS_READ],
    );

    assert!(!post_is_readable(
        &customer,
        BlogPostStatus::Published,
        &channel_slugs,
        None
    ));
    assert!(post_is_readable(
        &customer,
        BlogPostStatus::Published,
        &channel_slugs,
        Some("web")
    ));
    assert!(!post_is_readable(
        &customer,
        BlogPostStatus::Draft,
        &channel_slugs,
        Some("web")
    ));

    assert!(post_is_readable(
        &editor,
        BlogPostStatus::Draft,
        &channel_slugs,
        None
    ));
    assert!(post_is_readable(
        &editor,
        BlogPostStatus::Published,
        &channel_slugs,
        Some("other")
    ));
}

#[tokio::test]
async fn disabled_binding_error_is_redacted() {
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
        correlation_id: "test-correlation-id".to_string(),
    };

    let error = ensure_public_blog_channel_enabled(&db, tenant_id, Some(&request_context), false)
        .await
        .expect_err("disabled binding should be reported");

    assert_eq!(
        error.message,
        "Blog is not available for the current channel"
    );
}
