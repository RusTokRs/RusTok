#![cfg(feature = "mod-product")]

use std::{error::Error, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::ModuleRegistry;
use rustok_migrations::Migrator;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_product::{
    ProductModule,
    services::{
        AttributeOptionTranslationInput, AttributeTranslationInput, AttributeValueType,
        CreateProductAttributeInput, CreateProductAttributeOptionInput, ProductCatalogSchemaService,
    },
};
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
    ListTranslationResourcesRequest, OwnerSlug, ReadTranslationResourceRequest, ResourceKind,
    TranslationDataClassification, TranslationFieldPatch, TranslationPatchRequest,
    TranslationResourceSnapshot, TranslationTargetChangesRequest, TranslationTargetProgressRequest,
    TranslationTargetProvider, translation_target_registry,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "product";
const RESOURCE_KIND: &str = "attribute";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn product_attribute_registered_translation_provider_multi_replica_evidence_postgres(
) -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);
    let database_name = unique_postgres_database_name("rustok_attribute_translation_evidence");
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
    seed_tenant(&seed_connection, tenant_id, "attribute-main").await?;
    seed_tenant(&seed_connection, other_tenant_id, "attribute-isolated").await?;

    let owner = owner_service(seed_connection.clone());
    let actor_id = Uuid::new_v4();
    let attribute = owner
        .create_attribute(tenant_id, actor_id, public_attribute())
        .await?;
    let first_option = owner
        .create_attribute_option(
            tenant_id,
            actor_id,
            source_option(attribute.id, "red", 0, "Red"),
        )
        .await?;
    let private_attribute = owner
        .create_attribute(other_tenant_id, actor_id, private_attribute())
        .await?;

    let provider = registered_provider(seed_connection.clone());
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
    assert_eq!(identity.resource_id.as_str(), attribute.id.to_string());
    assert!(identity.subresource_id.is_none());

    let other_listed = provider
        .list_resources(
            read_context(other_tenant_id, "other-list"),
            list_request.clone(),
        )
        .await?;
    assert_eq!(other_listed.resources.len(), 1);
    assert_eq!(
        other_listed.resources[0].identity.resource_id.as_str(),
        private_attribute.id.to_string()
    );

    let read_request = ReadTranslationResourceRequest {
        identity: identity.clone(),
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    };
    let initial = provider
        .read_resource(read_context(tenant_id, "initial"), read_request.clone())
        .await?;
    assert_eq!(initial.fields.len(), 5);
    assert_eq!(field(&initial, "label").source_value, "Color");
    assert_eq!(field(&initial, "help_text").source_value, "Choose a color");
    assert_eq!(field(&initial, "facet_label").source_value, "Color");
    assert_eq!(field(&initial, "seo_label").source_value, "Product color");
    assert_eq!(
        field(&initial, &format!("option:{}", first_option.id)).source_value,
        "Red"
    );
    assert!(initial.fields.iter().all(|field| {
        field.descriptor.classification == TranslationDataClassification::Public
            && field.descriptor.ai_export_allowed
            && field.exact_target_value.is_none()
    }));

    let private_identity = other_listed.resources[0].identity.clone();
    let private_snapshot = provider
        .read_resource(
            read_context(other_tenant_id, "private-read"),
            ReadTranslationResourceRequest {
                identity: private_identity,
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(private_snapshot.fields.len(), 1);
    assert_eq!(
        private_snapshot.fields[0].descriptor.classification,
        TranslationDataClassification::TenantPrivate
    );
    assert!(!private_snapshot.fields[0].descriptor.ai_export_allowed);

    let owner_changes = provider
        .read_changes(
            read_context(tenant_id, "owner-create"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    assert_eq!(owner_changes.changes.len(), 2);
    assert_eq!(owner_changes.changes[0].identity, identity);
    assert_eq!(owner_changes.changes[1].identity, identity);
    assert_eq!(
        owner_changes.changes[1].resource_revision,
        initial.summary.resource_revision
    );
    let owner_cursor = owner_changes.next_cursor.expect("owner cursor");

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
    assert_eq!(initial_progress.required_units, 2);
    assert_eq!(initial_progress.exact_required_units, 0);
    assert_eq!(initial_progress.optional_units, 3);
    assert_eq!(initial_progress.exact_optional_units, 0);

    let first_connection = connect_postgres(database_url).await?;
    let second_connection = connect_postgres(database_url).await?;
    let first_provider = registered_provider(first_connection.clone());
    let second_provider = registered_provider(second_connection.clone());

    let first_patch = full_patch(&initial, "premier");
    let first_receipt = first_provider
        .apply_patch(
            apply_context(tenant_id, "first-apply", "attribute-first-apply"),
            first_patch.clone(),
        )
        .await?;
    let replay = second_provider
        .apply_patch(
            apply_context(tenant_id, "first-replay", "attribute-first-apply"),
            first_patch,
        )
        .await?;
    assert_eq!(replay.provider_receipt_id, first_receipt.provider_receipt_id);
    assert_eq!(replay.resource_revision, first_receipt.resource_revision);
    assert_eq!(replay.target_revision, first_receipt.target_revision);

    let after_first = provider
        .read_resource(
            read_context(tenant_id, "after-first"),
            read_request.clone(),
        )
        .await?;
    let completed = provider
        .read_progress(
            read_context(tenant_id, "completed-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(completed.required_units, 2);
    assert_eq!(completed.exact_required_units, 2);
    assert_eq!(completed.optional_units, 3);
    assert_eq!(completed.exact_optional_units, 3);
    assert_eq!(completed.complete_resources, 1);

    let replica_one = first_provider
        .read_resource(
            read_context(tenant_id, "replica-one"),
            read_request.clone(),
        )
        .await?;
    let replica_two = second_provider
        .read_resource(
            read_context(tenant_id, "replica-two"),
            read_request.clone(),
        )
        .await?;
    assert_eq!(
        replica_one.summary.resource_revision,
        replica_two.summary.resource_revision
    );
    let first_race = first_provider.apply_patch(
        apply_context(tenant_id, "race-one", "attribute-race-one"),
        full_patch(&replica_one, "race-a"),
    );
    let second_race = second_provider.apply_patch(
        apply_context(tenant_id, "race-two", "attribute-race-two"),
        full_patch(&replica_two, "race-b"),
    );
    let (first_result, second_result) = tokio::join!(first_race, second_race);
    let race_winner = match (first_result, second_result) {
        (Ok(receipt), Err(error)) | (Err(error), Ok(receipt)) => {
            assert!(matches!(error.kind, PortErrorKind::Conflict | PortErrorKind::Unavailable));
            receipt
        }
        other => panic!("expected exactly one concurrent Product Attribute CAS winner: {other:?}"),
    };

    let after_race = provider
        .read_resource(
            read_context(tenant_id, "after-race"),
            read_request.clone(),
        )
        .await?;
    assert_eq!(after_race.summary.resource_revision, race_winner.resource_revision);

    let frozen_first = provider
        .read_changes(
            read_context(tenant_id, "frozen-first"),
            TranslationTargetChangesRequest {
                after: Some(owner_cursor),
                limit: 1,
            },
        )
        .await?;
    assert_eq!(frozen_first.changes.len(), 1);
    assert_eq!(
        frozen_first.changes[0].resource_revision,
        first_receipt.resource_revision
    );
    let frozen_cursor = frozen_first.next_cursor.expect("frozen cursor");

    let stale_patch = full_patch(&after_race, "stale");
    let second_option = owner
        .create_attribute_option(
            tenant_id,
            actor_id,
            source_option(attribute.id, "blue", 1, "Blue"),
        )
        .await?;

    let frozen_second = provider
        .read_changes(
            read_context(tenant_id, "frozen-second"),
            TranslationTargetChangesRequest {
                after: Some(frozen_cursor),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(frozen_second.changes.len(), 1);
    assert_eq!(
        frozen_second.changes[0].resource_revision,
        race_winner.resource_revision
    );
    let frozen_terminal = frozen_second.next_cursor.expect("frozen terminal");

    let next_window = provider
        .read_changes(
            read_context(tenant_id, "next-window"),
            TranslationTargetChangesRequest {
                after: Some(frozen_terminal),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(next_window.changes.len(), 1);
    let late_option_revision = next_window.changes[0].resource_revision.clone();

    let stale_error = provider
        .apply_patch(
            apply_context(tenant_id, "stale", "attribute-stale-after-option"),
            stale_patch,
        )
        .await
        .expect_err("late canonical option creation must invalidate an old Translation patch");
    assert_eq!(stale_error.kind, PortErrorKind::Conflict);

    let after_late_option = provider
        .read_resource(
            read_context(tenant_id, "after-late-option"),
            read_request.clone(),
        )
        .await?;
    assert_eq!(
        after_late_option.summary.resource_revision,
        late_option_revision
    );
    assert_eq!(after_late_option.fields.len(), 6);
    assert_eq!(
        field(
            &after_late_option,
            &format!("option:{}", second_option.id)
        )
        .source_value,
        "Blue"
    );
    assert!(
        field(
            &after_late_option,
            &format!("option:{}", second_option.id)
        )
        .exact_target_value
        .is_none()
    );

    let incomplete = provider
        .read_progress(
            read_context(tenant_id, "late-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(incomplete.required_units, 3);
    assert_eq!(incomplete.exact_required_units, 2);
    assert_eq!(incomplete.optional_units, 3);
    assert_eq!(incomplete.exact_optional_units, 3);
    assert_eq!(incomplete.complete_resources, 0);

    let final_receipt = provider
        .apply_patch(
            apply_context(tenant_id, "final", "attribute-final-apply"),
            full_patch(&after_late_option, "final"),
        )
        .await?;
    let final_snapshot = provider
        .read_resource(read_context(tenant_id, "final-read"), read_request)
        .await?;
    assert_eq!(
        final_snapshot.summary.resource_revision,
        final_receipt.resource_revision
    );
    let final_progress = provider
        .read_progress(
            read_context(tenant_id, "final-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(final_progress.required_units, 3);
    assert_eq!(final_progress.exact_required_units, 3);
    assert_eq!(final_progress.optional_units, 3);
    assert_eq!(final_progress.exact_optional_units, 3);
    assert_eq!(final_progress.complete_resources, 1);

    let other_after = provider
        .list_resources(
            read_context(other_tenant_id, "other-after"),
            list_request,
        )
        .await?;
    assert_eq!(other_after.resources.len(), 1);
    assert_eq!(
        other_after.resources[0].identity.resource_id.as_str(),
        private_attribute.id.to_string()
    );

    drop(after_first);
    drop(provider);
    drop(first_provider);
    drop(second_provider);
    first_connection.close().await?;
    second_connection.close().await?;
    seed_connection.close().await?;
    Ok(())
}

fn owner_service(db: DatabaseConnection) -> ProductCatalogSchemaService {
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    ProductCatalogSchemaService::new(db, event_bus)
}

fn registered_provider(db: DatabaseConnection) -> Arc<dyn TranslationTargetProvider> {
    let registry = ModuleRegistry::new().register(ProductModule);
    let settings = RustokSettings::default();
    let runtime = ServerRuntimeContext::new(db, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime,
        AuthConfig::new("test-secret-key-for-attribute-translation-32bytes!".to_string()),
    )
    .expect("host composition must succeed");
    let targets = translation_target_registry(&extensions).expect("Translation registry");
    targets
        .get(
            &OwnerSlug::new(OWNER_SLUG).expect("owner slug"),
            &ResourceKind::new(RESOURCE_KIND).expect("resource kind"),
        )
        .expect("product/attribute provider")
}

fn public_attribute() -> CreateProductAttributeInput {
    CreateProductAttributeInput {
        code: "color".into(),
        value_type: AttributeValueType::Select,
        scope: "product".into(),
        is_localized: false,
        is_filterable: true,
        is_searchable: true,
        is_sortable: false,
        is_comparable: true,
        show_on_storefront: true,
        show_in_admin_grid: true,
        search_weight: 10,
        filter_display: Some("swatch".into()),
        facet_mode: Some("multi".into()),
        position: 0,
        validation: json!({}),
        default_value: None,
        metadata: json!({"evidence": "attribute-translation"}),
        translations: vec![AttributeTranslationInput {
            locale: "en".into(),
            label: "Color".into(),
            help_text: Some("Choose a color".into()),
            facet_label: Some("Color".into()),
            seo_label: Some("Product color".into()),
        }],
    }
}

fn private_attribute() -> CreateProductAttributeInput {
    CreateProductAttributeInput {
        code: "internal_note".into(),
        value_type: AttributeValueType::Text,
        scope: "product".into(),
        is_localized: true,
        is_filterable: false,
        is_searchable: false,
        is_sortable: false,
        is_comparable: false,
        show_on_storefront: false,
        show_in_admin_grid: true,
        search_weight: 0,
        filter_display: None,
        facet_mode: None,
        position: 0,
        validation: json!({}),
        default_value: None,
        metadata: json!({"evidence": "tenant-private"}),
        translations: vec![AttributeTranslationInput {
            locale: "en".into(),
            label: "Internal note".into(),
            help_text: None,
            facet_label: None,
            seo_label: None,
        }],
    }
}

fn source_option(
    attribute_id: Uuid,
    code: &str,
    position: i32,
    label: &str,
) -> CreateProductAttributeOptionInput {
    CreateProductAttributeOptionInput {
        attribute_id,
        code: code.into(),
        position,
        metadata: json!({}),
        translations: vec![AttributeOptionTranslationInput {
            locale: "en".into(),
            label: label.into(),
        }],
    }
}

fn full_patch(snapshot: &TranslationResourceSnapshot, suffix: &str) -> TranslationPatchRequest {
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
                    "label" => format!("Couleur {suffix}"),
                    "help_text" => format!("Choisissez une couleur {suffix}"),
                    "facet_label" => format!("Couleur facette {suffix}"),
                    "seo_label" => format!("Couleur produit {suffix}"),
                    key if key.starts_with("option:") => {
                        format!("FR {suffix} {}", field.source_value)
                    }
                    key => panic!("unexpected Product Attribute field: {key}"),
                },
                expected_source_hash: field.source_hash.clone(),
            })
            .collect(),
        proposal_id: format!("attribute-proposal-{suffix}"),
        approval_receipt_id: format!("attribute-approval-{suffix}"),
    }
}

fn field<'a>(
    snapshot: &'a TranslationResourceSnapshot,
    key: &str,
) -> &'a rustok_translation_targets::TranslationFieldSnapshot {
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
        format!("attribute-postgres-{suffix}"),
    )
    .with_deadline(Duration::from_secs(30))
}

fn apply_context(tenant_id: Uuid, suffix: &str, key: &str) -> PortContext {
    read_context(tenant_id, suffix).with_idempotency_key(key)
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
                "Product Attribute translation evidence".into(),
                format!("{slug_prefix}-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}
