use std::{collections::BTreeMap, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::SysEventsMigration;
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
    CreateMenuInput, MenuItemInput, MenuItemTranslationInput, MenuLocation, MenuService,
    MenuTranslationInput, NavigationMenuTranslationTargetProvider, NavigationModule,
};

async fn setup() -> (DatabaseConnection, Arc<MenuService>) {
    let database = setup_test_db().await;
    let manager = SchemaManager::new(&database);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in NavigationModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("Navigation migration should apply");
    }
    let service = Arc::new(MenuService::new(database.clone()));
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
    let mut migrations = NavigationModule.migrations();
    let translation_target_migration = migrations
        .pop()
        .expect("Navigation translation target migration should be registered");
    for migration in migrations {
        migration
            .up(&manager)
            .await
            .expect("base Navigation migration should apply");
    }
    translation_target_migration
        .up(&manager)
        .await
        .expect("Navigation translation target migration should apply");
    translation_target_migration
        .down(&manager)
        .await
        .expect("Navigation translation target migration should roll back");
    translation_target_migration
        .up(&manager)
        .await
        .expect("Navigation translation target migration should reapply");

    let service = MenuService::new(database);
    service
        .create(
            Uuid::new_v4(),
            SecurityContext::system(),
            "en",
            source_menu_input(),
        )
        .await
        .expect("reapplied translation target schema should accept revisioned menus");
}

fn source_menu_input() -> CreateMenuInput {
    CreateMenuInput {
        translations: vec![MenuTranslationInput {
            locale: "en".to_string(),
            name: "Main navigation".to_string(),
        }],
        location: MenuLocation::Header,
        items: vec![
            MenuItemInput {
                translations: vec![MenuItemTranslationInput {
                    locale: "en".to_string(),
                    title: "Home".to_string(),
                }],
                url: Some("/".to_string()),
                icon: None,
                position: 0,
                children: None,
            },
            MenuItemInput {
                translations: vec![MenuItemTranslationInput {
                    locale: "en".to_string(),
                    title: "Catalog".to_string(),
                }],
                url: Some("/catalog".to_string()),
                icon: Some("grid".to_string()),
                position: 1,
                children: Some(vec![MenuItemInput {
                    translations: vec![MenuItemTranslationInput {
                        locale: "en".to_string(),
                        title: "Sale".to_string(),
                    }],
                    url: Some("/catalog/sale".to_string()),
                    icon: None,
                    position: 0,
                    children: None,
                }]),
            },
        ],
    }
}

fn read_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "navigation-translation-read",
    )
    .with_deadline(Duration::from_secs(5))
}

fn field_patches(
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
) -> (Vec<TranslationFieldPatch>, BTreeMap<String, String>) {
    let mut expected_values = BTreeMap::new();
    let fields = snapshot
        .fields
        .iter()
        .map(|field| {
            let key = field.descriptor.key.as_str().to_string();
            let value = if key == "menu_name" {
                "Navigation principale".to_string()
            } else {
                format!("FR {}", field.source_value)
            };
            expected_values.insert(key, value.clone());
            TranslationFieldPatch {
                key: field.descriptor.key.clone(),
                value,
                expected_source_hash: field.source_hash.clone(),
            }
        })
        .collect();
    (fields, expected_values)
}

#[tokio::test]
async fn translation_target_rejects_partial_new_menu_locale_aggregate() {
    let (_database, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let menu = service
        .create(
            tenant_id,
            SecurityContext::system(),
            "en",
            source_menu_input(),
        )
        .await
        .expect("source Navigation menu should be created");
    let provider = NavigationMenuTranslationTargetProvider::new(service);
    let context = read_context(tenant_id);
    let snapshot = provider
        .read_resource(
            context.clone(),
            ReadTranslationResourceRequest {
                identity: rustok_translation_targets::TranslationResourceIdentity {
                    owner_slug: rustok_translation_targets::OwnerSlug::new("navigation")
                        .expect("static owner slug should be valid"),
                    resource_kind: rustok_translation_targets::ResourceKind::new("menu")
                        .expect("static resource kind should be valid"),
                    resource_id: rustok_translation_targets::ResourceId::new(menu.id.to_string())
                        .expect("menu UUID should be a valid resource id"),
                    subresource_id: None,
                },
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect("source Navigation menu should be readable");
    let menu_name = snapshot
        .fields
        .first()
        .expect("menu aggregate must expose its menu name field");
    let partial_patch = TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: None,
        fields: vec![TranslationFieldPatch {
            key: menu_name.descriptor.key.clone(),
            value: "Navigation principale".to_string(),
            expected_source_hash: menu_name.source_hash.clone(),
        }],
        proposal_id: "navigation-partial-proposal".to_string(),
        approval_receipt_id: "navigation-partial-approval".to_string(),
    };
    let error = provider
        .apply_patch(
            PortContext::new(
                tenant_id.to_string(),
                PortActor::system(),
                "en",
                "navigation-translation-partial-apply",
            )
            .with_idempotency_key("navigation-translation-partial-apply-1")
            .with_deadline(Duration::from_secs(5)),
            partial_patch,
        )
        .await
        .expect_err("a new menu locale must include every aggregate field");
    assert_eq!(error.kind, PortErrorKind::Validation);
    assert_eq!(error.code, "translation.target_required_field_missing");

    let after = provider
        .read_resource(
            context,
            ReadTranslationResourceRequest {
                identity: snapshot.summary.identity,
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect("rejected partial aggregate must leave no target locale behind");
    assert!(after.target_revision.is_none());
    assert!(
        after
            .fields
            .iter()
            .all(|field| field.exact_target_value.is_none())
    );
}

#[tokio::test]
async fn translation_target_applies_an_atomic_menu_locale_aggregate_and_replays() {
    let (_database, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let menu = service
        .create(
            tenant_id,
            SecurityContext::system(),
            "en",
            source_menu_input(),
        )
        .await
        .expect("source Navigation menu should be created");
    let provider = NavigationMenuTranslationTargetProvider::new(service);
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
        .expect("source Navigation write should record a translation change");
    assert_eq!(source_changes.changes.len(), 1);
    let source_cursor = source_changes
        .next_cursor
        .expect("source change should return a resume cursor");

    let page = provider
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
        .expect("exact source Navigation menu should be listed");
    assert_eq!(page.resources.len(), 1);
    assert_eq!(
        page.resources[0].identity.resource_id.as_str(),
        menu.id.to_string()
    );
    assert_eq!(page.resources[0].exact_locales.len(), 1);

    let snapshot = provider
        .read_resource(
            read_context.clone(),
            ReadTranslationResourceRequest {
                identity: page.resources[0].identity.clone(),
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect("exact source Navigation menu should be readable");
    assert!(snapshot.target_revision.is_none());
    assert_eq!(snapshot.rendered_fallback_locale, None);
    assert_eq!(snapshot.fields.len(), 4);
    assert_eq!(snapshot.fields[0].descriptor.key.as_str(), "menu_name");

    let (fields, expected_values) = field_patches(&snapshot);
    let patch = TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: None,
        fields,
        proposal_id: "navigation-proposal-1".to_string(),
        approval_receipt_id: "navigation-approval-1".to_string(),
    };
    assert!(
        provider
            .validate_patch(read_context.clone(), patch.clone())
            .await
            .expect("Navigation patch validation should complete")
            .accepted
    );

    let apply_context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "navigation-translation-apply",
    )
    .with_idempotency_key("navigation-translation-apply-1")
    .with_deadline(Duration::from_secs(5));
    let receipt = provider
        .apply_patch(apply_context.clone(), patch.clone())
        .await
        .expect("Navigation patch should apply atomically");
    assert_eq!(receipt.resource_revision.as_str(), "2");
    assert_eq!(receipt.target_revision.as_str(), "1");
    assert_eq!(receipt.applied_field_keys.len(), snapshot.fields.len());
    let replay = provider
        .apply_patch(apply_context.clone(), patch.clone())
        .await
        .expect("same Navigation idempotency request should replay");
    assert_eq!(replay, receipt);

    let mut conflicting_patch = patch.clone();
    conflicting_patch.proposal_id = "navigation-proposal-2".to_string();
    let conflict = provider
        .apply_patch(apply_context, conflicting_patch)
        .await
        .expect_err("same Navigation idempotency key must reject a changed request");
    assert_eq!(conflict.kind, PortErrorKind::Conflict);
    assert_eq!(conflict.code, "outbox.operation_receipt_conflict");

    assert!(
        !provider
            .validate_patch(read_context.clone(), patch)
            .await
            .expect("stale Navigation patch validation should complete")
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
        .expect("applied Navigation patch should record a change");
    assert_eq!(applied_changes.changes.len(), 1);
    assert_eq!(
        applied_changes.changes[0].identity,
        page.resources[0].identity
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
        .expect("Navigation progress should use exact aggregate target values");
    assert_eq!(progress.resources, 1);
    assert_eq!(progress.required_units, 4);
    assert_eq!(progress.exact_required_units, 4);
    assert_eq!(progress.optional_units, 0);
    assert_eq!(progress.exact_optional_units, 0);
    assert_eq!(progress.complete_resources, 1);
    assert!(progress.owner_change_cursor.is_some());

    let unauthorized = provider
        .read_resource(
            PortContext::new(
                tenant_id.to_string(),
                PortActor::user(Uuid::new_v4().to_string()),
                "en",
                "navigation-translation-forbidden",
            )
            .with_deadline(Duration::from_secs(5)),
            ReadTranslationResourceRequest {
                identity: page.resources[0].identity.clone(),
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect_err("unprivileged user should not read the Navigation target");
    assert_eq!(unauthorized.kind, PortErrorKind::Forbidden);
}
