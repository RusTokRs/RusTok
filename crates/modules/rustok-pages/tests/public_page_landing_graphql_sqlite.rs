//! Public GraphQL contract of a published page.
//!
//! A storefront renders the published landing of a page (`landing`): the sanitized markup and
//! stylesheet stored by the reviewed publish. The page body is the authoring document; it is
//! returned only to a principal that may save or publish it, never to public readers or
//! customers, and a persisted body mutation that was not published does not reach `landing`.
//!
//! Signing in does not make a reader an author: a customer token gets what an anonymous
//! reader gets. A `client_credentials` token is not a user: its subject id never makes it the
//! owner of a page, so it reads the document only through a grant that covers every page. The
//! landing is read through the generation-aware Pages cache when the host provides one.

use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_graphql::{EmptySubscription, Request as GraphqlRequest, Schema, Variables};
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use rustok_api::{AuthContext, Permission, RequestContext, TenantContext};
use rustok_channel::ChannelModule;
use rustok_core::{MigrationSource, Rbac, SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::{
    page, page_body, page_published_landing_artifact, page_static_landing_artifact,
};
use rustok_pages::graphql::PagesGraphqlRuntimeData;
use rustok_pages::services::{PageBuilderArtifactService, PageService};
use rustok_pages::{
    PAGES_STOREFRONT_CACHE_TTL_SECS, PageCacheError, PageCacheGenerationSnapshot,
    PagesCacheReadPort, PagesCacheReadRuntime, PagesModule, PagesMutation, PagesQuery,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::{Value, json};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
type PagesSchema = Schema<PagesQuery, PagesMutation, EmptySubscription>;

const PAGE_BY_SLUG_QUERY: &str = r#"
query StorefrontPage($slug: String!, $locale: String) {
  pageBySlug(slug: $slug, locale: $locale) {
    id
    status
    effectiveLocale
    translation { title slug }
    body { locale content format }
    landing { locale html css artifactHash }
  }
}
"#;

const PAGE_SOURCE_BY_SLUG_QUERY: &str = r#"
query PageSource($slug: String!, $locale: String) {
  pageBySlug(slug: $slug, locale: $locale) {
    documentReadable
    body { locale content }
    landing { artifactHash }
  }
}
"#;

const PAGE_BY_ID_QUERY: &str = r#"
query Page($id: UUID!, $locale: String) {
  page(id: $id, locale: $locale) {
    id
    body { locale content format }
    landing { locale html css artifactHash }
  }
}
"#;

#[derive(Clone)]
struct PublishedFixture {
    page_id: Uuid,
    body_id: Uuid,
    artifact_hash: String,
    document_html: String,
}

#[tokio::test]
async fn public_page_by_slug_returns_the_published_landing_and_no_source() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let fixture = create_reviewed_published_page(&db, event_bus(&db), tenant_id).await?;
    enable_pages_module(&db, tenant_id).await?;
    let artifact = PageBuilderArtifactService::new(db.clone())
        .load_public_bound_artifact_with_fallback(
            tenant_id,
            fixture.page_id,
            "en",
            None,
            Some("web"),
        )
        .await?
        .ok_or_else(|| std::io::Error::other("published artifact is missing"))?;
    assert_eq!(artifact.artifact_hash, fixture.artifact_hash);
    assert_eq!(artifact.document_html, fixture.document_html);

    let page = page_by_slug(&schema(&db, tenant_id, Some("web"), None)).await?;
    assert_eq!(page["id"], json!(fixture.page_id.to_string()));
    assert_eq!(page["effectiveLocale"], json!("en"));
    assert_eq!(page["translation"]["slug"], json!("home"));
    assert_eq!(page["body"], Value::Null);

    let landing = &page["landing"];
    assert_eq!(landing["locale"], json!("en"));
    assert_eq!(landing["artifactHash"], json!(fixture.artifact_hash));
    assert_eq!(landing["html"], json!(artifact.body_html));
    assert_eq!(landing["css"], json!(artifact.css));
    let html = landing["html"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("landing html is not a string"))?;
    assert!(html.contains("Published immutable artifact"));
    for document_only in ["<!doctype", "<html", "<head>", "<body", "<style"] {
        assert!(
            !html.contains(document_only),
            "landing html is body markup, not a document: found {document_only}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn unpublished_body_mutation_never_reaches_public_readers() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let fixture = create_reviewed_published_page(&db, event_bus(&db), tenant_id).await?;
    enable_pages_module(&db, tenant_id).await?;
    let public = schema(&db, tenant_id, Some("web"), None);
    let before = page_by_slug(&public).await?;

    persist_new_draft_body(&db, tenant_id, &fixture).await?;

    let after = page_by_slug(&public).await?;
    assert_eq!(after, before);
    assert!(!after.to_string().contains("Draft-only mutation"));

    let by_id = execute(
        &public,
        PAGE_BY_ID_QUERY,
        json!({ "id": fixture.page_id.to_string(), "locale": "en" }),
    )
    .await?;
    assert_eq!(by_id["page"]["body"], Value::Null);
    assert_eq!(by_id["page"]["landing"], before["landing"]);
    assert!(!by_id.to_string().contains("Draft-only mutation"));
    Ok(())
}

#[tokio::test]
async fn landing_follows_the_request_channel() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let fixture = create_reviewed_published_page(&db, event_bus(&db), tenant_id).await?;
    enable_pages_module(&db, tenant_id).await?;

    for channel in [Some("mobile"), None] {
        let data = execute(
            &schema(&db, tenant_id, channel, None),
            PAGE_BY_SLUG_QUERY,
            json!({ "slug": "home", "locale": "en" }),
        )
        .await?;
        assert_eq!(data["pageBySlug"], Value::Null);
    }

    // A signed-in customer is a visitor of the request channel.
    let customer = execute(
        &schema(
            &db,
            tenant_id,
            Some("mobile"),
            Some(auth(tenant_id, role_permissions(&UserRole::Customer))),
        ),
        PAGE_BY_SLUG_QUERY,
        json!({ "slug": "home", "locale": "en" }),
    )
    .await?;
    assert_eq!(customer["pageBySlug"], Value::Null);

    // A page author resolves the page on any channel, but the landing is what a visitor of
    // the request channel gets: none where the page is not published.
    let editor = auth(tenant_id, role_permissions(&UserRole::Manager));
    let on_other_channel = page_by_slug(&schema(
        &db,
        tenant_id,
        Some("mobile"),
        Some(editor.clone()),
    ))
    .await?;
    assert_eq!(on_other_channel["id"], json!(fixture.page_id.to_string()));
    assert_eq!(on_other_channel["landing"], Value::Null);

    let on_page_channel = page_by_slug(&schema(&db, tenant_id, Some("web"), Some(editor))).await?;
    assert_eq!(
        on_page_channel["landing"]["artifactHash"],
        json!(fixture.artifact_hash)
    );
    Ok(())
}

#[tokio::test]
async fn page_source_requires_update_authority_not_a_signed_in_token() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let fixture = create_reviewed_published_page(&db, event_bus(&db), tenant_id).await?;
    enable_pages_module(&db, tenant_id).await?;

    // The role label of a token is inferred from its permissions; the default customer set
    // must not be mistaken for staff when the source is at stake.
    let customer_permissions = role_permissions(&UserRole::Customer);
    assert!(customer_permissions.contains(&Permission::PAGES_READ));
    assert!(!customer_permissions.contains(&Permission::PAGES_UPDATE));
    let customer = page_by_slug(&schema(
        &db,
        tenant_id,
        Some("web"),
        Some(auth(tenant_id, customer_permissions)),
    ))
    .await?;
    assert_eq!(customer["body"], Value::Null);
    assert_eq!(
        customer["landing"]["artifactHash"],
        json!(fixture.artifact_hash)
    );

    let editor_permissions = role_permissions(&UserRole::Manager);
    assert!(editor_permissions.contains(&Permission::PAGES_UPDATE));
    let editor = page_by_slug(&schema(
        &db,
        tenant_id,
        Some("web"),
        Some(auth(tenant_id, editor_permissions)),
    ))
    .await?;
    assert_eq!(editor["body"]["locale"], json!("en"));
    assert!(
        editor["body"]["content"]
            .as_str()
            .is_some_and(|content| content.contains("Published immutable artifact"))
    );
    assert_eq!(
        editor["landing"]["artifactHash"],
        json!(fixture.artifact_hash)
    );
    Ok(())
}

#[tokio::test]
async fn a_service_token_never_owns_a_page_by_its_subject_id() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let fixture = create_reviewed_published_page(&db, event_bus(&db), tenant_id).await?;
    enable_pages_module(&db, tenant_id).await?;

    // The page is authored by the subject of the tokens below.
    let subject = Uuid::new_v4();
    let mut authored: page::ActiveModel = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("published page row is missing"))?
        .into();
    authored.author_id = Set(Some(subject));
    authored.update(&db).await?;

    // `pages:update` without `pages:manage` and without a staff label reaches the caller's
    // own pages only, so who the caller is decides whether the source is returned.
    let own_pages_only = vec![
        Permission::PAGES_READ,
        Permission::PAGES_LIST,
        Permission::PAGES_UPDATE,
    ];
    let source = |grant_type: &str, permissions: Vec<Permission>| {
        let schema = schema(
            &db,
            tenant_id,
            Some("web"),
            Some(auth_with_grant(tenant_id, subject, grant_type, permissions)),
        );
        async move {
            let data = execute(
                &schema,
                PAGE_SOURCE_BY_SLUG_QUERY,
                json!({ "slug": "home", "locale": "en" }),
            )
            .await?;
            TestResult::Ok(data["pageBySlug"].clone())
        }
    };

    let user = source("direct", own_pages_only.clone()).await?;
    assert_eq!(user["documentReadable"], json!(true));
    assert!(
        user["body"]["content"]
            .as_str()
            .is_some_and(|content| content.contains("Published immutable artifact"))
    );

    // The subject of a `client_credentials` token is an application id. It equals the page
    // author here, and still it is not a user that owns the page.
    let service = source("client_credentials", own_pages_only).await?;
    assert_eq!(service["documentReadable"], json!(false));
    assert_eq!(service["body"], Value::Null);
    assert_eq!(
        service["landing"]["artifactHash"],
        json!(fixture.artifact_hash)
    );

    // A service reads the source through a grant that covers every page.
    let managing_service = source(
        "client_credentials",
        vec![Permission::PAGES_READ, Permission::PAGES_MANAGE],
    )
    .await?;
    assert_eq!(managing_service["documentReadable"], json!(true));
    assert_eq!(managing_service["body"]["locale"], json!("en"));
    Ok(())
}

#[tokio::test]
async fn an_unpublished_page_does_not_exist_for_visitors() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    enable_pages_module(&db, tenant_id).await?;
    let page_id = create_unpublished_page(&db, tenant_id).await?;
    let by_id = json!({ "id": page_id.to_string(), "locale": "en" });

    // Neither an anonymous reader nor a signed-in customer learns that the id is taken: the
    // read answers `null`, not a permission error.
    let customer = auth(tenant_id, role_permissions(&UserRole::Customer));
    for visitor in [None, Some(customer)] {
        let visitor_schema = schema(&db, tenant_id, Some("web"), visitor);
        let data = execute(&visitor_schema, PAGE_BY_ID_QUERY, by_id.clone()).await?;
        assert_eq!(data["page"], Value::Null);
        let data = execute(
            &visitor_schema,
            PAGE_BY_SLUG_QUERY,
            json!({ "slug": "upcoming", "locale": "en" }),
        )
        .await?;
        assert_eq!(data["pageBySlug"], Value::Null);
    }

    let author = auth(tenant_id, role_permissions(&UserRole::Manager));
    let author_schema = schema(&db, tenant_id, Some("web"), Some(author));
    let data = execute(&author_schema, PAGE_BY_ID_QUERY, by_id).await?;
    assert_eq!(data["page"]["id"], json!(page_id.to_string()));
    assert_eq!(data["page"]["landing"], Value::Null);

    // The by-slug read is the storefront route lookup: it resolves published pages only,
    // for an author too. An unpublished page is read by id.
    let data = execute(
        &author_schema,
        PAGE_BY_SLUG_QUERY,
        json!({ "slug": "upcoming", "locale": "en" }),
    )
    .await?;
    assert_eq!(data["pageBySlug"], Value::Null);
    Ok(())
}

#[tokio::test]
async fn landing_is_read_through_the_generation_aware_cache() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let fixture = create_reviewed_published_page(&db, event_bus(&db), tenant_id).await?;
    enable_pages_module(&db, tenant_id).await?;
    let port = Arc::new(RecordingCachePort::default());
    let cache = PagesCacheReadRuntime::new(port.clone());
    let public = schema_with_cache(&db, tenant_id, Some("web"), None, Some(cache.clone()));

    let miss = page_by_slug(&public).await?;
    assert_eq!(
        miss["landing"]["artifactHash"],
        json!(fixture.artifact_hash)
    );
    assert_eq!(
        port.put_ttls(),
        vec![Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS)]
    );

    // The entry belongs to one channel: an author on another channel gets no landing and
    // leaves no entry ("no landing" is never stored).
    let author = auth(tenant_id, role_permissions(&UserRole::Manager));
    let on_other_channel = page_by_slug(&schema_with_cache(
        &db,
        tenant_id,
        Some("mobile"),
        Some(author),
        Some(cache),
    ))
    .await?;
    assert_eq!(on_other_channel["landing"], Value::Null);
    assert_eq!(port.put_ttls().len(), 1);

    // Remove the binding behind the cache: a hit is served without the artifact owner.
    page_published_landing_artifact::Entity::delete_by_id(fixture.body_id)
        .exec(&db)
        .await?;
    let hit = page_by_slug(&public).await?;
    assert_eq!(hit["landing"], miss["landing"]);
    assert_eq!(port.put_ttls().len(), 1);

    // A failing cache read degrades to the owner, which now has no landing.
    port.fail_reads(true);
    let degraded = page_by_slug(&public).await?;
    assert_eq!(degraded["landing"], Value::Null);
    port.fail_reads(false);

    // Every page event rotates the key. A page update rotates only the route and page
    // generations, and the entry must not survive it: the hit skips the owner's status and
    // channel-visibility decision.
    port.set_generations(PageCacheGenerationSnapshot::new(1, 1, 0));
    let after_update = page_by_slug(&public).await?;
    assert_eq!(after_update["landing"], Value::Null);
    assert_eq!(port.put_ttls().len(), 1);
    Ok(())
}

#[derive(Default)]
struct CacheState {
    generations: PageCacheGenerationSnapshot,
    values: std::collections::HashMap<String, Vec<u8>>,
    put_ttls: Vec<Duration>,
    fail_reads: bool,
}

#[derive(Default)]
struct RecordingCachePort {
    state: Mutex<CacheState>,
}

impl RecordingCachePort {
    fn state(&self) -> std::sync::MutexGuard<'_, CacheState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn put_ttls(&self) -> Vec<Duration> {
        self.state().put_ttls.clone()
    }

    fn set_generations(&self, generations: PageCacheGenerationSnapshot) {
        self.state().generations = generations;
    }

    fn fail_reads(&self, fail: bool) {
        self.state().fail_reads = fail;
    }
}

#[async_trait]
impl PagesCacheReadPort for RecordingCachePort {
    async fn generation_snapshot(
        &self,
        _tenant_id: Uuid,
    ) -> Result<PageCacheGenerationSnapshot, PageCacheError> {
        Ok(self.state().generations)
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PageCacheError> {
        let state = self.state();
        if state.fail_reads {
            return Err(PageCacheError::InvalidRequest(
                "injected cache read failure".to_string(),
            ));
        }
        Ok(state.values.get(key).cloned())
    }

    async fn put(&self, key: String, value: Vec<u8>, ttl: Duration) -> Result<(), PageCacheError> {
        let mut state = self.state();
        state.put_ttls.push(ttl);
        state.values.insert(key, value);
        Ok(())
    }
}

fn event_bus(db: &DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())))
}

fn role_permissions(role: &UserRole) -> Vec<Permission> {
    Rbac::permissions_for_role(role).iter().copied().collect()
}

fn auth(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
    auth_with_grant(tenant_id, Uuid::new_v4(), "direct", permissions)
}

fn auth_with_grant(
    tenant_id: Uuid,
    subject_id: Uuid,
    grant_type: &str,
    permissions: Vec<Permission>,
) -> AuthContext {
    AuthContext {
        user_id: subject_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions,
        client_id: None,
        scopes: Vec::new(),
        grant_type: grant_type.to_string(),
    }
}

fn schema(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    channel_slug: Option<&str>,
    auth: Option<AuthContext>,
) -> PagesSchema {
    schema_with_cache(db, tenant_id, channel_slug, auth, None)
}

fn schema_with_cache(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    channel_slug: Option<&str>,
    auth: Option<AuthContext>,
    cache: Option<PagesCacheReadRuntime>,
) -> PagesSchema {
    let builder = Schema::build(
        PagesQuery::default(),
        PagesMutation::default(),
        EmptySubscription,
    )
    .data(db.clone())
    .data(event_bus(db))
    .data(TenantContext {
        id: tenant_id,
        name: "Pages public landing".to_string(),
        slug: format!("pages-landing-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    })
    // The channel is addressed by slug only: without a channel id the per-channel module
    // binding is not consulted, so the page channel allowlist is the gate under test.
    .data(RequestContext {
        tenant_id,
        user_id: auth.as_ref().map(|auth| auth.user_id),
        channel_id: None,
        channel_slug: channel_slug.map(ToOwned::to_owned),
        channel_resolution_source: None,
        locale: "en".to_string(),
        correlation_id: "pages-public-landing".to_string(),
    });
    let builder = match cache {
        Some(cache) => {
            builder.data(PagesGraphqlRuntimeData::default().with_cache_read_runtime(cache))
        }
        None => builder,
    };
    match auth {
        Some(auth) => builder.data(auth).finish(),
        None => builder.finish(),
    }
}

async fn execute(schema: &PagesSchema, query: &str, variables: Value) -> TestResult<Value> {
    let response = schema
        .execute(GraphqlRequest::new(query).variables(Variables::from_json(variables)))
        .await;
    if !response.errors.is_empty() {
        return Err(std::io::Error::other(format!("GraphQL errors: {:?}", response.errors)).into());
    }
    Ok(response.data.into_json()?)
}

async fn page_by_slug(schema: &PagesSchema) -> TestResult<Value> {
    let data = execute(
        schema,
        PAGE_BY_SLUG_QUERY,
        json!({ "slug": "home", "locale": "en" }),
    )
    .await?;
    let page = data["pageBySlug"].clone();
    if page.is_null() {
        return Err(std::io::Error::other("pageBySlug did not resolve the published page").into());
    }
    Ok(page)
}

/// Enables the Pages module for the tenant, as the tenant module registry does in a host.
async fn enable_pages_module(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    let now = Utc::now().to_rfc3339();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenant_modules \
            (id, tenant_id, module_slug, enabled, settings, created_at, updated_at) \
            VALUES (?, ?, ?, ?, ?, ?, ?)",
        [
            Uuid::new_v4().to_string().into(),
            tenant_id.to_string().into(),
            "pages".into(),
            true.into(),
            "{}".into(),
            now.clone().into(),
            now.into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn create_unpublished_page(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<Uuid> {
    let page = PageService::new(db.clone(), event_bus(db))
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Upcoming".to_string(),
                    slug: Some("upcoming".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;
    Ok(page.id)
}

async fn persist_new_draft_body(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    fixture: &PublishedFixture,
) -> TestResult<()> {
    let body = page_body::Entity::find_by_id(fixture.body_id)
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(fixture.page_id))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("published page body is missing"))?;
    let mut active: page_body::ActiveModel = body.into();
    active.content = Set(serde_json::to_string(&json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Draft-only mutation",
                "description": "This persisted draft must not replace the selected artifact",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "draft-heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Draft-only mutation"
                }]
            }
        }]
    }))?);
    active.updated_at = Set((Utc::now() + ChronoDuration::seconds(1)).into());
    active.update(db).await?;
    Ok(())
}

async fn create_reviewed_published_page(
    db: &DatabaseConnection,
    event_bus: TransactionalEventBus,
    tenant_id: Uuid,
) -> TestResult<PublishedFixture> {
    let project = json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Published immutable artifact",
                "description": "Selected immutable artifact owner regression",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "published-heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Published immutable artifact"
                }]
            }
        }]
    });
    let service = PageService::new(db.clone(), event_bus);
    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Published immutable artifact".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: Some("Published immutable artifact".to_string()),
                    meta_description: Some(
                        "Selected immutable artifact owner regression".to_string(),
                    ),
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: project.clone(),
                }),
                channel_slugs: Some(vec!["web".to_string()]),
                publish: false,
            },
        )
        .await?;
    let body_response = draft
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("reviewed draft is missing its body"))?;
    let body_revision = body_response.updated_at.clone();
    let body = page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(draft.id))
        .filter(page_body::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| {
            std::io::Error::other("reviewed draft body is missing from owner storage")
        })?;
    let body_id = body.id;
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "public-landing-graphql",
        json!({ "surface": "storefront", "channel": "web" }),
    )?;
    let publish = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: draft.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: body_revision,
                }],
                idempotency_key: "public-landing-graphql-v1".to_string(),
                runtime: ReviewedPagePublishRuntimeInput {
                    format: reviewed.format,
                    scenario_id: reviewed.scenario_id,
                    context: reviewed.context,
                    review_hash: reviewed.review_hash,
                },
            },
        )
        .await?;
    assert_eq!(publish.page_id, draft.id);
    assert!(!publish.replayed);

    let artifact = page_static_landing_artifact::Entity::find()
        .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_static_landing_artifact::Column::PageId.eq(draft.id))
        .filter(page_static_landing_artifact::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("reviewed publish did not retain an artifact"))?;
    assert!(artifact.materialization_hash.is_some());
    assert!(artifact.materialization_identity.is_some());
    assert!(artifact.runtime_snapshots.is_some());

    let binding = page_published_landing_artifact::Entity::find_by_id(body_id)
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(draft.id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("reviewed publish did not retain a binding"))?;
    assert_eq!(binding.artifact_id, artifact.id);

    Ok(PublishedFixture {
        page_id: draft.id,
        body_id,
        artifact_hash: artifact.artifact_hash,
        document_html: artifact.document_html,
    })
}

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_public_landing_graphql_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)".to_string(),
    ))
    .await?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await?;
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenant_modules (\
            id TEXT PRIMARY KEY NOT NULL, \
            tenant_id TEXT NOT NULL, \
            module_slug TEXT NOT NULL, \
            enabled INTEGER NOT NULL, \
            settings TEXT NOT NULL, \
            created_at TEXT NOT NULL, \
            updated_at TEXT NOT NULL\
        )"
        .to_string(),
    ))
    .await?;

    let manager = SchemaManager::new(&db);
    SysEventsMigration.up(&manager).await?;
    for migration in ChannelModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in PagesModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}
