use super::{
    CachedChannelResolution, ChannelResolutionCache, ChannelResolutionOutcome,
    ChannelResolutionStage, bounded_cache_component,
    build_request_facts as try_build_request_facts, build_request_facts_for_context,
    channel_cache_entry_weight, channel_cache_key_from_facts, channel_id_from_header,
    channel_slug_from_header, channel_slug_from_query, resolve_channel_context_for_facts,
    resolved_detail_source_and_trace,
};
use crate::common::RustokSettings;
use crate::context::{ChannelContext, ChannelResolutionSource};
use crate::services::server_runtime_context::ServerRuntimeContext;
use axum::http::{Extensions, HeaderMap, header::HOST};
use rustok_api::{
    context::{AuthContext, AuthContextExtension},
    request::ResolvedRequestLocale,
};
use rustok_channel::{
    ChannelResolutionRuleDefinition, ChannelResolver, ChannelService, CreateChannelInput,
    CreateChannelResolutionPolicySetInput, CreateChannelResolutionRuleInput,
    CreateChannelTargetInput, RequestFacts, ResolutionAction, ResolutionPredicate, migrations,
};
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
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE o_auth_apps (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL,
            app_type TEXT NOT NULL DEFAULT 'machine',
            is_active BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    ))
    .await
    .expect("o_auth_apps table should exist for channel foreign keys");
    let manager = SchemaManager::new(&db);
    for migration in migrations::migrations().into_iter().filter(|migration| {
        rustok_migrations::sqlite_test_migration_is_compatible(migration.name())
    }) {
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

async fn create_channel(service: &ChannelService, tenant_id: Uuid, slug: &str) -> Uuid {
    service
        .create_channel(CreateChannelInput {
            tenant_id,
            slug: slug.to_string(),
            name: slug.to_string(),
            settings: None,
        })
        .await
        .expect("channel should be created")
        .id
}

async fn add_web_target(service: &ChannelService, channel_id: Uuid, host: &str) {
    service
        .add_target(
            channel_id,
            CreateChannelTargetInput {
                target_type: "web_domain".to_string(),
                value: host.to_string(),
                is_primary: true,
                settings: None,
            },
        )
        .await
        .expect("host target should be created");
}

async fn add_locale_policy_rule(
    service: &ChannelService,
    tenant_id: Uuid,
    channel_id: Uuid,
    locale: &str,
) {
    let policy_set = service
        .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
            tenant_id,
            slug: format!("locale-{locale}"),
            name: format!("Locale {locale}"),
            is_active: true,
        })
        .await
        .expect("locale policy set should be created");

    service
        .create_resolution_rule(
            policy_set.id,
            CreateChannelResolutionRuleInput {
                priority: 10,
                is_active: true,
                definition: ChannelResolutionRuleDefinition {
                    predicates: vec![ResolutionPredicate::LocaleEquals(locale.to_string())],
                    action: ResolutionAction::ResolveToChannel { channel_id },
                },
            },
        )
        .await
        .expect("locale policy rule should be created");
}

async fn add_oauth_policy_rule(
    service: &ChannelService,
    tenant_id: Uuid,
    channel_id: Uuid,
    oauth_app_id: Uuid,
) {
    let policy_set = service
        .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
            tenant_id,
            slug: format!("oauth-{oauth_app_id}"),
            name: format!("OAuth {oauth_app_id}"),
            is_active: true,
        })
        .await
        .expect("oauth policy set should be created");

    service
        .create_resolution_rule(
            policy_set.id,
            CreateChannelResolutionRuleInput {
                priority: 10,
                is_active: true,
                definition: ChannelResolutionRuleDefinition {
                    predicates: vec![ResolutionPredicate::OAuthAppEquals(oauth_app_id)],
                    action: ResolutionAction::ResolveToChannel { channel_id },
                },
            },
        )
        .await
        .expect("oauth policy rule should be created");
}

fn test_settings() -> RustokSettings {
    RustokSettings::default()
}

fn empty_extensions() -> Extensions {
    Extensions::new()
}

fn build_request_facts(
    tenant_id: Uuid,
    headers: &HeaderMap,
    query: Option<&str>,
    peer_ip: Option<std::net::IpAddr>,
    settings: &RustokSettings,
    extensions: &Extensions,
) -> RequestFacts {
    try_build_request_facts(tenant_id, headers, query, peer_ip, settings, extensions)
        .expect("test request selectors must be valid")
}

fn sample_context(extra_bytes: usize) -> ChannelContext {
    ChannelContext {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        slug: "storefront".to_string(),
        name: "Storefront".to_string(),
        is_active: true,
        status: "active".to_string(),
        target_type: Some("web_domain".to_string()),
        target_value: Some("shop.example.test".to_string()),
        settings: serde_json::json!({"payload": "x".repeat(extra_bytes)}),
        resolution_source: ChannelResolutionSource::Default,
        resolution_trace: Vec::new(),
    }
}

#[test]
fn channel_cache_key_hashes_dynamic_request_values() {
    let value = "private-selector";
    let component = bounded_cache_component(value);
    assert_ne!(component, value);
    assert!(component.starts_with("sha256-"));
    assert_eq!(component.len(), "sha256-".len() + 64);
}

#[test]
fn channel_cache_weight_accounts_for_cached_context_size() {
    let facts = build_request_facts(
        Uuid::new_v4(),
        &HeaderMap::new(),
        Some("channel=storefront"),
        None,
        &test_settings(),
        &empty_extensions(),
    );
    let key = channel_cache_key_from_facts(&facts, 1);
    let short = CachedChannelResolution::Found(Box::new(sample_context(16)));
    let large = CachedChannelResolution::Found(Box::new(sample_context(4_096)));

    assert!(channel_cache_entry_weight(&key, &large) > channel_cache_entry_weight(&key, &short));
    assert!(
        ChannelResolutionCache::ttl_for(&CachedChannelResolution::Missing)
            < ChannelResolutionCache::ttl_for(&short)
    );
}

#[test]
fn request_facts_include_auth_and_locale_extensions() {
    let tenant_id = Uuid::new_v4();
    let client_id = Uuid::new_v4();
    let mut extensions = Extensions::new();
    extensions.insert(AuthContextExtension(AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: Vec::new(),
        client_id: Some(client_id),
        scopes: vec!["catalog:read".to_string()],
        grant_type: "client_credentials".to_string(),
    }));
    extensions.insert(ResolvedRequestLocale {
        requested_locale: Some("ru".to_string()),
        effective_locale: "ru-RU".to_string(),
    });

    let facts = build_request_facts(
        tenant_id,
        &HeaderMap::new(),
        None,
        None,
        &test_settings(),
        &extensions,
    );

    assert_eq!(facts.oauth_app_id, Some(client_id));
    assert_eq!(facts.locale.as_deref(), Some("ru-RU"));
}

#[test]
fn channel_cache_key_varies_by_oauth_app_and_locale() {
    let tenant_id = Uuid::new_v4();
    let client_id = Uuid::new_v4();

    let base_facts = build_request_facts(
        tenant_id,
        &HeaderMap::new(),
        Some("channel=storefront"),
        None,
        &test_settings(),
        &empty_extensions(),
    );

    let mut locale_facts = base_facts.clone();
    locale_facts.locale = Some("ru-RU".to_string());
    let mut oauth_facts = base_facts.clone();
    oauth_facts.oauth_app_id = Some(client_id);

    let base_key = channel_cache_key_from_facts(&base_facts, 7);
    let locale_key = channel_cache_key_from_facts(&locale_facts, 7);
    let oauth_key = channel_cache_key_from_facts(&oauth_facts, 7);

    assert_ne!(base_key, locale_key);
    assert_ne!(base_key, oauth_key);
    assert_ne!(locale_key, oauth_key);
}

#[test]
fn parses_channel_id_header() {
    let mut headers = HeaderMap::new();
    let channel_id = Uuid::new_v4();
    headers.insert(
        "X-Channel-ID",
        channel_id.to_string().parse().expect("header"),
    );

    assert_eq!(channel_id_from_header(&headers), Ok(Some(channel_id)));
}

#[test]
fn parses_channel_slug_from_header_and_query() {
    let mut headers = HeaderMap::new();
    headers.insert("X-Channel-Slug", "mobile-app".parse().expect("header"));

    assert_eq!(
        channel_slug_from_header(&headers)
            .expect("valid channel slug")
            .as_deref(),
        Some("mobile-app")
    );
    assert_eq!(
        channel_slug_from_query(Some("locale=ru&channel=web-store"))
            .expect("valid query channel slug")
            .as_deref(),
        Some("web-store")
    );
}

#[test]
fn rejects_oversized_channel_selectors_before_resolution() {
    let oversized = "x".repeat(101);

    let mut headers = HeaderMap::new();
    headers.insert("X-Channel-Slug", oversized.parse().expect("header"));

    assert!(channel_slug_from_header(&headers).is_err());
    assert!(channel_slug_from_query(Some(&format!("channel={oversized}"))).is_err());
}

#[test]
fn decodes_percent_encoded_query_channel_slug() {
    assert_eq!(
        channel_slug_from_query(Some("channel=web%2Dstore"))
            .expect("valid encoded query channel")
            .as_deref(),
        Some("web-store")
    );
    assert_eq!(
        channel_slug_from_query(Some("channel=mobile+app"))
            .expect("valid plus-encoded query channel")
            .as_deref(),
        Some("mobile app")
    );
}

#[test]
fn cache_key_canonicalizes_equivalent_hosts() {
    let tenant_id = Uuid::new_v4();
    let base = build_request_facts(
        tenant_id,
        &HeaderMap::new(),
        None,
        None,
        &test_settings(),
        &empty_extensions(),
    );

    let mut mixed_case = base.clone();
    mixed_case.host = Some("SHOP.Example.TEST".to_string());

    let mut with_port_and_dot = base.clone();
    with_port_and_dot.host = Some("shop.example.test.:443".to_string());

    let mut canonical = base;
    canonical.host = Some("shop.example.test".to_string());

    let canonical_key = channel_cache_key_from_facts(&canonical, 1);
    assert_eq!(channel_cache_key_from_facts(&mixed_case, 1), canonical_key);
    assert_eq!(
        channel_cache_key_from_facts(&with_port_and_dot, 1),
        canonical_key
    );
}

#[test]
fn request_facts_reject_malformed_and_ambiguous_channel_selectors() {
    let tenant_id = Uuid::new_v4();
    let settings = test_settings();
    let extensions = empty_extensions();

    let mut malformed_id = HeaderMap::new();
    malformed_id.insert("X-Channel-ID", "not-a-uuid".parse().expect("header"));
    assert!(
        try_build_request_facts(tenant_id, &malformed_id, None, None, &settings, &extensions,)
            .is_err()
    );

    let mut nil_id = HeaderMap::new();
    nil_id.insert(
        "X-Channel-ID",
        Uuid::nil().to_string().parse().expect("header"),
    );
    assert!(
        try_build_request_facts(tenant_id, &nil_id, None, None, &settings, &extensions,).is_err()
    );

    let mut conflicting_ids = HeaderMap::new();
    conflicting_ids.append(
        "X-Channel-ID",
        Uuid::new_v4().to_string().parse().expect("header"),
    );
    conflicting_ids.append(
        "X-Channel-ID",
        Uuid::new_v4().to_string().parse().expect("header"),
    );
    assert!(
        try_build_request_facts(
            tenant_id,
            &conflicting_ids,
            None,
            None,
            &settings,
            &extensions,
        )
        .is_err()
    );

    assert!(
        try_build_request_facts(
            tenant_id,
            &HeaderMap::new(),
            Some("channel=&channel=valid"),
            None,
            &settings,
            &extensions,
        )
        .is_err()
    );
}

#[tokio::test]
async fn deferred_tenant_resolution_can_resolve_the_same_host_channel_as_http() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());
    let channel_id = create_channel(&service, tenant_id, "ws-host-channel").await;
    add_web_target(&service, channel_id, "ws.example.test").await;

    let settings = test_settings();
    let mut headers = HeaderMap::new();
    headers.insert(HOST, "ws.example.test".parse().expect("host header"));
    let facts = build_request_facts_for_context(
        tenant_id,
        &headers,
        None,
        None,
        &settings,
        None,
        Some("en".to_string()),
    )
    .expect("websocket facts should use the canonical channel selectors");
    let runtime_ctx = ServerRuntimeContext::new(db, settings);

    let resolved = resolve_channel_context_for_facts(&runtime_ctx, facts.clone())
        .await
        .expect("channel resolution should succeed")
        .expect("host target should resolve a channel");
    assert_eq!(resolved.id, channel_id);
    assert_eq!(resolved.tenant_id, tenant_id);
    assert_eq!(resolved.target_value.as_deref(), Some("ws.example.test"));

    let cached = resolve_channel_context_for_facts(&runtime_ctx, facts)
        .await
        .expect("cached channel resolution should succeed")
        .expect("cached host target should remain available");
    assert_eq!(cached.id, channel_id);
    assert_eq!(cached.target_value.as_deref(), Some("ws.example.test"));
}

#[tokio::test]
async fn cached_host_resolution_preserves_non_primary_matched_target() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let channel_id = create_channel(&service, tenant_id, "host-channel").await;
    add_web_target(&service, channel_id, "primary.example.test").await;
    service
        .add_target(
            channel_id,
            CreateChannelTargetInput {
                target_type: "web_domain".to_string(),
                value: "secondary.example.test".to_string(),
                is_primary: false,
                settings: None,
            },
        )
        .await
        .expect("secondary host target should be created");

    let mut headers = HeaderMap::new();
    headers.insert(HOST, "secondary.example.test".parse().expect("host header"));

    let facts = build_request_facts(
        tenant_id,
        &headers,
        None,
        None,
        &test_settings(),
        &empty_extensions(),
    );
    let resolver = ChannelResolver::new(db);
    let decision = resolver
        .resolve(&facts)
        .await
        .expect("host resolution should succeed");
    assert_eq!(
        decision.source,
        Some(rustok_channel::ChannelResolutionOrigin::Host)
    );

    let cached = CachedChannelResolution::from_decision(decision, &facts);
    let CachedChannelResolution::Found(context) = cached else {
        panic!("host resolution should produce a cached context");
    };

    assert_eq!(context.target_type.as_deref(), Some("web_domain"));
    assert_eq!(
        context.target_value.as_deref(),
        Some("secondary.example.test")
    );
}

#[tokio::test]
async fn select_channel_prefers_header_id_over_slug_query_host_and_default() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let _default_channel_id = create_channel(&service, tenant_id, "default").await;
    let header_id_channel_id = create_channel(&service, tenant_id, "header-id").await;
    let _header_slug_channel_id = create_channel(&service, tenant_id, "header-slug").await;
    let _query_channel_id = create_channel(&service, tenant_id, "query-channel").await;
    let host_channel_id = create_channel(&service, tenant_id, "host-channel").await;
    add_web_target(&service, host_channel_id, "shop.example.test").await;

    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Channel-ID",
        header_id_channel_id
            .to_string()
            .parse()
            .expect("channel id header"),
    );
    headers.insert(
        "X-Channel-Slug",
        "header-slug".parse().expect("slug header"),
    );
    headers.insert(HOST, "shop.example.test".parse().expect("host header"));

    let resolver = ChannelResolver::new(db.clone());
    let selected = resolved_detail_source_and_trace(
        resolver
            .resolve(&build_request_facts(
                tenant_id,
                &headers,
                Some("channel=query-channel"),
                None,
                &test_settings(),
                &empty_extensions(),
            ))
            .await
            .expect("resolution should succeed"),
    )
    .expect("channel should be resolved");

    assert_eq!(selected.0.channel.id, header_id_channel_id);
    assert_eq!(selected.0.channel.slug, "header-id");
    assert_eq!(selected.1, ChannelResolutionSource::HeaderId);
}

#[tokio::test]
async fn select_channel_falls_back_from_missing_query_to_host() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let _default_channel_id = create_channel(&service, tenant_id, "default").await;
    let host_channel_id = create_channel(&service, tenant_id, "host-channel").await;
    add_web_target(&service, host_channel_id, "https://shop.example.test/").await;

    let mut headers = HeaderMap::new();
    headers.insert(HOST, "SHOP.EXAMPLE.TEST.:443".parse().expect("host header"));

    let resolver = ChannelResolver::new(db.clone());
    let selected = resolved_detail_source_and_trace(
        resolver
            .resolve(&build_request_facts(
                tenant_id,
                &headers,
                Some("channel=missing"),
                None,
                &test_settings(),
                &empty_extensions(),
            ))
            .await
            .expect("resolution should succeed"),
    )
    .expect("host fallback should resolve");

    assert_eq!(selected.0.channel.id, host_channel_id);
    assert_eq!(selected.0.channel.slug, "host-channel");
    assert_eq!(selected.1, ChannelResolutionSource::Host);
    assert_eq!(selected.0.targets.len(), 1);
    assert_eq!(selected.0.targets[0].value, "shop.example.test");
}

#[tokio::test]
async fn select_channel_falls_back_to_default_when_no_selector_matches() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let first_channel_id = create_channel(&service, tenant_id, "default").await;
    let explicit_default_channel_id = create_channel(&service, tenant_id, "secondary").await;
    service
        .set_default_channel(explicit_default_channel_id)
        .await
        .expect("explicit default channel should be saved");

    let headers = HeaderMap::new();
    let resolver = ChannelResolver::new(db.clone());
    let selected = resolved_detail_source_and_trace(
        resolver
            .resolve(&build_request_facts(
                tenant_id,
                &headers,
                Some("channel=missing"),
                None,
                &test_settings(),
                &empty_extensions(),
            ))
            .await
            .expect("resolution should succeed"),
    )
    .expect("default fallback should resolve");

    assert_ne!(selected.0.channel.id, first_channel_id);
    assert_eq!(selected.0.channel.id, explicit_default_channel_id);
    assert_eq!(selected.0.channel.slug, "secondary");
    assert_eq!(selected.1, ChannelResolutionSource::Default);
}

#[tokio::test]
async fn select_channel_skips_inactive_explicit_slug_and_uses_host_fallback() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let inactive_channel_id = create_channel(&service, tenant_id, "inactive").await;
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "UPDATE channels SET is_active = ? WHERE id = ?",
        [false.into(), inactive_channel_id.into()],
    ))
    .await
    .expect("channel should be deactivated");

    let host_channel_id = create_channel(&service, tenant_id, "host-channel").await;
    add_web_target(&service, host_channel_id, "shop.example.test").await;

    let mut headers = HeaderMap::new();
    headers.insert("X-Channel-Slug", "inactive".parse().expect("slug header"));
    headers.insert(HOST, "SHOP.EXAMPLE.TEST.:443".parse().expect("host header"));

    let resolver = ChannelResolver::new(db.clone());
    let selected = resolved_detail_source_and_trace(
        resolver
            .resolve(&build_request_facts(
                tenant_id,
                &headers,
                None,
                None,
                &test_settings(),
                &empty_extensions(),
            ))
            .await
            .expect("resolution should succeed"),
    )
    .expect("inactive channel must be skipped");

    assert_eq!(selected.0.channel.id, host_channel_id);
    assert_eq!(selected.0.channel.slug, "host-channel");
    assert_eq!(selected.1, ChannelResolutionSource::Host);
}

#[tokio::test]
async fn runtime_locale_extension_can_select_policy_channel() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let default_channel_id = create_channel(&service, tenant_id, "default").await;
    let locale_channel_id = create_channel(&service, tenant_id, "locale-ru").await;
    add_locale_policy_rule(&service, tenant_id, locale_channel_id, "ru-by").await;

    let mut extensions = Extensions::new();
    extensions.insert(ResolvedRequestLocale {
        requested_locale: Some("ru".to_string()),
        effective_locale: "RU_BY".to_string(),
    });

    let resolver = ChannelResolver::new(db);
    let selected = resolved_detail_source_and_trace(
        resolver
            .resolve(&build_request_facts(
                tenant_id,
                &HeaderMap::new(),
                None,
                None,
                &test_settings(),
                &extensions,
            ))
            .await
            .expect("resolution should succeed"),
    )
    .expect("locale policy channel should resolve");

    assert_eq!(selected.0.channel.id, locale_channel_id);
    assert_ne!(selected.0.channel.id, default_channel_id);
    assert_eq!(selected.1, ChannelResolutionSource::Policy);
}

#[tokio::test]
async fn runtime_oauth_extension_can_select_policy_channel() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    let client_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let default_channel_id = create_channel(&service, tenant_id, "default").await;
    let oauth_channel_id = create_channel(&service, tenant_id, "oauth-app").await;
    add_oauth_policy_rule(&service, tenant_id, oauth_channel_id, client_id).await;

    let mut extensions = Extensions::new();
    extensions.insert(AuthContextExtension(AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: Vec::new(),
        client_id: Some(client_id),
        scopes: vec!["catalog:read".to_string()],
        grant_type: "client_credentials".to_string(),
    }));

    let resolver = ChannelResolver::new(db);
    let selected = resolved_detail_source_and_trace(
        resolver
            .resolve(&build_request_facts(
                tenant_id,
                &HeaderMap::new(),
                None,
                None,
                &test_settings(),
                &extensions,
            ))
            .await
            .expect("resolution should succeed"),
    )
    .expect("oauth policy channel should resolve");

    assert_eq!(selected.0.channel.id, oauth_channel_id);
    assert_ne!(selected.0.channel.id, default_channel_id);
    assert_eq!(selected.1, ChannelResolutionSource::Policy);
}

#[tokio::test]
async fn resolved_context_keeps_resolution_trace_for_runtime_diagnostics() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let _default_channel_id = create_channel(&service, tenant_id, "default").await;
    let host_channel_id = create_channel(&service, tenant_id, "host-channel").await;
    add_web_target(&service, host_channel_id, "shop.example.test").await;

    let mut headers = HeaderMap::new();
    headers.insert(HOST, "shop.example.test".parse().expect("host header"));

    let resolver = ChannelResolver::new(db);
    let selected = resolved_detail_source_and_trace(
        resolver
            .resolve(&build_request_facts(
                tenant_id,
                &headers,
                Some("channel=missing"),
                None,
                &test_settings(),
                &empty_extensions(),
            ))
            .await
            .expect("resolution should succeed"),
    )
    .expect("host fallback should resolve");

    assert!(
        selected
            .2
            .iter()
            .any(|step| step.stage == ChannelResolutionStage::Query
                && step.outcome == ChannelResolutionOutcome::Miss),
        "trace should preserve pre-host misses for runtime diagnostics"
    );
    assert!(
        selected
            .2
            .iter()
            .any(|step| step.stage == ChannelResolutionStage::Host
                && step.outcome == ChannelResolutionOutcome::Matched),
    );
}

#[test]
fn channel_id_from_header_rejects_conflicting_duplicates() {
    let mut headers = HeaderMap::new();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    headers.append(
        "X-Channel-ID",
        id1.to_string().parse().expect("valid header"),
    );
    headers.append(
        "X-Channel-ID",
        id2.to_string().parse().expect("valid header"),
    );
    assert!(channel_id_from_header(&headers).is_err());

    let mut headers_matching = HeaderMap::new();
    headers_matching.append(
        "X-Channel-ID",
        id1.to_string().parse().expect("valid header"),
    );
    headers_matching.append(
        "X-Channel-ID",
        id1.to_string().parse().expect("valid header"),
    );
    assert_eq!(channel_id_from_header(&headers_matching), Ok(Some(id1)));
}

#[test]
fn channel_slug_from_header_rejects_conflicting_duplicates() {
    let mut headers = HeaderMap::new();
    headers.append("X-Channel-Slug", "channel-a".parse().expect("valid header"));
    headers.append("X-Channel-Slug", "channel-b".parse().expect("valid header"));
    assert!(channel_slug_from_header(&headers).is_err());

    let mut headers_matching = HeaderMap::new();
    headers_matching.append("X-Channel-Slug", "channel-a".parse().expect("valid header"));
    headers_matching.append("X-Channel-Slug", "channel-a".parse().expect("valid header"));
    assert_eq!(
        channel_slug_from_header(&headers_matching),
        Ok(Some("channel-a".to_string()))
    );
}

#[test]
fn channel_slug_from_query_rejects_conflicting_duplicates() {
    assert!(channel_slug_from_query(Some("channel=channel-a&channel=channel-b")).is_err());
    assert_eq!(
        channel_slug_from_query(Some("channel=channel-a&channel=channel-a")),
        Ok(Some("channel-a".to_string()))
    );
    assert_eq!(
        channel_slug_from_query(Some("channel=channel-a")),
        Ok(Some("channel-a".to_string()))
    );
}
