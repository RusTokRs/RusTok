use std::{collections::BTreeMap, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_test_utils::db::setup_test_db;
use rustok_translation_targets::{
    ListTranslationResourcesRequest, ReadTranslationResourceRequest, TranslationFieldPatch,
    TranslationPatchRequest, TranslationTargetChangesRequest, TranslationTargetProgressRequest,
    TranslationTargetProvider,
};
use sea_orm::DatabaseConnection;
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use crate::{
    CreatePageInput, PageService, PageTranslationInput, PagesMetadataTranslationTargetProvider,
    PagesModule, PatchPageMetadataInput,
};

const TRANSLATION_TARGET_MIGRATION: &str = "m20260806_000014_add_translation_target_support";

async fn setup() -> (DatabaseConnection, Arc<PageService>) {
    let database = setup_test_db().await;
    let manager = SchemaManager::new(&database);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in PagesModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("Pages migration should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone())));
    let service = Arc::new(PageService::new(database.clone(), event_bus));
    (database, service)
}

#[tokio::test]
async fn translation_target_schema_supports_up_down_up() {
    let database = setup_test_db().await;
    let manager = SchemaManager::new(&database);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    let mut migrations = PagesModule.migrations();
    let translation_target_index = migrations
        .iter()
        .position(|migration| migration.name() == TRANSLATION_TARGET_MIGRATION)
        .expect("Pages translation target migration should be registered");
    let translation_target_migration = migrations.remove(translation_target_index);
    for migration in migrations.into_iter().take(translation_target_index) {
        migration
            .up(&manager)
            .await
            .expect("base Pages migration should apply");
    }
    translation_target_migration
        .up(&manager)
        .await
        .expect("Pages translation target migration should apply");
    translation_target_migration
        .down(&manager)
        .await
        .expect("Pages translation target migration should roll back");
    translation_target_migration
        .up(&manager)
        .await
        .expect("Pages translation target migration should reapply");

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone())));
    let service = PageService::new(database, event_bus);
    service
        .create(
            Uuid::new_v4(),
            SecurityContext::system(),
            source_page_input(),
        )
        .await
        .expect("reapplied translation target schema should accept revisioned Pages");
}

fn source_page_input() -> CreatePageInput {
    CreatePageInput {
        translations: vec![PageTranslationInput {
            locale: "en".to_string(),
            title: "About us".to_string(),
            slug: Some("about-us".to_string()),
            meta_title: Some("About RusTok".to_string()),
            meta_description: Some("Learn about RusTok".to_string()),
        }],
        template: Some("default".to_string()),
        body: None,
        channel_slugs: None,
        publish: false,
    }
}

fn read_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "pages-translation-read",
    )
    .with_deadline(Duration::from_secs(5))
}

fn field_patches(
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
) -> (Vec<TranslationFieldPatch>, BTreeMap<String, String>) {
    let expected_values = BTreeMap::from([
        ("title".to_string(), "A propos".to_string()),
        ("slug".to_string(), "a-propos".to_string()),
        ("meta_title".to_string(), "A propos de RusTok".to_string()),
        (
            "meta_description".to_string(),
            "Decouvrez RusTok".to_string(),
        ),
    ]);
    let fields = snapshot
        .fields
        .iter()
        .map(|field| TranslationFieldPatch {
            key: field.descriptor.key.clone(),
            value: expected_values
                .get(field.descriptor.key.as_str())
                .expect("every Pages target field should have a test translation")
                .clone(),
            expected_source_hash: field.source_hash.clone(),
        })
        .collect();
    (fields, expected_values)
}

#[tokio::test]
async fn metadata_update_advances_exact_source_locale_and_owner_cursor() {
    let (_database, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let page = service
        .create(tenant_id, SecurityContext::system(), source_page_input())
        .await
        .expect("source Page should be created");

    let updated = service
        .patch_metadata(
            tenant_id,
            SecurityContext::system(),
            page.id,
            PatchPageMetadataInput {
                expected_version: page.version,
                translations: Some(vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Our story".to_string(),
                    slug: Some("our-story".to_string()),
                    meta_title: Some("Our RusTok story".to_string()),
                    meta_description: Some("Learn our story".to_string()),
                }]),
                template: None,
                channel_slugs: None,
            },
        )
        .await
        .expect("Pages metadata update should advance revisioned owner state");
    assert_eq!(updated.version, 2);

    let provider = PagesMetadataTranslationTargetProvider::new(service);
    let snapshot = provider
        .read_resource(
            read_context(tenant_id),
            ReadTranslationResourceRequest {
                identity: page_identity(page.id),
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect("updated source Page should be readable through the owner target");
    assert_eq!(snapshot.summary.resource_revision.as_str(), "2");
    assert_eq!(snapshot.source_revision.as_str(), "2");
}

#[tokio::test]
async fn translation_target_applies_replays_and_tracks_exact_page_metadata() {
    let (_database, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let page = service
        .create(tenant_id, SecurityContext::system(), source_page_input())
        .await
        .expect("source Page should be created");
    let provider = PagesMetadataTranslationTargetProvider::new(service);
    let read_context = read_context(tenant_id);

    let source_changes = provider
        .read_changes(
            read_context.clone(),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await
        .expect("source Pages write should record a translation change");
    assert_eq!(source_changes.changes.len(), 1);
    let source_cursor = source_changes
        .next_cursor
        .expect("source change should return a resume cursor");

    let page_list = provider
        .list_resources(
            read_context.clone(),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
                cursor: None,
                limit: 10,
            },
        )
        .await
        .expect("exact source Page should be listed");
    assert_eq!(page_list.resources.len(), 1);
    assert_eq!(
        page_list.resources[0].identity.resource_id.as_str(),
        page.id.to_string()
    );

    let snapshot = provider
        .read_resource(
            read_context.clone(),
            ReadTranslationResourceRequest {
                identity: page_list.resources[0].identity.clone(),
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect("exact source Page should be readable");
    assert!(snapshot.target_revision.is_none());
    assert_eq!(snapshot.rendered_fallback_locale, None);
    assert_eq!(snapshot.fields.len(), 4);

    let (fields, expected_values) = field_patches(&snapshot);
    let patch = TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: None,
        fields,
        proposal_id: "pages-proposal-1".to_string(),
        approval_receipt_id: "pages-approval-1".to_string(),
    };
    assert!(
        provider
            .validate_patch(read_context.clone(), patch.clone())
            .await
            .expect("Pages patch validation should complete")
            .accepted
    );

    let apply_context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "pages-translation-apply",
    )
    .with_idempotency_key("pages-translation-apply-1")
    .with_deadline(Duration::from_secs(5));
    let receipt = provider
        .apply_patch(apply_context.clone(), patch.clone())
        .await
        .expect("Pages patch should apply");
    assert_eq!(receipt.resource_revision.as_str(), "2");
    assert_eq!(receipt.target_revision.as_str(), "1");
    assert_eq!(receipt.applied_field_keys.len(), snapshot.fields.len());
    let replay = provider
        .apply_patch(apply_context.clone(), patch.clone())
        .await
        .expect("same Pages idempotency request should replay");
    assert_eq!(replay, receipt);

    let mut conflicting_patch = patch.clone();
    conflicting_patch.proposal_id = "pages-proposal-2".to_string();
    let conflict = provider
        .apply_patch(apply_context, conflicting_patch)
        .await
        .expect_err("same Pages idempotency key must reject a changed request");
    assert_eq!(conflict.kind, PortErrorKind::Conflict);
    assert_eq!(conflict.code, "outbox.operation_receipt_conflict");

    assert!(
        !provider
            .validate_patch(read_context.clone(), patch)
            .await
            .expect("stale Pages patch validation should complete")
            .accepted
    );

    let updated = provider
        .read_resource(
            read_context.clone(),
            ReadTranslationResourceRequest {
                identity: snapshot.summary.identity,
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect("applied exact target should be readable");
    assert_eq!(
        updated
            .target_revision
            .as_ref()
            .map(|revision| revision.as_str()),
        Some("1")
    );
    for field in &updated.fields {
        assert_eq!(
            field.exact_target_value.as_deref(),
            expected_values
                .get(field.descriptor.key.as_str())
                .map(String::as_str)
        );
    }

    let applied_changes = provider
        .read_changes(
            read_context.clone(),
            TranslationTargetChangesRequest {
                after: Some(source_cursor),
                limit: 10,
            },
        )
        .await
        .expect("applied Pages patch should record a change");
    assert_eq!(applied_changes.changes.len(), 1);
    assert_eq!(
        applied_changes.changes[0].identity,
        page_list.resources[0].identity
    );
    assert_eq!(
        applied_changes.changes[0].resource_revision,
        receipt.resource_revision
    );

    let progress = provider
        .read_progress(
            read_context.clone(),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect("Pages progress should use exact target values");
    assert_eq!(progress.resources, 1);
    assert_eq!(progress.required_units, 2);
    assert_eq!(progress.exact_required_units, 2);
    assert_eq!(progress.optional_units, 2);
    assert_eq!(progress.exact_optional_units, 2);
    assert_eq!(progress.complete_resources, 1);
    assert!(progress.owner_change_cursor.is_some());

    let unauthorized = provider
        .read_resource(
            PortContext::new(
                tenant_id.to_string(),
                PortActor::user(Uuid::new_v4().to_string()),
                "en",
                "pages-translation-forbidden",
            )
            .with_deadline(Duration::from_secs(5)),
            ReadTranslationResourceRequest {
                identity: page_list.resources[0].identity.clone(),
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect_err("unprivileged user should not read the Pages target");
    assert_eq!(unauthorized.kind, PortErrorKind::Forbidden);
}

fn page_identity(page_id: Uuid) -> rustok_translation_targets::TranslationResourceIdentity {
    rustok_translation_targets::TranslationResourceIdentity {
        owner_slug: rustok_translation_targets::OwnerSlug::new("pages")
            .expect("static owner slug should be valid"),
        resource_kind: rustok_translation_targets::ResourceKind::new("page_metadata")
            .expect("static resource kind should be valid"),
        resource_id: rustok_translation_targets::ResourceId::new(page_id.to_string())
            .expect("Page UUID should be a valid resource id"),
        subresource_id: None,
    }
}
