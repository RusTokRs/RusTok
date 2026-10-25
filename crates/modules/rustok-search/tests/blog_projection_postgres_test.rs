//! Blog Search projection lifecycle against the real owner source.
//!
//! The documents are produced by `rustok-blog`'s `SearchProjectionSource` and stored
//! by Search's neutral `SearchProjectionDriver`, so this harness exercises the
//! production path end to end instead of a Search-side copy of the Blog SQL. Two
//! consequences are asserted explicitly:
//!
//! * the indexed corpus is the owner's public corpus — a draft is not stored at all,
//!   and unpublishing or archiving removes the stored document;
//! * the author name comes from the Profiles owner port, never from a `users` join, so
//!   a profile whose visibility forbids public presentation yields no author name even
//!   while the account row still exists.

use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_blog::BlogSearchProjectionSource;
use rustok_core::{MigrationSource, events::EventHandler};
use rustok_events::{ContractEventEnvelope, DomainEvent, EventEnvelope};
use rustok_profiles::api::{
    ProfileSummary, ProfileSummaryAudience, ProfileSummaryReadResult, ProfileSummaryReader,
    ProfileSummaryVisibility,
};
use rustok_search::{SearchIngestionHandler, SearchModule, SearchProjectionSource};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use serde_json::Value as JsonValue;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const SEARCH_TEST_DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";

struct PostgresSearchTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresSearchTestDb {
    async fn setup(prefix: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Blog search projection lifecycle test"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_search_{}_{}",
            sanitize_identifier(prefix),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect_in_schema(&database_url, &schema_name).await?;
        assert_schema_resolves(&db, &schema_name).await?;

        let setup_result = async {
            let manager = SchemaManager::new(&db);
            for migration in SearchModule.migrations() {
                migration.up(&manager).await?;
            }
            create_blog_projection_source_tables(&db).await
        }
        .await;

        if let Err(error) = setup_result {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await;
            return Err(error.into());
        }

        Ok(Some(Self {
            control,
            db,
            schema_name,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

#[derive(Debug)]
struct SearchDocumentSnapshot {
    status: String,
    is_public: bool,
    title: String,
    slug: Option<String>,
    locale: String,
    body: String,
    payload: JsonValue,
}

/// Minimal Profiles owner port used as the *only* author-name source.
///
/// The Blog source asks for `ProfileSummaryAudience::Anonymous`, so anything that is
/// not publicly visible must not reach the document. The real service applies the full
/// visibility ladder; this stub keeps the public/non-public distinction, which is what
/// the assertion depends on.
#[derive(Clone, Default)]
struct StubProfiles {
    entries: Vec<StubProfile>,
}

#[derive(Clone)]
struct StubProfile {
    tenant_id: Uuid,
    author_id: Uuid,
    handle: String,
    display_name: String,
    visibility: ProfileSummaryVisibility,
}

impl StubProfiles {
    fn new() -> Self {
        Self::default()
    }

    fn with_author(
        mut self,
        tenant_id: Uuid,
        author_id: Uuid,
        handle: &str,
        display_name: &str,
        visibility: ProfileSummaryVisibility,
    ) -> Self {
        self.entries.push(StubProfile {
            tenant_id,
            author_id,
            handle: handle.to_string(),
            display_name: display_name.to_string(),
            visibility,
        });
        self
    }

    fn rename(mut self, author_id: Uuid, display_name: &str) -> Self {
        for entry in &mut self.entries {
            if entry.author_id == author_id {
                entry.display_name = display_name.to_string();
            }
        }
        self
    }
}

#[async_trait]
impl ProfileSummaryReader for StubProfiles {
    async fn find_profile_summaries(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
        _requested_locale: Option<&str>,
        _tenant_default_locale: Option<&str>,
        audience: ProfileSummaryAudience,
    ) -> ProfileSummaryReadResult<HashMap<Uuid, ProfileSummary>> {
        let mut summaries = HashMap::new();
        for entry in &self.entries {
            if entry.tenant_id != tenant_id || !user_ids.contains(&entry.author_id) {
                continue;
            }
            let visible = match entry.visibility {
                ProfileSummaryVisibility::Public => true,
                ProfileSummaryVisibility::Authenticated
                | ProfileSummaryVisibility::FollowersOnly
                | ProfileSummaryVisibility::Private => audience.is_authenticated(),
            };
            if !visible {
                continue;
            }
            summaries.insert(
                entry.author_id,
                ProfileSummary {
                    user_id: entry.author_id,
                    handle: entry.handle.clone(),
                    display_name: entry.display_name.clone(),
                    tags: Vec::new(),
                    avatar_media_id: None,
                    preferred_locale: None,
                    visibility: entry.visibility,
                },
            );
        }
        Ok(summaries)
    }
}

fn blog_handler(db: &DatabaseConnection, profiles: Option<StubProfiles>) -> SearchIngestionHandler {
    let profiles_reader = profiles.map(|stub| Arc::new(stub) as Arc<dyn ProfileSummaryReader>);
    let source: Arc<dyn SearchProjectionSource> = Arc::new(BlogSearchProjectionSource::new(
        db.clone(),
        profiles_reader,
    ));
    SearchIngestionHandler::with_all_projection_sources(db.clone(), Some(source), None, None, None)
}

#[tokio::test]
async fn blog_events_upsert_publish_archive_and_delete_search_document() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("blog_lifecycle").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    insert_blog_post(
        &test_db.db,
        tenant_id,
        post_id,
        author_id,
        "draft",
        "release-notes",
        "Release notes",
    )
    .await?;

    let profiles = StubProfiles::new().with_author(
        tenant_id,
        author_id,
        "search-author",
        "Public Author",
        ProfileSummaryVisibility::Public,
    );
    let handler = blog_handler(&test_db.db, Some(profiles));
    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::BlogPostCreated {
                post_id,
                author_id: Some(author_id),
                locale: "en".to_string(),
            },
        )?)
        .await?;

    // A draft is not part of the public corpus the owner hands to Search, so the
    // neutral driver stores nothing for it. This is the deliberate difference from the
    // retired `blog_projector`, which indexed drafts as non-public rows.
    assert!(
        load_blog_document(&test_db.db, tenant_id, post_id)
            .await?
            .is_none(),
        "a draft Blog post must not enter the public Search corpus"
    );

    test_db
        .db
        .execute_unprepared(&format!(
            r#"
            UPDATE blog_posts
            SET status = 'published', published_at = NOW(), updated_at = NOW(), version = 2
            WHERE id = '{post_id}' AND tenant_id = '{tenant_id}';
            UPDATE blog_post_translations
            SET title = 'Release notes v2',
                body = '{{"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"Published body"}}]}}]}}',
                updated_at = NOW()
            WHERE post_id = '{post_id}' AND locale = 'en';
            "#
        ))
        .await?;
    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::BlogPostPublished {
                post_id,
                author_id: Some(author_id),
            },
        )?)
        .await?;

    let published = load_blog_document(&test_db.db, tenant_id, post_id)
        .await?
        .expect("published Blog post should be projected");
    assert_eq!(published.status, "published");
    assert!(published.is_public);
    assert_eq!(published.title, "Release notes v2");
    assert_eq!(published.slug.as_deref(), Some("release-notes"));
    assert_eq!(published.locale, "en");
    // The article body, not the excerpt, is what full-text search can see.
    assert_eq!(published.body, "Published body");
    assert_eq!(published.payload["slug"], "release-notes");
    assert_eq!(published.payload["category_name"], "Search Category");
    assert_eq!(published.payload["category_slug"], "search-category");
    // The `users` fixture still says "Search Author"; a document that carried that name
    // would prove Search had gone back to joining the account table itself.
    assert_eq!(published.payload["author_name"], "Public Author");
    // Relation order is `blog_post_tags.created_at`, so the fixture stamps the two
    // attachments a second apart and the projection has a defined order.
    assert_eq!(
        published.payload["tags"],
        serde_json::json!(["rust", "cms"])
    );
    assert_eq!(published.payload["channel_slugs"], serde_json::json!(["web"]));
    assert_eq!(published.payload["version"], 2);
    assert!(!published.payload["published_at"].is_null());

    test_db
        .db
        .execute_unprepared(&format!(
            r#"
            UPDATE blog_posts
            SET status = 'archived', archived_at = NOW(), updated_at = NOW(), version = 3
            WHERE id = '{post_id}' AND tenant_id = '{tenant_id}'
            "#
        ))
        .await?;
    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::BlogPostArchived {
                post_id,
                reason: Some("superseded".to_string()),
            },
        )?)
        .await?;

    assert!(
        load_blog_document(&test_db.db, tenant_id, post_id)
            .await?
            .is_none(),
        "an archived Blog post must leave the public Search corpus"
    );

    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::BlogPostDeleted { post_id },
        )?)
        .await?;
    assert!(
        load_blog_document(&test_db.db, tenant_id, post_id)
            .await?
            .is_none()
    );

    test_db.cleanup().await
}

#[tokio::test]
async fn blog_author_profile_change_rebuilds_the_public_corpus() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("blog_author_refresh").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    insert_blog_post(
        &test_db.db,
        tenant_id,
        post_id,
        author_id,
        "published",
        "author-refresh",
        "Author refresh",
    )
    .await?;

    let profiles = StubProfiles::new().with_author(
        tenant_id,
        author_id,
        "search-author",
        "Public Author",
        ProfileSummaryVisibility::Public,
    );
    let handler = blog_handler(&test_db.db, Some(profiles.clone()));
    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: None,
            },
        )?)
        .await?;
    let before = load_blog_document(&test_db.db, tenant_id, post_id)
        .await?
        .expect("published Blog post should be projected");
    assert_eq!(before.payload["author_name"], "Public Author");

    // A profile rename is a related-owner change: the post row does not move, but every
    // document that embeds the public summary is stale. The handler rebuilds the Blog
    // scope for `ProfileUpdated`, which is also a Forum inbox scope, so this also pins
    // that the Blog refresh happens before the Forum inbox can consume the event.
    let renamed = blog_handler(
        &test_db.db,
        Some(profiles.rename(author_id, "Renamed Author")),
    );
    renamed
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::ProfileUpdated {
                user_id: author_id,
                handle: "renamed-author".to_string(),
                locale: Some("en".to_string()),
            },
        )?)
        .await?;

    let after = load_blog_document(&test_db.db, tenant_id, post_id)
        .await?
        .expect("published Blog post should stay projected");
    assert_eq!(after.payload["author_name"], "Renamed Author");

    test_db.cleanup().await
}

#[tokio::test]
async fn blog_projection_uses_the_profiles_port_not_the_account_row() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("blog_author_port").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let public_post_id = Uuid::new_v4();
    let private_post_id = Uuid::new_v4();
    let foreign_post_id = Uuid::new_v4();
    let public_author_id = Uuid::new_v4();
    let private_author_id = Uuid::new_v4();
    let foreign_author_id = Uuid::new_v4();

    for (post_id, author_id, slug) in [
        (public_post_id, public_author_id, "public-author"),
        (private_post_id, private_author_id, "private-author"),
        (foreign_post_id, foreign_author_id, "foreign-author"),
    ] {
        insert_blog_post(
            &test_db.db,
            tenant_id,
            post_id,
            author_id,
            "published",
            slug,
            "Author visibility",
        )
        .await?;
    }

    // The account row exists for every author and always says "Search Author": the
    // retired projector joined it, the owner port does not.
    let profiles = StubProfiles::new()
        .with_author(
            tenant_id,
            public_author_id,
            "public-author",
            "Public Author",
            ProfileSummaryVisibility::Public,
        )
        .with_author(
            tenant_id,
            private_author_id,
            "private-author",
            "Private Author",
            ProfileSummaryVisibility::Private,
        )
        .with_author(
            other_tenant_id,
            foreign_author_id,
            "foreign-author",
            "Foreign Author",
            ProfileSummaryVisibility::Public,
        );
    let handler = blog_handler(&test_db.db, Some(profiles));
    handler
        .handle(&envelope(
            tenant_id,
            None,
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: None,
            },
        )?)
        .await?;

    let public = load_blog_document(&test_db.db, tenant_id, public_post_id)
        .await?
        .expect("public-author post should be projected");
    assert_eq!(public.payload["author_name"], "Public Author");
    assert_eq!(
        public.payload["author_id"],
        public_author_id.to_string()
    );

    let private = load_blog_document(&test_db.db, tenant_id, private_post_id)
        .await?
        .expect("private-author post should still be projected");
    assert!(
        private.payload["author_name"].is_null(),
        "a profile that does not publish to anonymous readers must not surface in Search"
    );

    let foreign = load_blog_document(&test_db.db, tenant_id, foreign_post_id)
        .await?
        .expect("foreign-author post should still be projected");
    assert!(
        foreign.payload["author_name"].is_null(),
        "the Profiles port is asked in the document tenant and must not answer across tenants"
    );
    assert_ne!(foreign.payload["author_name"], "Search Author");

    test_db.cleanup().await
}

#[tokio::test]
async fn blog_projection_degrades_without_the_profiles_port() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("blog_author_absent").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    insert_blog_post(
        &test_db.db,
        tenant_id,
        post_id,
        author_id,
        "published",
        "no-profiles-port",
        "No Profiles port",
    )
    .await?;

    let handler = blog_handler(&test_db.db, None);
    handler
        .handle(&envelope(
            tenant_id,
            None,
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: None,
            },
        )?)
        .await?;

    let projected = load_blog_document(&test_db.db, tenant_id, post_id)
        .await?
        .expect("Blog post should be projected without a Profiles port");
    assert!(projected.payload["author_name"].is_null());
    assert_ne!(projected.payload["author_name"], "Search Author");

    test_db.cleanup().await
}

#[tokio::test]
async fn blog_reindex_projects_attached_taxonomy_tags_not_metadata_snapshot() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("blog_taxonomy_tags").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    insert_blog_post(
        &test_db.db,
        tenant_id,
        post_id,
        author_id,
        "published",
        "taxonomy-tags",
        "Taxonomy tags",
    )
    .await?;

    test_db
        .db
        .execute_unprepared(&format!(
            r#"
            UPDATE blog_posts
            SET metadata = '{{"tags":["stale-metadata-only"]}}'::jsonb
            WHERE id = '{post_id}' AND tenant_id = '{tenant_id}';

            UPDATE taxonomy_term_translations
            SET name = 'backend', slug = 'backend'
            WHERE tenant_id = '{tenant_id}'
              AND locale = 'en'
              AND term_id IN (
                  SELECT relation.tag_id
                  FROM blog_post_tags relation
                  WHERE relation.post_id = '{post_id}'
              )
              AND name = 'rust';
            "#
        ))
        .await?;

    let handler = blog_handler(&test_db.db, None);
    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: Some(post_id),
            },
        )?)
        .await?;

    let document = load_blog_document(&test_db.db, tenant_id, post_id)
        .await?
        .expect("Blog post should be projected from canonical tag attachments");
    assert_eq!(
        document.payload["tags"],
        serde_json::json!(["backend", "cms"])
    );
    assert_ne!(
        document.payload["tags"],
        serde_json::json!(["stale-metadata-only"])
    );

    test_db.cleanup().await
}

#[tokio::test]
async fn full_blog_reindex_replaces_only_current_tenant_blog_documents() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("blog_reindex").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let published_post_id = Uuid::new_v4();
    let draft_post_id = Uuid::new_v4();
    insert_blog_post(
        &test_db.db,
        tenant_id,
        published_post_id,
        author_id,
        "published",
        "first-post",
        "First post",
    )
    .await?;
    insert_blog_post(
        &test_db.db,
        tenant_id,
        draft_post_id,
        author_id,
        "draft",
        "second-post",
        "Second post",
    )
    .await?;

    let stale_id = Uuid::new_v4();
    let other_tenant_document_id = Uuid::new_v4();
    insert_search_document(
        &test_db.db,
        tenant_id,
        stale_id,
        "blog",
        "blog_post",
        "stale",
    )
    .await?;
    insert_search_document(
        &test_db.db,
        other_tenant_id,
        other_tenant_document_id,
        "blog",
        "blog_post",
        "other-tenant",
    )
    .await?;

    let handler = blog_handler(&test_db.db, None);
    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: None,
            },
        )?)
        .await?;

    // Only the published post is part of the public corpus; the draft leaves no row.
    assert_eq!(count_blog_documents(&test_db.db, tenant_id).await?, 1);
    assert!(
        load_blog_document(&test_db.db, tenant_id, stale_id)
            .await?
            .is_none()
    );
    assert!(
        load_blog_document(&test_db.db, tenant_id, published_post_id)
            .await?
            .is_some()
    );
    assert!(
        load_blog_document(&test_db.db, tenant_id, draft_post_id)
            .await?
            .is_none()
    );
    assert!(
        load_blog_document(&test_db.db, other_tenant_id, other_tenant_document_id)
            .await?
            .is_some()
    );

    test_db.cleanup().await
}

#[tokio::test]
async fn blog_module_disable_cleans_scope_and_enable_rebuilds_it() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("blog_module_toggle").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let other_tenant_document_id = Uuid::new_v4();
    insert_blog_post(
        &test_db.db,
        tenant_id,
        post_id,
        author_id,
        "published",
        "module-toggle",
        "Module toggle",
    )
    .await?;
    insert_search_document(
        &test_db.db,
        other_tenant_id,
        other_tenant_document_id,
        "blog",
        "blog_post",
        "other-tenant",
    )
    .await?;

    let handler = blog_handler(&test_db.db, None);
    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: None,
            },
        )?)
        .await?;
    assert_eq!(count_blog_documents(&test_db.db, tenant_id).await?, 1);

    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::TenantModuleToggled {
                tenant_id,
                module_slug: "blog".to_string(),
                enabled: false,
            },
        )?)
        .await?;
    assert_eq!(count_blog_documents(&test_db.db, tenant_id).await?, 0);
    assert!(
        load_blog_document(&test_db.db, other_tenant_id, other_tenant_document_id)
            .await?
            .is_some()
    );

    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::TenantModuleToggled {
                tenant_id,
                module_slug: "blog".to_string(),
                enabled: true,
            },
        )?)
        .await?;
    assert_eq!(count_blog_documents(&test_db.db, tenant_id).await?, 1);
    assert!(
        load_blog_document(&test_db.db, tenant_id, post_id)
            .await?
            .is_some()
    );

    test_db.cleanup().await
}

#[tokio::test]
async fn targeted_reindex_removes_stale_document_when_source_post_is_missing() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("blog_missing_target").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let missing_post_id = Uuid::new_v4();
    insert_search_document(
        &test_db.db,
        tenant_id,
        missing_post_id,
        "blog",
        "blog_post",
        "missing-source",
    )
    .await?;

    let handler = blog_handler(&test_db.db, None);
    handler
        .handle(&envelope(
            tenant_id,
            None,
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: Some(missing_post_id),
            },
        )?)
        .await?;

    assert!(
        load_blog_document(&test_db.db, tenant_id, missing_post_id)
            .await?
            .is_none()
    );

    test_db.cleanup().await
}

fn envelope(
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    event: DomainEvent,
) -> TestResult<EventEnvelope> {
    Ok(ContractEventEnvelope::new(tenant_id, actor_id, event)?.into_root_envelope()?)
}

async fn create_blog_projection_source_tables(
    db: &DatabaseConnection,
) -> Result<(), sea_orm::DbErr> {
    // The shape mirrors the Blog and Taxonomy owners' entities, because the projection
    // now runs the owners' own queries instead of Search-side SQL. A column added to an
    // owner entity must be added here as well, which is exactly the coupling a neutral
    // driver cannot hide: Search stores documents, the owners read their own tables.
    db.execute_unprepared(
        r#"
        CREATE TABLE users (
            id UUID PRIMARY KEY,
            tenant_id UUID NOT NULL,
            name TEXT NOT NULL
        );

        CREATE TABLE blog_posts (
            id UUID PRIMARY KEY,
            tenant_id UUID NOT NULL,
            author_id UUID NOT NULL,
            category_id UUID NULL,
            status TEXT NOT NULL,
            slug TEXT NOT NULL,
            metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
            featured_image_url TEXT NULL,
            published_at TIMESTAMPTZ NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            archived_at TIMESTAMPTZ NULL,
            scheduled_at TIMESTAMPTZ NULL,
            is_pinned BOOLEAN NOT NULL DEFAULT FALSE,
            pinned_at TIMESTAMPTZ NULL,
            comment_count INTEGER NOT NULL DEFAULT 0,
            version INTEGER NOT NULL DEFAULT 1
        );

        CREATE TABLE blog_post_translations (
            id UUID PRIMARY KEY,
            post_id UUID NOT NULL,
            locale TEXT NOT NULL,
            title TEXT NOT NULL,
            excerpt TEXT NULL,
            seo_title TEXT NULL,
            seo_description TEXT NULL,
            body TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );

        CREATE TABLE blog_post_channel_visibility (
            id UUID PRIMARY KEY,
            tenant_id UUID NOT NULL,
            post_id UUID NOT NULL,
            channel_slug TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );

        CREATE TABLE taxonomy_terms (
            id UUID PRIMARY KEY,
            tenant_id UUID NOT NULL,
            kind TEXT NOT NULL,
            scope_type TEXT NOT NULL,
            scope_value TEXT NOT NULL,
            canonical_key TEXT NOT NULL,
            revision BIGINT NOT NULL DEFAULT 1,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );

        CREATE TABLE taxonomy_term_translations (
            id UUID PRIMARY KEY,
            term_id UUID NOT NULL,
            tenant_id UUID NOT NULL,
            locale TEXT NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL,
            description TEXT NULL,
            revision BIGINT NOT NULL DEFAULT 1,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );

        CREATE TABLE blog_post_tags (
            post_id UUID NOT NULL,
            tag_id UUID NOT NULL,
            tenant_id UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            PRIMARY KEY (post_id, tag_id)
        );
        "#,
    )
    .await?;
    Ok(())
}

async fn insert_blog_post(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    post_id: Uuid,
    author_id: Uuid,
    status: &str,
    slug: &str,
    title: &str,
) -> Result<(), sea_orm::DbErr> {
    let translation_id = Uuid::new_v4();
    let channel_visibility_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let category_translation_id = Uuid::new_v4();
    let rust_tag_id = Uuid::new_v4();
    let rust_translation_id = Uuid::new_v4();
    let cms_tag_id = Uuid::new_v4();
    let cms_translation_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        r#"
        INSERT INTO users (id, tenant_id, name)
        VALUES ('{author_id}', '{tenant_id}', 'Search Author')
        ON CONFLICT (id) DO NOTHING;

        INSERT INTO blog_posts (
            id, tenant_id, author_id, category_id, status, slug, metadata, published_at,
            created_at, updated_at, comment_count, version
        ) VALUES (
            '{post_id}', '{tenant_id}', '{author_id}', '{category_id}', '{status}', '{slug}',
            '{{"tags":["metadata-only"]}}'::jsonb,
            CASE WHEN '{status}' = 'published' THEN NOW() ELSE NULL END,
            NOW(), NOW(), 4, 1
        );

        INSERT INTO blog_post_translations (
            id, post_id, locale, title, excerpt, seo_title, seo_description,
            body, created_at, updated_at
        ) VALUES (
            '{translation_id}', '{post_id}', 'en', '{title}', 'Excerpt',
            'SEO title', 'SEO description',
            '{{"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"Draft body"}}]}}]}}',
            NOW(), NOW()
        );

        INSERT INTO blog_post_channel_visibility (id, tenant_id, post_id, channel_slug, created_at)
        VALUES ('{channel_visibility_id}', '{tenant_id}', '{post_id}', 'web', NOW());

        INSERT INTO taxonomy_terms (id, tenant_id, kind, scope_type, scope_value, canonical_key)
        VALUES
            ('{category_id}', '{tenant_id}', 'category', 'module', 'blog', 'blog-category'),
            ('{rust_tag_id}', '{tenant_id}', 'tag', 'global', '', 'rust'),
            ('{cms_tag_id}', '{tenant_id}', 'tag', 'global', '', 'cms');

        INSERT INTO taxonomy_term_translations (id, term_id, tenant_id, locale, name, slug)
        VALUES
            ('{category_translation_id}', '{category_id}', '{tenant_id}', 'en', 'Search Category', 'search-category');

        INSERT INTO taxonomy_term_translations (id, term_id, tenant_id, locale, name, slug)
        VALUES
            ('{rust_translation_id}', '{rust_tag_id}', '{tenant_id}', 'en', 'rust', 'rust'),
            ('{cms_translation_id}', '{cms_tag_id}', '{tenant_id}', 'en', 'cms', 'cms');

        INSERT INTO blog_post_tags (post_id, tag_id, tenant_id, created_at)
        VALUES
            ('{post_id}', '{rust_tag_id}', '{tenant_id}', NOW()),
            ('{post_id}', '{cms_tag_id}', '{tenant_id}', NOW() + INTERVAL '1 second');
        "#
    ))
    .await?;
    Ok(())
}

async fn insert_search_document(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    document_id: Uuid,
    source_module: &str,
    entity_type: &str,
    slug: &str,
) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(&format!(
        r#"
        INSERT INTO search_documents (
            document_key, tenant_id, document_id, source_module, entity_type,
            locale, status, is_public, title, slug, body, keywords_text,
            facets, payload, created_at, updated_at, indexed_at
        ) VALUES (
            '{source_module}:{document_id}:en', '{tenant_id}', '{document_id}',
            '{source_module}', '{entity_type}', 'en', 'published', TRUE,
            'Stale document', '{slug}', '', '', '{{}}'::jsonb,
            '{{"slug":"{slug}"}}'::jsonb, NOW(), NOW(), NOW()
        )
        "#
    ))
    .await?;
    Ok(())
}

async fn load_blog_document(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    post_id: Uuid,
) -> Result<Option<SearchDocumentSnapshot>, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT status, is_public, title, slug, locale, body, payload
            FROM search_documents
            WHERE tenant_id = $1
              AND document_id = $2
              AND source_module = 'blog'
              AND entity_type = 'blog_post'
            "#,
            vec![tenant_id.into(), post_id.into()],
        ))
        .await?;

    row.map(|row| {
        Ok(SearchDocumentSnapshot {
            status: row.try_get("", "status")?,
            is_public: row.try_get("", "is_public")?,
            title: row.try_get("", "title")?,
            slug: row.try_get("", "slug")?,
            locale: row.try_get("", "locale")?,
            body: row.try_get("", "body")?,
            payload: row.try_get("", "payload")?,
        })
    })
    .transpose()
}

async fn count_blog_documents(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT COUNT(*)::bigint AS count
            FROM search_documents
            WHERE tenant_id = $1
              AND source_module = 'blog'
              AND entity_type = 'blog_post'
            "#,
            vec![tenant_id.into()],
        ))
        .await?
        .expect("count query should return one row");
    row.try_get("", "count")
}

fn postgres_database_url() -> Option<String> {
    std::env::var(SEARCH_TEST_DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(4)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

/// Connects with the test schema already on the server startup `search_path`.
///
/// The projection driver replaces a scope inside a transaction while the owner source
/// answers its queries on its own pooled connection, so a single `SET search_path` on
/// one checkout is not enough: every connection of the pool has to resolve the test
/// schema. `options` is a libpq-style startup parameter, and setting it here is the only
/// pool-wide way to do that without a per-statement prefix.
async fn connect_in_schema(
    database_url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    connect(&format!(
        "{database_url}{separator}options=-c%20search_path%3D{schema_name}%2Cpublic"
    ))
    .await
}

/// Fails the harness when the startup `search_path` did not take effect.
///
/// Without this check a pool that silently ignored the option fails much later, as a
/// confusing "relation blog_posts does not exist" inside the owner source.
async fn assert_schema_resolves(
    db: &DatabaseConnection,
    schema_name: &str,
) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_setting('search_path') AS search_path".to_string(),
        ))
        .await?
        .ok_or_else(|| io::Error::other("search_path query returned no row"))?;
    let search_path: String = row.try_get("", "search_path")?;
    if !search_path
        .split(',')
        .any(|entry| entry.trim().trim_matches('"') == schema_name)
    {
        return Err(io::Error::other(format!(
            "test connection does not resolve the `{schema_name}` schema (search_path = `{search_path}`)"
        ))
        .into());
    }
    Ok(())
}

fn sanitize_identifier(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let normalized = normalized.trim_matches('_');
    if normalized.is_empty() {
        "test".to_string()
    } else {
        normalized.to_string()
    }
}
