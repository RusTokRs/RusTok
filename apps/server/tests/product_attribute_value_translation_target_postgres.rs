#![cfg(feature = "mod-product")]

use std::{error::Error, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::ModuleRegistry;
use rustok_migrations::Migrator;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_product::{
    CatalogService, ProductModule,
    dto::{CreateProductInput, CreateVariantInput, ProductTranslationInput},
    services::{
        AttributeTranslationInput, AttributeValueType, BindCategoryAttributeInput,
        CatalogCategoryKind, CategoryAttributeBindingKind, CategoryTranslationInput,
        CreateCatalogCategoryInput, CreateProductAttributeInput, ProductAttributeValuePatch,
        ProductAttributeValuePatchValue, ProductCatalogSchemaService,
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
    TranslationResourceLifecycle, TranslationResourceSnapshot, TranslationTargetChangesRequest,
    TranslationTargetProgressRequest, TranslationTargetProvider, TranslationValueProfile,
    translation_target_registry,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "product";
const RESOURCE_KIND: &str = "attribute_value";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn product_attribute_value_registered_translation_provider_multi_replica_evidence_postgres(
) -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);
    let database_name = unique_postgres_database_name("rustok_product_attribute_value_translation");
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
    seed_tenant(&seed_connection, tenant_id, "attribute-value-main").await?;
    seed_tenant(&seed_connection, other_tenant_id, "attribute-value-isolated").await?;

    let schema_owner = schema_owner(seed_connection.clone());
    let catalog_owner = catalog_owner(seed_connection.clone());
    let actor_id = Uuid::new_v4();
    let category = schema_owner
        .create_category(tenant_id, actor_id, source_category())
        .await?;
    let attribute = schema_owner
        .create_attribute(tenant_id, actor_id, localized_attribute())
        .await?;
    schema_owner
        .bind_category_attribute(
            tenant_id,
            actor_id,
            BindCategoryAttributeInput {
                category_id: category.id,
                attribute_id: attribute.id,
                group_code: None,
                binding_kind: CategoryAttributeBindingKind::Addition,
                is_required: Some(false),
                is_disabled: false,
                position: Some(0),
                visibility_overrides: json!({}),
                validation_overrides: json!({}),
                metadata: json!({"evidence": "attribute-value-binding"}),
            },
        )
        .await?;
    let product = catalog_owner
        .create_product(tenant_id, actor_id, source_product(category.id))
        .await?;
    let product_id = product.id;
    schema_owner
        .save_product_attribute_values(
            tenant_id,
            actor_id,
            product_id,
            " EN ",
            vec![ProductAttributeValuePatch {
                attribute_id: attribute.id,
                value: ProductAttributeValuePatchValue::Text("Professional audience".into()),
            }],
        )
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
    assert!(Uuid::parse_str(identity.resource_id.as_str()).is_ok());
    assert!(identity.subresource_id.is_none());
    assert!(
        provider
            .list_resources(
                read_context(other_tenant_id, "isolation"),
                list_request.clone(),
            )
            .await?
            .resources
            .is_empty()
    );

    let read_request = ReadTranslationResourceRequest {
        identity: identity.clone(),
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    };
    let initial = provider
        .read_resource(read_context(tenant_id, "initial"), read_request.clone())
        .await?;
    assert_eq!(initial.summary.display_label, "audience_note");
    assert_eq!(initial.fields.len(), 1);
    let value = field(&initial);
    assert_eq!(value.descriptor.key.as_str(), "value");
    assert_eq!(value.source_value, "Professional audience");
    assert!(value.exact_target_value.is_none());
    assert!(value.descriptor.required);
    assert_eq!(value.descriptor.profile, TranslationValueProfile::PlainText);
    assert_eq!(
        value.descriptor.classification,
        TranslationDataClassification::TenantPrivate
    );
    assert!(!value.descriptor.ai_export_allowed);

    let initial_changes = provider
        .read_changes(
            read_context(tenant_id, "owner-create"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    assert_eq!(initial_changes.changes.len(), 1);
    assert_eq!(initial_changes.changes[0].identity, identity);
    assert_eq!(
        initial_changes.changes[0].resource_revision,
        initial.summary.resource_revision
    );
    assert_eq!(
        initial_changes.changes[0].lifecycle,
        TranslationResourceLifecycle::Active
    );
    let owner_cursor = initial_changes.next_cursor.expect("owner cursor");

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
    assert_eq!(initial_progress.required_units, 1);
    assert_eq!(initial_progress.exact_required_units, 0);
    assert_eq!(initial_progress.optional_units, 0);
    assert_eq!(initial_progress.exact_optional_units, 0);
    assert_eq!(
        initial_progress.owner_change_cursor.as_ref(),
        Some(&owner_cursor)
    );

    let first_connection = connect_postgres(database_url).await?;
    let second_connection = connect_postgres(database_url).await?;
    let first_provider = registered_provider(first_connection.clone());
    let second_provider = registered_provider(second_connection.clone());
    let first_replica_snapshot = first_provider
        .read_resource(read_context(tenant_id, "replica-one"), read_request.clone())
        .await?;
    let second_replica_snapshot = second_provider
        .read_resource(read_context(tenant_id, "replica-two"), read_request.clone())
        .await?;
    assert_eq!(
        first_replica_snapshot.summary.resource_revision,
        second_replica_snapshot.summary.resource_revision
    );
    assert_eq!(
        first_replica_snapshot.source_revision,
        second_replica_snapshot.source_revision
    );

    let first_patch = patch(&first_replica_snapshot, "Public professionnel", "first");
    let first_receipt = first_provider
        .apply_patch(
            apply_context(tenant_id, "first-apply", "attribute-value-first-apply"),
            first_patch.clone(),
        )
        .await?;
    let replay = second_provider
        .apply_patch(
            apply_context(tenant_id, "first-replay", "attribute-value-first-apply"),
            first_patch,
        )
        .await?;
    assert_eq!(replay.provider_receipt_id, first_receipt.provider_receipt_id);
    assert_eq!(replay.resource_revision, first_receipt.resource_revision);
    assert_eq!(replay.target_revision, first_receipt.target_revision);

    let after_first = first_provider
        .read_resource(read_context(tenant_id, "after-first"), read_request.clone())
        .await?;
    assert_eq!(field(&after_first).exact_target_value.as_deref(), Some("Public professionnel"));

    let first_race_snapshot = first_provider
        .read_resource(read_context(tenant_id, "race-one-read"), read_request.clone())
        .await?;
    let second_race_snapshot = second_provider
        .read_resource(read_context(tenant_id, "race-two-read"), read_request.clone())
        .await?;
    let first_apply = first_provider.apply_patch(
        apply_context(tenant_id, "race-one", "attribute-value-race-one"),
        patch(&first_race_snapshot, "Public A", "race-one"),
    );
    let second_apply = second_provider.apply_patch(
        apply_context(tenant_id, "race-two", "attribute-value-race-two"),
        patch(&second_race_snapshot, "Public B", "race-two"),
    );
    let (first_result, second_result) = tokio::join!(first_apply, second_apply);
    let winner = match (first_result, second_result) {
        (Ok(receipt), Err(error)) | (Err(error), Ok(receipt)) => {
            assert_eq!(error.kind, PortErrorKind::Conflict);
            receipt
        }
        other => panic!("expected exactly one concurrent CAS winner: {other:?}"),
    };

    let progress = provider
        .read_progress(
            read_context(tenant_id, "translated-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(progress.resources, 1);
    assert_eq!(progress.complete_resources, 1);
    assert_eq!(progress.required_units, 1);
    assert_eq!(progress.exact_required_units, 1);

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

    schema_owner
        .save_product_attribute_values(
            tenant_id,
            actor_id,
            product_id,
            "en",
            vec![ProductAttributeValuePatch {
                attribute_id: attribute.id,
                value: ProductAttributeValuePatchValue::Text("Updated professional audience".into()),
            }],
        )
        .await?;
    let late_snapshot = provider
        .read_resource(read_context(tenant_id, "late-source"), read_request.clone())
        .await?;
    assert_eq!(field(&late_snapshot).source_value, "Updated professional audience");

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
    assert_eq!(frozen_second.changes[0].resource_revision, winner.resource_revision);
    let frozen_terminal = frozen_second.next_cursor.expect("frozen terminal cursor");

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
    assert_eq!(
        next_window.changes[0].resource_revision,
        late_snapshot.summary.resource_revision
    );
    assert_eq!(
        next_window.changes[0].lifecycle,
        TranslationResourceLifecycle::Active
    );
    let pre_delete_cursor = next_window.next_cursor.expect("pre-delete cursor");

    schema_owner
        .save_product_attribute_values(
            tenant_id,
            actor_id,
            product_id,
            "en",
            vec![ProductAttributeValuePatch {
                attribute_id: attribute.id,
                value: ProductAttributeValuePatchValue::Clear,
            }],
        )
        .await?;

    let deleted = provider
        .read_changes(
            read_context(tenant_id, "deleted-change"),
            TranslationTargetChangesRequest {
                after: Some(pre_delete_cursor),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(deleted.changes.len(), 1);
    assert_eq!(deleted.changes[0].identity, identity);
    assert_eq!(
        deleted.changes[0].lifecycle,
        TranslationResourceLifecycle::Deleted
    );
    assert!(
        deleted.changes[0]
            .resource_revision
            .as_str()
            .starts_with("product-attribute-value-deleted-v1:")
    );
    let deleted_cursor = deleted.next_cursor.expect("deleted cursor");

    let read_error = provider
        .read_resource(read_context(tenant_id, "deleted-read"), read_request)
        .await
        .expect_err("cleared Product attribute value must not remain readable");
    assert_eq!(read_error.kind, PortErrorKind::NotFound);
    let after_delete = provider
        .list_resources(read_context(tenant_id, "deleted-list"), list_request)
        .await?;
    assert!(after_delete.resources.is_empty());
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
    assert_eq!(deleted_progress.required_units, 0);
    assert_eq!(deleted_progress.exact_required_units, 0);
    assert_eq!(
        deleted_progress.owner_change_cursor.as_ref(),
        Some(&deleted_cursor)
    );

    drop(provider);
    drop(first_provider);
    drop(second_provider);
    first_connection.close().await?;
    second_connection.close().await?;
    seed_connection.close().await?;
    Ok(())
}

fn schema_owner(db: DatabaseConnection) -> ProductCatalogSchemaService {
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    ProductCatalogSchemaService::new(db, event_bus)
}

fn catalog_owner(db: DatabaseConnection) -> CatalogService {
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    CatalogService::new(db, event_bus)
}

fn registered_provider(db: DatabaseConnection) -> Arc<dyn TranslationTargetProvider> {
    let registry = ModuleRegistry::new().register(ProductModule);
    let settings = RustokSettings::default();
    let runtime = ServerRuntimeContext::new(db, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime,
        AuthConfig::new("test-secret-key-for-attribute-value-32bytes!".to_string()),
    )
    .expect("host composition must succeed");
    let targets = translation_target_registry(&extensions).expect("Translation registry");
    targets
        .get(
            &OwnerSlug::new(OWNER_SLUG).expect("owner slug"),
            &ResourceKind::new(RESOURCE_KIND).expect("resource kind"),
        )
        .expect("product/attribute_value provider")
}

fn source_category() -> CreateCatalogCategoryInput {
    CreateCatalogCategoryInput {
        parent_id: None,
        code: "attribute-value-category".into(),
        slug: "attribute-value-category".into(),
        kind: CatalogCategoryKind::Structural,
        position: 0,
        rule_config: json!({}),
        metadata: json!({"evidence": "attribute-value-translation"}),
        translations: vec![CategoryTranslationInput {
            locale: "en".into(),
            name: "Attribute value category".into(),
            description: None,
            meta_title: None,
            meta_description: None,
        }],
    }
}

fn localized_attribute() -> CreateProductAttributeInput {
    CreateProductAttributeInput {
        code: "audience_note".into(),
        value_type: AttributeValueType::Text,
        scope: "product".into(),
        is_localized: true,
        is_filterable: false,
        is_searchable: true,
        is_sortable: false,
        is_comparable: false,
        show_on_storefront: true,
        show_in_admin_grid: true,
        search_weight: 1,
        filter_display: None,
        facet_mode: None,
        position: 0,
        validation: json!({}),
        default_value: None,
        metadata: json!({"evidence": "attribute-value-translation"}),
        translations: vec![AttributeTranslationInput {
            locale: "en".into(),
            label: "Audience note".into(),
            help_text: None,
            facet_label: None,
            seo_label: None,
        }],
    }
}

fn source_product(category_id: Uuid) -> CreateProductInput {
    CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en".into(),
            title: "Attribute Value Evidence Product".into(),
            handle: Some("attribute-value-evidence-product".into()),
            description: None,
            meta_title: None,
            meta_description: None,
        }],
        variant_axes: Vec::new(),
        variants: vec![CreateVariantInput {
            sku: Some("ATTRIBUTE-VALUE-EVIDENCE-SKU".into()),
            barcode: None,
            shipping_profile_slug: None,
            axis_values: Vec::new(),
            prices: Vec::new(),
            inventory_quantity: 0,
            inventory_policy: "deny".into(),
            weight: None,
            weight_unit: None,
        }],
        seller_id: None,
        vendor: Some("Evidence Vendor".into()),
        product_type: Some("evidence".into()),
        shipping_profile_slug: None,
        primary_category_id: Some(category_id),
        tags: Vec::new(),
        metadata: json!({"evidence": "attribute-value-translation"}),
        publish: false,
    }
}

fn patch(snapshot: &TranslationResourceSnapshot, value: &str, suffix: &str) -> TranslationPatchRequest {
    TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: snapshot.target_revision.clone(),
        fields: vec![TranslationFieldPatch {
            key: field(snapshot).descriptor.key.clone(),
            value: value.into(),
            expected_source_hash: field(snapshot).source_hash.clone(),
        }],
        proposal_id: format!("attribute-value-proposal-{suffix}"),
        approval_receipt_id: format!("attribute-value-approval-{suffix}"),
    }
}

fn field(snapshot: &TranslationResourceSnapshot) -> &rustok_translation_targets::TranslationFieldSnapshot {
    snapshot
        .fields
        .iter()
        .find(|field| field.descriptor.key.as_str() == "value")
        .expect("attribute value translation field")
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("attribute-value-postgres-{suffix}"),
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
                "Product Attribute Value translation evidence".into(),
                format!("{slug_prefix}-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}
