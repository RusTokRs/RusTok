#![cfg(all(feature = "mod-seo", feature = "mod-translation"))]

use std::{error::Error, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_content::ContentModule;
use rustok_core::ModuleRegistry;
use rustok_migrations::Migrator;
use rustok_seo::SeoModule;
use rustok_server::{
    auth::AuthConfig,
    common::settings::RustokSettings,
    services::{
        module_event_dispatcher::build_shared_runtime_extensions_with_host_providers,
        server_runtime_context::ServerRuntimeContext,
    },
};
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OpaqueCursor, OwnerSlug, ReadTranslationResourceRequest,
    ResourceKind, TranslationFieldPatch, TranslationFieldSnapshot, TranslationPatchRequest,
    TranslationResourceLifecycle, TranslationResourceSnapshot, TranslationTargetCapability,
    TranslationTargetChangesRequest, TranslationTargetProgressRequest, TranslationTargetProvider,
    translation_target_registry,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use sea_orm_migration::MigratorTrait;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "seo";
const RESOURCE_KIND: &str = "seo_copy";
const TARGET_KIND: &str = "page";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn seo_registered_translation_provider_multi_replica_evidence_postgres() -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);
    let database_name = unique_postgres_database_name("rustok_seo_translation_evidence");
    let database_url = postgres_database_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url).await?;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    create_postgres_database(&admin, &database_name).await?;

    let result = run_contract(&database_url).await;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    admin.close().await?;
    result
}

async fn run_contract(database_url: &str) -> TestResult<()> {
    let seed_connection = connect_postgres(database_url).await?;
    Migrator::up(&seed_connection, None).await?;

    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    seed_tenant(&seed_connection, tenant_id, "seo-main").await?;
    seed_tenant(&seed_connection, other_tenant_id, "seo-isolated").await?;

    let target_id = Uuid::new_v4();
    let other_target_id = Uuid::new_v4();
    let meta_id = seed_seo_source(&seed_connection, tenant_id, target_id, "SEO source title").await?;
    seed_seo_source(
        &seed_connection,
        other_tenant_id,
        other_target_id,
        "Isolated SEO source",
    )
    .await?;

    let provider = registered_provider(seed_connection.clone());
    let descriptor = provider.descriptor();
    assert_eq!(descriptor.owner_slug.as_str(), OWNER_SLUG);
    assert_eq!(descriptor.resource_kind.as_str(), RESOURCE_KIND);
    for capability in [
        TranslationTargetCapability::ListResources,
        TranslationTargetCapability::ReadExactResource,
        TranslationTargetCapability::AggregateProgress,
        TranslationTargetCapability::ValidatePatch,
        TranslationTargetCapability::ApplyPatch,
        TranslationTargetCapability::ChangeCursor,
    ] {
        assert!(descriptor.capabilities.contains(&capability));
    }

    let list_request = ListTranslationResourcesRequest {
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
        cursor: None,
        limit: 10,
    };
    let listed = provider
        .list_resources(read_context(tenant_id, "list"), list_request.clone())
        .await?;
    assert_eq!(listed.resources.len(), 1);
    assert!(listed.next_cursor.is_none());
    let identity = listed.resources[0].identity.clone();
    assert_eq!(identity.owner_slug.as_str(), OWNER_SLUG);
    assert_eq!(identity.resource_kind.as_str(), RESOURCE_KIND);
    assert_eq!(identity.resource_id.as_str(), target_id.to_string());
    assert_eq!(
        identity
            .subresource_id
            .as_ref()
            .expect("SEO target kind")
            .as_str(),
        TARGET_KIND
    );

    let isolated = provider
        .list_resources(read_context(other_tenant_id, "isolated-list"), list_request)
        .await?;
    assert_eq!(isolated.resources.len(), 1);
    assert_eq!(
        isolated.resources[0].identity.resource_id.as_str(),
        other_target_id.to_string()
    );
    assert_ne!(isolated.resources[0].identity, identity);

    let read_request = ReadTranslationResourceRequest {
        identity: identity.clone(),
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    };
    let initial = provider
        .read_resource(read_context(tenant_id, "initial"), read_request.clone())
        .await?;
    assert_eq!(
        initial.summary.lifecycle,
        TranslationResourceLifecycle::Active
    );
    assert_eq!(initial.fields.len(), 5);
    assert_eq!(field(&initial, "title").source_value, "SEO source title");
    assert!(field(&initial, "title").exact_target_value.is_none());
    for field in &initial.fields {
        assert!(!field.descriptor.required);
        assert!(!field.descriptor.ai_export_allowed);
    }

    let initial_changes = provider
        .read_changes(
            read_context(tenant_id, "initial-changes"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    assert_eq!(initial_changes.changes.len(), 1);
    assert!(initial_changes.next_cursor.is_none());
    assert_eq!(initial_changes.changes[0].identity, identity);
    assert_eq!(
        initial_changes.changes[0].resource_revision,
        initial.summary.resource_revision
    );
    assert_eq!(
        initial_changes.changes[0].lifecycle,
        TranslationResourceLifecycle::Active
    );

    let initial_progress = provider
        .read_progress(
            read_context(tenant_id, "initial-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(initial_progress.resources, 1);
    assert_eq!(initial_progress.complete_resources, 0);
    assert_eq!(initial_progress.required_units, 0);
    assert_eq!(initial_progress.exact_required_units, 0);
    assert_eq!(initial_progress.optional_units, 5);
    assert_eq!(initial_progress.exact_optional_units, 0);
    let seed_cursor = initial_progress
        .owner_change_cursor
        .clone()
        .expect("seed owner cursor");

    let actor_id = Uuid::new_v4();
    let first_connection = connect_postgres(database_url).await?;
    let second_connection = connect_postgres(database_url).await?;
    let first_provider = registered_provider(first_connection.clone());
    let second_provider = registered_provider(second_connection.clone());

    let first_patch = patch(&initial, "Premier", "first");
    let first_receipt = first_provider
        .apply_patch(
            apply_context(tenant_id, actor_id, "first-apply", "seo-first-apply"),
            first_patch.clone(),
        )
        .await?;
    let replay = second_provider
        .apply_patch(
            apply_context(tenant_id, actor_id, "first-replay", "seo-first-apply"),
            first_patch,
        )
        .await?;
    assert_eq!(replay.provider_receipt_id, first_receipt.provider_receipt_id);
    assert_eq!(replay.resource_revision, first_receipt.resource_revision);
    assert_eq!(replay.target_revision, first_receipt.target_revision);
    assert_eq!(
        event_reindex_counts(&seed_connection, tenant_id, target_id).await?,
        (1, 1)
    );

    let after_first = provider
        .read_resource(read_context(tenant_id, "after-first"), read_request.clone())
        .await?;
    assert_eq!(
        after_first.summary.resource_revision,
        first_receipt.resource_revision
    );
    assert_eq!(
        field(&after_first, "title").exact_target_value.as_deref(),
        Some("Premier title")
    );

    let translated_progress = provider
        .read_progress(
            read_context(tenant_id, "translated-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(translated_progress.resources, 1);
    assert_eq!(translated_progress.complete_resources, 1);
    assert_eq!(translated_progress.optional_units, 5);
    assert_eq!(translated_progress.exact_optional_units, 5);

    let replica_one = first_provider
        .read_resource(read_context(tenant_id, "replica-one"), read_request.clone())
        .await?;
    let replica_two = second_provider
        .read_resource(read_context(tenant_id, "replica-two"), read_request.clone())
        .await?;
    assert_eq!(
        replica_one.summary.resource_revision,
        replica_two.summary.resource_revision
    );

    let race_one = first_provider.apply_patch(
        apply_context(tenant_id, actor_id, "race-one", "seo-race-one"),
        patch(&replica_one, "Course A", "race-one"),
    );
    let race_two = second_provider.apply_patch(
        apply_context(tenant_id, actor_id, "race-two", "seo-race-two"),
        patch(&replica_two, "Course B", "race-two"),
    );
    let (race_one, race_two) = tokio::join!(race_one, race_two);
    let winner = match (race_one, race_two) {
        (Ok(receipt), Err(error)) | (Err(error), Ok(receipt)) => {
            assert!(matches!(
                error.kind,
                PortErrorKind::Conflict | PortErrorKind::Unavailable
            ));
            receipt
        }
        other => panic!("expected exactly one concurrent SEO CAS winner: {other:?}"),
    };
    assert_eq!(
        event_reindex_counts(&seed_connection, tenant_id, target_id).await?,
        (2, 2)
    );

    let post_race = provider
        .read_resource(read_context(tenant_id, "post-race"), read_request.clone())
        .await?;
    assert_eq!(post_race.summary.resource_revision, winner.resource_revision);
    let stale_patch = patch(&post_race, "Stale", "stale-after-owner-write");

    let first_window = provider
        .read_changes(
            read_context(tenant_id, "window-one"),
            TranslationTargetChangesRequest {
                after: Some(seed_cursor.clone()),
                limit: 1,
            },
        )
        .await?;
    assert_eq!(first_window.changes.len(), 1);
    assert_eq!(
        first_window.changes[0].resource_revision,
        first_receipt.resource_revision
    );
    let page_cursor = first_window.next_cursor.expect("paged SEO cursor");

    let before_noncopy = provider
        .read_progress(
            read_context(tenant_id, "before-noncopy"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    update_non_copy_fields(&seed_connection, meta_id).await?;
    let after_noncopy = provider
        .read_progress(
            read_context(tenant_id, "after-noncopy"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(
        before_noncopy.owner_change_cursor,
        after_noncopy.owner_change_cursor
    );
    assert_eq!(
        provider
            .read_resource(
                read_context(tenant_id, "after-noncopy-read"),
                read_request.clone(),
            )
            .await?
            .summary
            .resource_revision,
        post_race.summary.resource_revision
    );

    update_source_title(&seed_connection, meta_id, "SEO source title revised").await?;
    let source_changed = provider
        .read_resource(read_context(tenant_id, "source-changed"), read_request.clone())
        .await?;
    assert_eq!(
        field(&source_changed, "title").source_value,
        "SEO source title revised"
    );
    assert_ne!(
        source_changed.summary.resource_revision,
        post_race.summary.resource_revision
    );
    let stale_error = provider
        .apply_patch(
            apply_context(
                tenant_id,
                actor_id,
                "stale-owner-revision",
                "seo-stale-owner-revision",
            ),
            stale_patch,
        )
        .await
        .expect_err("stale SEO owner revision must fail closed");
    assert_eq!(stale_error.kind, PortErrorKind::Conflict);
    assert_eq!(
        event_reindex_counts(&seed_connection, tenant_id, target_id).await?,
        (2, 2)
    );

    let second_window = provider
        .read_changes(
            read_context(tenant_id, "window-two"),
            TranslationTargetChangesRequest {
                after: Some(page_cursor),
                limit: 1,
            },
        )
        .await?;
    assert_eq!(second_window.changes.len(), 1);
    assert_eq!(
        second_window.changes[0].resource_revision,
        winner.resource_revision
    );
    let late_cursor = second_window
        .next_cursor
        .expect("late write must keep pagination open");

    let terminal_window = provider
        .read_changes(
            read_context(tenant_id, "window-terminal"),
            TranslationTargetChangesRequest {
                after: Some(late_cursor),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(terminal_window.changes.len(), 1);
    assert_eq!(
        terminal_window.changes[0].resource_revision,
        source_changed.summary.resource_revision
    );
    assert_eq!(
        terminal_window.changes[0].lifecycle,
        TranslationResourceLifecycle::Active
    );
    assert!(terminal_window.next_cursor.is_none());

    let pre_delete_progress = provider
        .read_progress(
            read_context(tenant_id, "pre-delete-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(pre_delete_progress.resources, 1);
    assert_eq!(pre_delete_progress.complete_resources, 1);
    let pre_delete_cursor = pre_delete_progress
        .owner_change_cursor
        .clone()
        .expect("pre-delete cursor");

    delete_meta(&seed_connection, tenant_id, target_id).await?;
    let deleted = provider
        .read_changes(
            read_context(tenant_id, "deleted-change"),
            TranslationTargetChangesRequest {
                after: Some(pre_delete_cursor.clone()),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(deleted.changes.len(), 1);
    assert!(deleted.next_cursor.is_none());
    assert_eq!(deleted.changes[0].identity, identity);
    assert_eq!(
        deleted.changes[0].lifecycle,
        TranslationResourceLifecycle::Deleted
    );
    assert!(
        deleted.changes[0]
            .resource_revision
            .as_str()
            .starts_with("deleted:page:")
    );

    let deleted_error = provider
        .read_resource(read_context(tenant_id, "deleted-read"), read_request)
        .await
        .expect_err("deleted SEO copy must not remain readable");
    assert_eq!(deleted_error.kind, PortErrorKind::NotFound);

    let deleted_progress = provider
        .read_progress(
            read_context(tenant_id, "deleted-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(deleted_progress.resources, 0);
    assert_eq!(deleted_progress.complete_resources, 0);
    assert_eq!(deleted_progress.optional_units, 0);
    assert_eq!(deleted_progress.exact_optional_units, 0);
    assert!(
        cursor_seq(
            deleted_progress
                .owner_change_cursor
                .as_ref()
                .expect("deleted highwater")
        ) > cursor_seq(&pre_delete_cursor)
    );

    let other_after_delete = provider
        .list_resources(
            read_context(other_tenant_id, "other-after-delete"),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
                cursor: None,
                limit: 10,
            },
        )
        .await?;
    assert_eq!(other_after_delete.resources.len(), 1);
    assert_eq!(
        other_after_delete.resources[0].identity.resource_id.as_str(),
        other_target_id.to_string()
    );

    drop(provider);
    drop(first_provider);
    drop(second_provider);
    first_connection.close().await?;
    second_connection.close().await?;
    seed_connection.close().await?;
    Ok(())
}

fn registered_provider(db: DatabaseConnection) -> Arc<dyn TranslationTargetProvider> {
    let registry = ModuleRegistry::new()
        .register(ContentModule)
        .register(SeoModule);
    let settings = RustokSettings::default();
    let runtime = ServerRuntimeContext::new(db, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime,
        AuthConfig::new("test-secret-key-for-seo-translation-32bytes!!".to_string()),
    )
    .expect("host composition must succeed");
    let targets = translation_target_registry(&extensions).expect("Translation registry");
    targets
        .get(
            &OwnerSlug::new(OWNER_SLUG).expect("owner slug"),
            &ResourceKind::new(RESOURCE_KIND).expect("resource kind"),
        )
        .expect("seo/seo_copy provider")
}

fn patch(
    snapshot: &TranslationResourceSnapshot,
    variant: &str,
    suffix: &str,
) -> TranslationPatchRequest {
    TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: snapshot.target_revision.clone(),
        fields: snapshot
            .fields
            .iter()
            .map(|field| TranslationFieldPatch {
                key: field.descriptor.key.clone(),
                value: match field.descriptor.key.as_str() {
                    "title" => format!("{variant} title"),
                    "description" => format!("{variant} description"),
                    "keywords" => format!("{variant},seo"),
                    "og_title" => format!("{variant} Open Graph title"),
                    "og_description" => format!("{variant} Open Graph description"),
                    other => panic!("unexpected SEO translation field: {other}"),
                },
                expected_source_hash: field.source_hash.clone(),
            })
            .collect(),
        proposal_id: format!("seo-proposal-{suffix}"),
        approval_receipt_id: format!("seo-approval-{suffix}"),
    }
}

fn field<'a>(snapshot: &'a TranslationResourceSnapshot, key: &str) -> &'a TranslationFieldSnapshot {
    snapshot
        .fields
        .iter()
        .find(|field| field.descriptor.key.as_str() == key)
        .expect("translation field")
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("seo-postgres-{suffix}"),
    )
    .with_deadline(Duration::from_secs(30))
}

fn apply_context(tenant_id: Uuid, actor_id: Uuid, suffix: &str, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("seo-postgres-{suffix}"),
    )
    .with_claim("seo:read")
    .with_claim("seo:update")
    .with_role("manager")
    .with_idempotency_key(key)
    .with_deadline(Duration::from_secs(30))
}

fn cursor_seq(cursor: &OpaqueCursor) -> u64 {
    cursor
        .as_str()
        .parse()
        .expect("SEO owner change cursor must be a positive sequence")
}

async fn seed_tenant(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    slug_prefix: &str,
) -> TestResult<()> {
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)",
            vec![
                tenant_id.into(),
                "SEO translation evidence".into(),
                format!("{slug_prefix}-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}

async fn seed_seo_source(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    target_id: Uuid,
    title: &str,
) -> TestResult<Uuid> {
    let meta_id = Uuid::new_v4();
    let txn = database.begin().await?;
    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO meta (
            id, tenant_id, target_type, target_id,
            no_index, no_follow, canonical_url, structured_data
         ) VALUES ($1, $2, $3, $4, FALSE, FALSE, NULL, NULL)",
        vec![
            meta_id.into(),
            tenant_id.into(),
            TARGET_KIND.into(),
            target_id.into(),
        ],
    ))
    .await?;
    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO meta_translations (
            id, meta_id, locale, title, description, keywords,
            og_title, og_description, og_image
         ) VALUES ($1, $2, 'en', $3, $4, $5, $6, $7, NULL)",
        vec![
            Uuid::new_v4().into(),
            meta_id.into(),
            title.to_string().into(),
            "SEO source description".into(),
            "source,seo".into(),
            "SEO source Open Graph title".into(),
            "SEO source Open Graph description".into(),
        ],
    ))
    .await?;
    txn.commit().await?;
    Ok(meta_id)
}

async fn update_non_copy_fields(database: &DatabaseConnection, meta_id: Uuid) -> TestResult<()> {
    let txn = database.begin().await?;
    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE meta SET no_index = NOT no_index WHERE id = $1",
        vec![meta_id.into()],
    ))
    .await?;
    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE meta_translations
         SET og_image = 'https://example.invalid/seo-evidence.png'
         WHERE meta_id = $1 AND locale = 'en'",
        vec![meta_id.into()],
    ))
    .await?;
    txn.commit().await?;
    Ok(())
}

async fn update_source_title(
    database: &DatabaseConnection,
    meta_id: Uuid,
    title: &str,
) -> TestResult<()> {
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE meta_translations SET title = $1 WHERE meta_id = $2 AND locale = 'en'",
            vec![title.to_string().into(), meta_id.into()],
        ))
        .await?;
    Ok(())
}

async fn delete_meta(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    target_id: Uuid,
) -> TestResult<()> {
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM meta WHERE tenant_id = $1 AND target_type = $2 AND target_id = $3",
            vec![tenant_id.into(), TARGET_KIND.into(), target_id.into()],
        ))
        .await?;
    Ok(())
}

async fn event_reindex_counts(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    target_id: Uuid,
) -> TestResult<(i64, i64)> {
    let row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT
                (SELECT COUNT(*)
                   FROM seo_event_delivery
                  WHERE tenant_id = $1
                    AND event_type = 'seo.meta.upserted'
                    AND source_kind = $2
                    AND source_id = $3
                    AND status = 'sent') AS event_count,
                (SELECT COUNT(*)
                   FROM seo_index_delivery
                  WHERE tenant_id = $1
                    AND target_type = $2
                    AND target_id = $3
                    AND target_scope = 'entity'
                    AND status = 'sent') AS reindex_count",
            vec![tenant_id.into(), TARGET_KIND.into(), target_id.into()],
        ))
        .await?
        .expect("SEO evidence count query must return one row");
    Ok((
        row.try_get("", "event_count")?,
        row.try_get("", "reindex_count")?,
    ))
}
