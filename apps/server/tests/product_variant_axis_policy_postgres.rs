/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

#![cfg(feature = "mod-product")]

use rustok_migrations::Migrator;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_product::{
    CatalogService, CommerceError, ProductCatalogCommandPort, ProductCatalogSchemaWritePort,
    dto::{
        CreateProductInput, CreateVariantInput, ProductTranslationInput, SetVariantAxesInput,
        VariantAxisInput, VariantAxisPolicy, VariantAxisReassignmentInput, VariantAxisValueInput,
    },
    services::{
        AttributeDefinitionAction, AttributeDefinitionCopy, AttributeDefinitionPatch,
        AttributeDefinitionRefusal, AttributeLifecycleRequest, AttributeLifecycleWorkspace,
        AttributeOptionDefinitionPatch, AttributeOptionTranslationInput, AttributeTranslationInput,
        AttributeValueType, BindCategoryAttributeInput, BindSchemaAttributeInput,
        CatalogCategoryKind, CategoryAttributeBindingKind, CategorySchemaMode,
        CategoryTranslationInput, CreateCatalogCategoryInput, CreateProductAttributeInput,
        CreateProductAttributeOptionInput, CreateProductAttributeSchemaInput,
        ProductCatalogSchemaService, SchemaTranslationInput, SetCategorySchemaModeInput,
        UpdateAttributeDefinitionInput,
    },
};
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, FromQueryResult, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use std::{error::Error, sync::Arc, time::Duration};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn effective_axis_policy_sparse_creation_rollback_and_concurrent_product_writes_postgres()
-> TestResult<()> {
    let admin_url = std::env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".into());
    assert_postgres_url(&admin_url);
    let name = unique_postgres_database_name("rustok_product_axis_policy");
    let url = postgres_database_url(&admin_url, &name);
    let admin = connect_postgres(&admin_url).await?;
    create_postgres_database(&admin, &name).await?;
    let result = run_contract(&url).await;
    drop_postgres_database_if_exists(&admin, &name).await?;
    admin.close().await?;
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access; source only until run by the maintainer"]
async fn attribute_definition_lifecycle_receipts_and_reference_fences_postgres() -> TestResult<()> {
    let admin_url = std::env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".into());
    assert_postgres_url(&admin_url);
    let name = unique_postgres_database_name("rustok_attribute_lifecycle");
    let url = postgres_database_url(&admin_url, &name);
    let admin = connect_postgres(&admin_url).await?;
    create_postgres_database(&admin, &name).await?;
    let result = attribute_definition_contract(&url).await;
    drop_postgres_database_if_exists(&admin, &name).await?;
    admin.close().await?;
    result
}

async fn run_contract(url: &str) -> TestResult<()> {
    let db = connect_postgres(url).await?;
    Migrator::up(&db, None).await?;
    let tenant = Uuid::new_v4();
    let other_tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    for id in [tenant, other_tenant] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id, name, slug) VALUES ($1, 'Axis policy evidence', $2)",
            vec![id.into(), format!("axis-{}", id.simple()).into()],
        ))
        .await?;
    }
    let owner = schema_owner(db.clone());
    let catalog = catalog_owner(db.clone());
    let schema = owner
        .create_schema(
            tenant,
            actor,
            CreateProductAttributeSchemaInput {
                code: "axes".into(),
                metadata: json!({}),
                translations: vec![SchemaTranslationInput {
                    locale: "en".into(),
                    name: "Axes".into(),
                    description: None,
                }],
            },
        )
        .await?;
    let color = owner
        .create_attribute(tenant, actor, attribute("color", "both"))
        .await?;
    let size = owner
        .create_attribute(tenant, actor, attribute("size", "variant"))
        .await?;
    let material = owner
        .create_attribute(tenant, actor, attribute("material", "variant"))
        .await?;
    let product_only = owner
        .create_attribute(tenant, actor, attribute("product_only", "product"))
        .await?;
    let invalid = schema_binding(
        schema.id,
        product_only.id,
        Some(VariantAxisPolicy::Allowed),
        Some(false),
    );
    assert!(matches!(
        owner.bind_schema_attribute(tenant, actor, invalid).await,
        Err(CommerceError::Validation(_))
    ));
    for attribute in [color.id, size.id] {
        owner
            .bind_schema_attribute(
                tenant,
                actor,
                schema_binding(
                    schema.id,
                    attribute,
                    Some(VariantAxisPolicy::Allowed),
                    Some(true),
                ),
            )
            .await?;
        owner
            .bind_schema_attribute(
                tenant,
                actor,
                schema_binding(schema.id, attribute, None, None),
            )
            .await?;
    }
    let red = option(&owner, tenant, actor, color.id, "red").await?;
    let blue = option(&owner, tenant, actor, color.id, "blue").await?;
    let small = option(&owner, tenant, actor, size.id, "small").await?;
    let large = option(&owner, tenant, actor, size.id, "large").await?;
    let parent = owner
        .create_category(tenant, actor, category("parent", None))
        .await?;
    let child = owner
        .create_category(tenant, actor, category("child", Some(parent.id)))
        .await?;
    let clone = owner
        .create_category(tenant, actor, category("clone", None))
        .await?;
    owner
        .set_category_schema_mode(
            tenant,
            actor,
            SetCategorySchemaModeInput {
                category_id: parent.id,
                mode: CategorySchemaMode::UseSchema,
                schema_id: Some(schema.id),
                clone_from_category_id: None,
            },
        )
        .await?;
    owner
        .set_category_schema_mode(
            tenant,
            actor,
            SetCategorySchemaModeInput {
                category_id: clone.id,
                mode: CategorySchemaMode::CloneFromCategory,
                schema_id: None,
                clone_from_category_id: Some(child.id),
            },
        )
        .await?;
    let inherited = owner
        .load_effective_form_for_category(tenant, child.id, &[])
        .await?;
    assert!(
        inherited
            .attributes
            .iter()
            .all(|attribute| attribute.variant_axis_policy == "allowed"
                && attribute.default_variant_axis)
    );

    let axes = vec![
        VariantAxisInput {
            attribute_id: color.id,
            position: 0,
            allowed_option_ids: vec![red, blue],
        },
        VariantAxisInput {
            attribute_id: size.id,
            position: 1,
            allowed_option_ids: vec![small, large],
        },
    ];
    let rows = vec![
        variant("red-small", vec![(color.id, red), (size.id, small)], 3),
        variant("blue-large", vec![(color.id, blue), (size.id, large)], 5),
    ];
    let created = catalog
        .create_product(
            tenant,
            actor,
            product("sparse", child.id, axes.clone(), rows.clone()),
        )
        .await?;
    assert_eq!(
        created.variants.len(),
        2,
        "no default or unselected Cartesian tuples may be materialized"
    );
    assert!(
        created
            .variants
            .iter()
            .all(|variant| variant.axis_values.len() == 2)
    );
    assert_eq!(created.variant_axes.len(), 2);
    let original_ids = created
        .variants
        .iter()
        .map(|variant| variant.id)
        .collect::<Vec<_>>();
    let events_before = count(&db, tenant, "sys_events").await?;
    let error = owner
        .bind_schema_attribute(
            tenant,
            actor,
            schema_binding(
                schema.id,
                color.id,
                Some(VariantAxisPolicy::Forbidden),
                Some(false),
            ),
        )
        .await
        .unwrap_err();
    let CommerceError::VariantAxisPolicyConflict(conflict) = error else {
        panic!("expected existing-product impact conflict");
    };
    assert!(
        conflict
            .violations
            .iter()
            .any(|violation| violation.product_id == created.id
                && violation.category_id == child.id
                && violation.attribute_id == color.id)
    );
    assert_eq!(
        count(&db, tenant, "sys_events").await?,
        events_before,
        "refused policy writes must not publish outbox events"
    );
    assert!(
        owner
            .load_effective_form_for_category(tenant, child.id, &[])
            .await?
            .attributes
            .iter()
            .all(|attribute| attribute.variant_axis_policy == "allowed")
    );

    owner
        .bind_category_attribute(
            tenant,
            actor,
            category_binding(child.id, color.id, VariantAxisPolicy::Allowed),
        )
        .await?;
    owner
        .bind_schema_attribute(
            tenant,
            actor,
            schema_binding(
                schema.id,
                color.id,
                Some(VariantAxisPolicy::Forbidden),
                Some(false),
            ),
        )
        .await?;
    assert_eq!(
        owner
            .load_effective_form_for_category(tenant, child.id, &[])
            .await?
            .attributes
            .iter()
            .find(|attribute| attribute.attribute_id == color.id)
            .unwrap()
            .variant_axis_policy,
        "allowed"
    );
    let snapshot = owner
        .load_effective_form_for_category(tenant, clone.id, &[])
        .await?;
    assert!(
        snapshot
            .attributes
            .iter()
            .all(|attribute| attribute.variant_axis_policy == "allowed"
                && attribute.default_variant_axis),
        "clone policies must not follow later schema changes"
    );
    assert!(matches!(
        owner
            .set_category_schema_mode(
                tenant,
                actor,
                SetCategorySchemaModeInput {
                    category_id: child.id,
                    mode: CategorySchemaMode::Custom,
                    schema_id: None,
                    clone_from_category_id: None
                }
            )
            .await,
        Err(CommerceError::VariantAxisPolicyConflict(_))
    ));
    assert_eq!(
        owner
            .load_effective_form_for_category(tenant, child.id, &[])
            .await?
            .attributes
            .len(),
        2,
        "refused mode changes must retain the previous inherited schema"
    );
    let error = owner
        .bind_schema_attribute(
            tenant,
            actor,
            schema_binding(
                schema.id,
                material.id,
                Some(VariantAxisPolicy::Required),
                Some(false),
            ),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, CommerceError::VariantAxisPolicyConflict(_)));
    assert!(
        !owner
            .load_effective_form_for_category(tenant, child.id, &[])
            .await?
            .attributes
            .iter()
            .any(|attribute| attribute.attribute_id == material.id)
    );
    assert!(
        owner
            .bind_category_attribute(
                other_tenant,
                actor,
                category_binding(child.id, color.id, VariantAxisPolicy::Allowed)
            )
            .await
            .is_err()
    );
    let after = catalog.get_product(tenant, created.id).await?;
    assert_eq!(
        after
            .variants
            .iter()
            .map(|variant| variant.id)
            .collect::<Vec<_>>(),
        original_ids,
        "schema writes must not recreate SKU identities"
    );

    // Fail after at least one candidate SKU can be provisioned. Everything, including stock and
    // outbox rows, must roll back rather than leaving a temporary Product/default SKU behind.
    let totals = (
        count(&db, tenant, "products").await?,
        count(&db, tenant, "product_variants").await?,
        count(&db, tenant, "inventory_items").await?,
        count(&db, tenant, "sys_events").await?,
    );
    let mut invalid_rows = rows.clone();
    invalid_rows[0].sku = Some("new-before-rollback".into());
    assert!(
        catalog
            .create_product(
                tenant,
                actor,
                product("rollback", child.id, axes.clone(), invalid_rows)
            )
            .await
            .is_err()
    );
    assert_eq!(
        (
            count(&db, tenant, "products").await?,
            count(&db, tenant, "product_variants").await?,
            count(&db, tenant, "inventory_items").await?,
            count(&db, tenant, "sys_events").await?
        ),
        totals
    );

    // Real separate database connections, not a process-local mutex: exactly one contender may
    // commit when a policy withdrawal races initial Product + axes + SKU creation.
    let race = owner
        .create_category(tenant, actor, category("race", None))
        .await?;
    owner
        .bind_category_attribute(
            tenant,
            actor,
            category_binding(race.id, color.id, VariantAxisPolicy::Allowed),
        )
        .await?;
    let first = connect_postgres(url).await?;
    let second = connect_postgres(url).await?;
    let writer = catalog_owner(first.clone());
    let policy_writer = schema_owner(second.clone());
    let barrier = tokio::sync::Barrier::new(2);
    let create = async {
        barrier.wait().await;
        writer
            .create_product(
                tenant,
                actor,
                product(
                    "racing-product",
                    race.id,
                    vec![VariantAxisInput {
                        attribute_id: color.id,
                        position: 0,
                        allowed_option_ids: vec![red],
                    }],
                    vec![variant("racing-red", vec![(color.id, red)], 1)],
                ),
            )
            .await
    };
    let withdraw = async {
        barrier.wait().await;
        policy_writer
            .bind_category_attribute(
                tenant,
                actor,
                category_binding(race.id, color.id, VariantAxisPolicy::Forbidden),
            )
            .await
    };
    let (create, withdraw) = tokio::time::timeout(Duration::from_secs(20), async {
        tokio::join!(create, withdraw)
    })
    .await?;
    assert_ne!(
        create.is_ok(),
        withdraw.is_ok(),
        "exactly one conflicting operation may commit"
    );
    let final_form = owner
        .load_effective_form_for_category(tenant, race.id, &[])
        .await?;
    assert_eq!(
        final_form.attributes[0].variant_axis_policy,
        if create.is_ok() {
            "allowed"
        } else {
            "forbidden"
        }
    );
    first.close().await?;
    second.close().await?;
    reconfiguration_contract(
        &db,
        &owner,
        &catalog,
        tenant,
        actor,
        [color.id, size.id],
        [[red, blue], [small, large]],
    )
    .await?;
    standalone_recovery_contract(&db, &owner, &catalog, tenant, actor, color.id, [red, blue])
        .await?;
    db.close().await?;
    Ok(())
}

fn schema_owner(db: DatabaseConnection) -> ProductCatalogSchemaService {
    let bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    ProductCatalogSchemaService::new(db, bus)
}
fn catalog_owner(db: DatabaseConnection) -> CatalogService {
    let bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    CatalogService::new(db, bus)
}
fn category(code: &str, parent_id: Option<Uuid>) -> CreateCatalogCategoryInput {
    CreateCatalogCategoryInput {
        parent_id,
        code: code.into(),
        slug: code.into(),
        kind: CatalogCategoryKind::Structural,
        position: 0,
        rule_config: json!({}),
        metadata: json!({}),
        translations: vec![CategoryTranslationInput {
            locale: "en".into(),
            name: code.into(),
            description: None,
            meta_title: None,
            meta_description: None,
        }],
    }
}
fn attribute(code: &str, scope: &str) -> CreateProductAttributeInput {
    CreateProductAttributeInput {
        code: code.into(),
        value_type: AttributeValueType::Select,
        scope: scope.into(),
        is_localized: false,
        is_filterable: false,
        is_searchable: false,
        is_sortable: false,
        is_comparable: false,
        show_on_storefront: true,
        show_in_admin_grid: false,
        search_weight: 1,
        filter_display: None,
        facet_mode: None,
        position: 0,
        validation: json!({}),
        default_value: None,
        metadata: json!({}),
        translations: vec![AttributeTranslationInput {
            locale: "en".into(),
            label: code.into(),
            help_text: None,
            facet_label: None,
            seo_label: None,
        }],
    }
}
async fn option(
    owner: &ProductCatalogSchemaService,
    tenant: Uuid,
    actor: Uuid,
    attribute_id: Uuid,
    code: &str,
) -> TestResult<Uuid> {
    Ok(owner
        .create_attribute_option(
            tenant,
            actor,
            CreateProductAttributeOptionInput {
                attribute_id,
                code: code.into(),
                position: 0,
                metadata: json!({}),
                translations: vec![AttributeOptionTranslationInput {
                    locale: "en".into(),
                    label: code.into(),
                }],
            },
        )
        .await?
        .id)
}
fn schema_binding(
    schema_id: Uuid,
    attribute_id: Uuid,
    policy: Option<VariantAxisPolicy>,
    hint: Option<bool>,
) -> BindSchemaAttributeInput {
    BindSchemaAttributeInput {
        schema_id,
        attribute_id,
        group_code: None,
        is_required: false,
        is_disabled: false,
        position: 0,
        visibility_overrides: json!({}),
        validation_overrides: json!({}),
        metadata: json!({}),
        variant_axis_policy: policy,
        default_variant_axis: hint,
    }
}
fn category_binding(
    category_id: Uuid,
    attribute_id: Uuid,
    policy: VariantAxisPolicy,
) -> BindCategoryAttributeInput {
    BindCategoryAttributeInput {
        category_id,
        attribute_id,
        group_code: None,
        binding_kind: CategoryAttributeBindingKind::Addition,
        is_required: None,
        is_disabled: false,
        position: Some(0),
        visibility_overrides: json!({}),
        validation_overrides: json!({}),
        metadata: json!({}),
        variant_axis_policy: Some(policy),
        default_variant_axis: Some(false),
    }
}
fn variant(sku: &str, values: Vec<(Uuid, Uuid)>, quantity: i32) -> CreateVariantInput {
    CreateVariantInput {
        sku: Some(sku.into()),
        barcode: None,
        shipping_profile_slug: None,
        axis_values: values
            .into_iter()
            .map(|(attribute_id, option_id)| VariantAxisValueInput {
                attribute_id,
                option_id,
            })
            .collect(),
        prices: vec![],
        inventory_quantity: quantity,
        inventory_policy: "deny".into(),
        weight: None,
        weight_unit: None,
    }
}
fn product(
    code: &str,
    category: Uuid,
    variant_axes: Vec<VariantAxisInput>,
    variants: Vec<CreateVariantInput>,
) -> CreateProductInput {
    CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en".into(),
            title: code.into(),
            handle: Some(code.into()),
            description: None,
            meta_title: None,
            meta_description: None,
        }],
        primary_category_id: Some(category),
        variant_axes,
        variants,
        ..Default::default()
    }
}
#[derive(FromQueryResult)]
struct Count {
    count: i64,
}
async fn count(db: &DatabaseConnection, tenant: Uuid, table: &str) -> TestResult<i64> {
    let sql = if table == "inventory_items" {
        "SELECT COUNT(*)::bigint AS count FROM inventory_items i JOIN product_variants v ON v.id = i.variant_id WHERE v.tenant_id = $1".to_string()
    } else {
        format!("SELECT COUNT(*)::bigint AS count FROM {table} WHERE tenant_id = $1")
    };
    Ok(Count::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        vec![tenant.into()],
    ))
    .one(db)
    .await?
    .ok_or("count missing")?
    .count)
}

// Source only: the maintainer runs this with PostgreSQL. It exercises actual deferred EAV
// constraints, immediate position/combination indexes, owner receipts and outbox coupling.
async fn reconfiguration_contract(
    db: &DatabaseConnection,
    schema: &ProductCatalogSchemaService,
    catalog: &CatalogService,
    tenant: Uuid,
    actor: Uuid,
    attributes: [Uuid; 2],
    options: [[Uuid; 2]; 2],
) -> TestResult<()> {
    let [color, size] = attributes;
    let [[red, blue], [small, large]] = options;
    let category = schema
        .create_category(tenant, actor, category("reconfiguration", None))
        .await?;
    for attribute in [color, size] {
        schema
            .bind_category_attribute(
                tenant,
                actor,
                category_binding(category.id, attribute, VariantAxisPolicy::Allowed),
            )
            .await?;
    }
    let original = catalog
        .create_product(
            tenant,
            actor,
            product(
                "reconfiguration",
                category.id,
                vec![
                    VariantAxisInput {
                        attribute_id: color,
                        position: 0,
                        allowed_option_ids: vec![red, blue],
                    },
                    VariantAxisInput {
                        attribute_id: size,
                        position: 1,
                        allowed_option_ids: vec![small, large],
                    },
                ],
                vec![
                    variant("reconfiguration-left", vec![(color, red), (size, small)], 3),
                    variant(
                        "reconfiguration-right",
                        vec![(color, red), (size, large)],
                        5,
                    ),
                ],
            ),
        )
        .await?;
    let stock = stock_links(db, tenant, original.id).await?;
    let options = axis_links(db, tenant, original.id).await?;
    let values = value_links(db, tenant, original.id).await?;
    let event_count = count(db, tenant, "sys_events").await?;
    let context = |key: &str| {
        rustok_api::PortContext::new(
            tenant.to_string(),
            rustok_api::PortActor::user(actor.to_string()),
            "en",
            format!("axes-reconfiguration:{key}"),
        )
        .with_deadline(Duration::from_secs(20))
        .with_idempotency_key(key)
        .with_claim("products:update")
    };
    let mut proposal = SetVariantAxesInput {
        axes: vec![
            VariantAxisInput {
                attribute_id: size,
                position: 0,
                allowed_option_ids: vec![large, small],
            },
            VariantAxisInput {
                attribute_id: color,
                position: 1,
                allowed_option_ids: vec![blue, red],
            },
        ],
        reassignments: vec![
            VariantAxisReassignmentInput {
                variant_id: original.variants[0].id,
                axis_values: vec![
                    VariantAxisValueInput {
                        attribute_id: color,
                        option_id: red,
                    },
                    VariantAxisValueInput {
                        attribute_id: size,
                        option_id: large,
                    },
                ],
            },
            VariantAxisReassignmentInput {
                variant_id: original.variants[1].id,
                axis_values: vec![
                    VariantAxisValueInput {
                        attribute_id: color,
                        option_id: red,
                    },
                    VariantAxisValueInput {
                        attribute_id: size,
                        option_id: small,
                    },
                ],
            },
        ],
        ..Default::default()
    };
    let preview = catalog
        .preview_variant_axes(tenant, original.id, proposal.clone())
        .await?;
    assert!(preview.can_apply && preview.has_changes);
    assert_eq!(preview.variant_count, 2);
    assert_eq!(
        count(db, tenant, "sys_events").await?,
        event_count,
        "preview has no outbox side effects"
    );
    proposal.expected_state = Some(preview.state_token);
    let key = Uuid::new_v4().to_string();
    let acknowledgement = ProductCatalogCommandPort::set_variant_axes(
        catalog,
        context(&key),
        original.id,
        proposal.clone(),
    )
    .await?;
    let changed = catalog.get_product(tenant, original.id).await?;
    assert_eq!(changed.revision, original.revision + 1);
    assert_eq!(
        changed
            .variants
            .iter()
            .map(|variant| (variant.id, variant.inventory_quantity))
            .collect::<Vec<_>>(),
        original
            .variants
            .iter()
            .map(|variant| (variant.id, variant.inventory_quantity))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        changed
            .variant_axes
            .iter()
            .map(|axis| (axis.attribute_id, axis.id))
            .collect::<std::collections::BTreeMap<_, _>>(),
        original
            .variant_axes
            .iter()
            .map(|axis| (axis.attribute_id, axis.id))
            .collect()
    );
    assert_eq!(
        axis_links(db, tenant, original.id).await?,
        options,
        "retained allowed-value row IDs survive position swaps"
    );
    assert_eq!(stock_links(db, tenant, original.id).await?, stock);
    assert_eq!(
        value_links(db, tenant, original.id).await?,
        values,
        "reassignment updates existing EAV values in place"
    );
    let after_event_count = count(db, tenant, "sys_events").await?;
    assert_eq!(after_event_count, event_count + 1);
    let replay = ProductCatalogCommandPort::set_variant_axes(
        catalog,
        context(&key),
        original.id,
        proposal.clone(),
    )
    .await?;
    assert_eq!(
        serde_json::to_value(&replay)?,
        serde_json::to_value(&acknowledgement)?,
        "the existing Vec receipt replays before predecessor validation"
    );
    assert_eq!(
        catalog.get_product(tenant, original.id).await?.revision,
        changed.revision
    );
    assert_eq!(count(db, tenant, "sys_events").await?, after_event_count);
    let stale_key = Uuid::new_v4().to_string();
    let stale = ProductCatalogCommandPort::set_variant_axes(
        catalog,
        context(&stale_key),
        original.id,
        proposal.clone(),
    )
    .await
    .unwrap_err();
    assert_eq!(stale.code, "product.variant_axes_stale");
    assert!(rustok_product::product_catalog_command_error_is_terminal(
        &stale
    ));
    let rejected_replay = ProductCatalogCommandPort::set_variant_axes(
        catalog,
        context(&stale_key),
        original.id,
        proposal,
    )
    .await
    .unwrap_err();
    assert_eq!(rejected_replay.code, stale.code);
    assert_eq!(count(db, tenant, "sys_events").await?, after_event_count);
    let mut drop_axis = SetVariantAxesInput {
        axes: vec![VariantAxisInput {
            attribute_id: color,
            position: 0,
            allowed_option_ids: vec![red, blue],
        }],
        ..Default::default()
    };
    let conflict = catalog
        .preview_variant_axes(tenant, original.id, drop_axis.clone())
        .await?;
    assert!(!conflict.can_apply);
    assert!(conflict.conflicts.iter().any(|conflict| conflict.kind
        == rustok_product::dto::VariantAxesConflictKind::DuplicateCombination));
    drop_axis.expected_state = Some(conflict.state_token);
    let refused = ProductCatalogCommandPort::set_variant_axes(
        catalog,
        context(&Uuid::new_v4().to_string()),
        original.id,
        drop_axis.clone(),
    )
    .await
    .unwrap_err();
    assert_eq!(refused.code, "product.variant_axes_conflict");
    assert_eq!(
        catalog.get_product(tenant, original.id).await?.revision,
        changed.revision
    );
    drop_axis.expected_state = None;
    drop_axis.reassignments.push(VariantAxisReassignmentInput {
        variant_id: original.variants[1].id,
        axis_values: vec![VariantAxisValueInput {
            attribute_id: color,
            option_id: blue,
        }],
    });
    let resolved = catalog
        .preview_variant_axes(tenant, original.id, drop_axis.clone())
        .await?;
    assert!(resolved.can_apply);
    drop_axis.expected_state = Some(resolved.state_token);
    ProductCatalogCommandPort::set_variant_axes(
        catalog,
        context(&Uuid::new_v4().to_string()),
        original.id,
        drop_axis,
    )
    .await?;
    let final_product = catalog.get_product(tenant, original.id).await?;
    assert_eq!(final_product.variant_axes.len(), 1);
    assert_eq!(final_product.variants.len(), 2);
    assert_eq!(stock_links(db, tenant, original.id).await?, stock);
    assert_eq!(
        value_links(db, tenant, original.id).await?,
        values,
        "removed-axis EAV rows and their IDs must not be deleted"
    );
    let mut noop = SetVariantAxesInput {
        axes: final_product
            .variant_axes
            .iter()
            .map(|axis| VariantAxisInput {
                attribute_id: axis.attribute_id,
                position: axis.position,
                allowed_option_ids: axis
                    .allowed_values
                    .iter()
                    .map(|value| value.option_id)
                    .collect(),
            })
            .collect(),
        ..Default::default()
    };
    let reviewed_noop = ProductCatalogCommandPort::preview_variant_axes(
        catalog,
        context("noop-preview"),
        original.id,
        noop.clone(),
    )
    .await?;
    assert!(reviewed_noop.can_apply);
    assert!(!reviewed_noop.has_changes);
    noop.expected_state = Some(reviewed_noop.state_token);
    let before_noop_events = count(db, tenant, "sys_events").await?;
    ProductCatalogCommandPort::set_variant_axes(
        catalog,
        context(&Uuid::new_v4().to_string()),
        original.id,
        noop,
    )
    .await?;
    assert_eq!(
        catalog.get_product(tenant, original.id).await?.revision,
        final_product.revision
    );
    assert_eq!(count(db, tenant, "sys_events").await?, before_noop_events);
    Ok(())
}

// Source only; no PostgreSQL execution is performed by the implementation agent.
async fn standalone_recovery_contract(
    db: &DatabaseConnection,
    schema: &ProductCatalogSchemaService,
    catalog: &CatalogService,
    tenant: Uuid,
    actor: Uuid,
    attribute: Uuid,
    options: [Uuid; 2],
) -> TestResult<()> {
    let category = schema
        .create_category(tenant, actor, category("standalone-recovery", None))
        .await?;
    schema
        .bind_category_attribute(
            tenant,
            actor,
            category_binding(category.id, attribute, VariantAxisPolicy::Allowed),
        )
        .await?;
    let original = catalog
        .create_product(
            tenant,
            actor,
            product(
                "standalone-recovery",
                category.id,
                vec![VariantAxisInput {
                    attribute_id: attribute,
                    position: 0,
                    allowed_option_ids: options.to_vec(),
                }],
                vec![variant(
                    "standalone-initial",
                    vec![(attribute, options[0])],
                    5,
                )],
            ),
        )
        .await?;
    let context = |operation, key: &str, locale: &str| {
        let scoped = rustok_product::scoped_caller_idempotency_key(
            tenant,
            actor,
            operation,
            Some(original.id),
            key,
        );
        rustok_api::PortContext::new(
            tenant.to_string(),
            rustok_api::PortActor::user(actor.to_string()),
            locale,
            scoped.clone(),
        )
        .with_idempotency_key(scoped)
        .with_deadline(Duration::from_secs(20))
        .with_claim("products:update")
    };
    let key = Uuid::new_v4().to_string();
    let input = variant("standalone-frozen", vec![(attribute, options[1])], 11);
    let created = ProductCatalogCommandPort::create_variant(
        catalog,
        context("create_product_variant", &key, "en"),
        original.id,
        input.clone(),
    )
    .await?;
    assert_eq!(created.inventory_quantity, 11);
    // A later stock update is not the response to the earlier create attempt.
    ProductCatalogCommandPort::update_variant(
        catalog,
        context("update_product_variant", &Uuid::new_v4().to_string(), "en"),
        created.id,
        rustok_product::dto::UpdateVariantInput {
            inventory_quantity: Some(19),
            ..Default::default()
        },
    )
    .await?;
    let stock = stock_links(db, tenant, original.id).await?;
    let values = value_links(db, tenant, original.id).await?;
    let events = count(db, tenant, "sys_events").await?;
    let replay = ProductCatalogCommandPort::create_variant(
        catalog,
        context("create_product_variant", &key, "en"),
        original.id,
        input.clone(),
    )
    .await?;
    assert_eq!(
        serde_json::to_value(&replay)?,
        serde_json::to_value(&created)?
    );
    assert_eq!(replay.id, created.id);
    assert_eq!(
        replay.inventory_quantity, 11,
        "replay uses the stored response, not the later stock read"
    );
    for field in 0..7 {
        let mut changed = input.clone();
        match field {
            0 => changed.sku = Some("another-sku".into()),
            1 => changed.barcode = Some("123".into()),
            2 => changed.shipping_profile_slug = Some("not-an-admitted-retry".into()),
            3 => changed.axis_values[0].option_id = options[0],
            4 => changed.prices.push(rustok_product::dto::PriceInput {
                currency_code: "USD".into(),
                channel_id: None,
                channel_slug: None,
                amount: "0".parse()?,
                compare_at_amount: Some("2".parse()?),
            }),
            5 => changed.inventory_quantity = 12,
            _ => changed.inventory_policy = "continue".into(),
        }
        let error = ProductCatalogCommandPort::create_variant(
            catalog,
            context("create_product_variant", &key, "en"),
            original.id,
            changed,
        )
        .await
        .unwrap_err();
        assert!(
            !rustok_product::product_catalog_command_error_is_terminal(&error),
            "receipt body mismatch is not permission to abandon the saved request"
        );
    }
    let changed_locale = ProductCatalogCommandPort::create_variant(
        catalog,
        context("create_product_variant", &key, "ru"),
        original.id,
        input,
    )
    .await
    .unwrap_err();
    assert!(!rustok_product::product_catalog_command_error_is_terminal(
        &changed_locale
    ));
    let current = catalog.get_product(tenant, original.id).await?;
    assert_eq!(current.variants.len(), 2);
    assert_eq!(
        current
            .variants
            .iter()
            .find(|variant| variant.id == created.id)
            .unwrap()
            .inventory_quantity,
        19
    );
    assert_eq!(stock_links(db, tenant, original.id).await?, stock);
    assert_eq!(value_links(db, tenant, original.id).await?, values);
    assert_eq!(count(db, tenant, "sys_events").await?, events);
    Ok(())
}

#[derive(Debug, PartialEq, Eq, FromQueryResult)]
struct StableLink {
    id: Uuid,
    parent_id: Uuid,
}
async fn axis_links(
    db: &DatabaseConnection,
    tenant: Uuid,
    product: Uuid,
) -> TestResult<Vec<StableLink>> {
    Ok(StableLink::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres, "SELECT value.id, value.axis_id AS parent_id FROM product_variant_axis_values value JOIN product_variant_axes axis ON axis.tenant_id = value.tenant_id AND axis.id = value.axis_id WHERE axis.tenant_id = $1 AND axis.product_id = $2 ORDER BY value.id", vec![tenant.into(), product.into()])).all(db).await?)
}
async fn value_links(
    db: &DatabaseConnection,
    tenant: Uuid,
    product: Uuid,
) -> TestResult<Vec<StableLink>> {
    Ok(StableLink::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres, "SELECT value.id, value.variant_id AS parent_id FROM product_variant_attribute_values value JOIN product_variants variant ON variant.tenant_id = value.tenant_id AND variant.id = value.variant_id WHERE value.tenant_id = $1 AND variant.product_id = $2 ORDER BY value.id", vec![tenant.into(), product.into()])).all(db).await?)
}
async fn stock_links(
    db: &DatabaseConnection,
    tenant: Uuid,
    product: Uuid,
) -> TestResult<Vec<StableLink>> {
    Ok(StableLink::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres, "SELECT item.id, item.variant_id AS parent_id FROM inventory_items item JOIN product_variants variant ON variant.id = item.variant_id WHERE variant.tenant_id = $1 AND variant.product_id = $2 ORDER BY item.id", vec![tenant.into(), product.into()])).all(db).await?)
}

// Shares only fixture builders with the axis contract. All data is disposable and every
// lifecycle write below uses the existing schema port/receipt, not direct service mutation.
fn definition_context(
    tenant: Uuid,
    actor: Uuid,
    key: &str,
    locale: &str,
) -> rustok_api::PortContext {
    let scoped = rustok_product::scoped_caller_idempotency_key(
        tenant,
        actor,
        "update_attribute_definition",
        None,
        key,
    );
    rustok_api::PortContext::new(
        tenant.to_string(),
        rustok_api::PortActor::user(actor.to_string()),
        locale,
        scoped.clone(),
    )
    .with_idempotency_key(scoped)
    .with_deadline(Duration::from_secs(20))
    .with_claim("products:manage")
}
async fn definition_workspace(
    owner: &ProductCatalogSchemaService,
    tenant: Uuid,
    attribute: Uuid,
    option: Option<Uuid>,
    locale: &str,
) -> TestResult<AttributeLifecycleWorkspace> {
    Ok(owner
        .read_attribute_lifecycle(
            tenant,
            locale,
            AttributeLifecycleRequest {
                attribute_id: Some(attribute),
                option_id: option,
                include_archived: true,
            },
        )
        .await?)
}
fn definition_input(
    workspace: &AttributeLifecycleWorkspace,
    option_id: Option<Uuid>,
    locale: &str,
    action: AttributeDefinitionAction,
    attribute_patch: Option<AttributeDefinitionPatch>,
    option_patch: Option<AttributeOptionDefinitionPatch>,
) -> TestResult<UpdateAttributeDefinitionInput> {
    let attribute = workspace
        .attributes
        .first()
        .ok_or("selected definition missing")?;
    let token = match option_id {
        None => attribute.state_token.clone(),
        Some(id) => workspace
            .options
            .as_ref()
            .and_then(|rows| rows.iter().find(|row| row.id == id))
            .ok_or("selected option missing")?
            .state_token
            .clone(),
    };
    Ok(UpdateAttributeDefinitionInput {
        attribute_id: attribute.id,
        option_id,
        locale: locale.into(),
        expected_state: token,
        action,
        attribute_patch,
        option_patch,
    })
}

async fn attribute_definition_contract(url: &str) -> TestResult<()> {
    let db = connect_postgres(url).await?;
    Migrator::up(&db, None).await?;
    let tenant = Uuid::new_v4();
    let other_tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    for id in [tenant, other_tenant] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id, name, slug) VALUES ($1, 'Definition lifecycle evidence', $2)",
            vec![id.into(), format!("definition-{}", id.simple()).into()],
        ))
        .await?;
    }
    let owner = schema_owner(db.clone());
    let catalog = catalog_owner(db.clone());
    let color = owner
        .create_attribute(tenant, actor, attribute("lifecycle_color", "both"))
        .await?;
    let red = option(&owner, tenant, actor, color.id, "red").await?;
    let blue = option(&owner, tenant, actor, color.id, "blue").await?;
    let initial = definition_workspace(&owner, tenant, color.id, None, "ru").await?;
    assert!(
        initial.attributes[0].translation.is_none(),
        "exact locale has no translated fallback"
    );
    assert!(
        initial
            .options
            .as_ref()
            .unwrap()
            .iter()
            .all(|row| row.label.is_none())
    );

    let key = Uuid::new_v4().to_string();
    let original = definition_input(
        &initial,
        None,
        "ru",
        AttributeDefinitionAction::Update,
        Some(AttributeDefinitionPatch {
            translation: Some(AttributeDefinitionCopy {
                label: "  Цвет  ".into(),
                help_text: Some("exact Russian copy".into()),
                facet_label: None,
                seo_label: None,
            }),
            is_searchable: Some(true),
            position: Some(4),
            ..Default::default()
        }),
        None,
    )?;
    let applied = ProductCatalogSchemaWritePort::update_attribute_definition(
        &owner,
        definition_context(tenant, actor, &key, "ru"),
        original.clone(),
    )
    .await?;
    assert!(applied.applied);
    assert_eq!(applied.attribute_id, color.id);
    let russian = definition_workspace(&owner, tenant, color.id, None, "ru").await?;
    assert_eq!(
        russian.attributes[0].translation.as_ref().unwrap().label,
        "  Цвет  "
    );
    let english = definition_workspace(&owner, tenant, color.id, None, "en").await?;
    assert_eq!(
        english.attributes[0].translation.as_ref().unwrap().label,
        "lifecycle_color"
    );
    assert_eq!(russian.attributes[0].code, initial.attributes[0].code);
    assert_eq!(
        russian.attributes[0].value_type,
        initial.attributes[0].value_type
    );
    assert_eq!(russian.attributes[0].scope, initial.attributes[0].scope);
    assert_eq!(
        russian.attributes[0].is_localized,
        initial.attributes[0].is_localized
    );

    let later = definition_input(
        &russian,
        None,
        "ru",
        AttributeDefinitionAction::Update,
        Some(AttributeDefinitionPatch {
            position: Some(5),
            ..Default::default()
        }),
        None,
    )?;
    ProductCatalogSchemaWritePort::update_attribute_definition(
        &owner,
        definition_context(tenant, actor, &Uuid::new_v4().to_string(), "ru"),
        later,
    )
    .await?;
    let events = count(&db, tenant, "sys_events").await?;
    let replay = ProductCatalogSchemaWritePort::update_attribute_definition(
        &owner,
        definition_context(tenant, actor, &key, "ru"),
        original.clone(),
    )
    .await?;
    assert_eq!(
        replay, applied,
        "replay returns the original receipt, not the later definition"
    );
    assert_eq!(count(&db, tenant, "sys_events").await?, events);
    let mut changed = original.clone();
    changed.attribute_patch.as_mut().unwrap().position = Some(99);
    let mismatch = ProductCatalogSchemaWritePort::update_attribute_definition(
        &owner,
        definition_context(tenant, actor, &key, "ru"),
        changed,
    )
    .await
    .unwrap_err();
    assert!(!rustok_product::product_catalog_command_error_is_terminal(
        &mismatch
    ));
    let mismatch = ProductCatalogSchemaWritePort::update_attribute_definition(
        &owner,
        definition_context(tenant, actor, &key, "en"),
        original.clone(),
    )
    .await
    .unwrap_err();
    assert_eq!(mismatch.code, "product.schema_locale_mismatch");
    assert!(!rustok_product::product_catalog_command_error_is_terminal(
        &mismatch
    ));
    let stale = ProductCatalogSchemaWritePort::update_attribute_definition(
        &owner,
        definition_context(tenant, actor, &Uuid::new_v4().to_string(), "ru"),
        original,
    )
    .await?;
    assert_eq!(stale.refusal, Some(AttributeDefinitionRefusal::Stale));
    assert_eq!(count(&db, tenant, "sys_events").await?, events);
    let foreign = definition_workspace(&owner, other_tenant, color.id, Some(red), "ru").await?;
    assert!(foreign.attributes.is_empty());
    assert!(foreign.options.is_none());
    assert!(foreign.impact.is_none());
    let missing =
        definition_workspace(&owner, tenant, color.id, Some(Uuid::new_v4()), "ru").await?;
    assert_eq!(missing.attributes[0].id, color.id);
    assert_eq!(missing.options.as_ref().unwrap().len(), 2);
    assert!(
        missing.impact.is_none(),
        "missing option must not receive the parent impact"
    );

    let before_option = definition_workspace(&owner, tenant, color.id, Some(blue), "ru").await?;
    let label = definition_input(
        &before_option,
        Some(blue),
        "ru",
        AttributeDefinitionAction::Update,
        None,
        Some(AttributeOptionDefinitionPatch {
            label: Some("Синий".into()),
            position: Some(2),
        }),
    )?;
    assert!(
        ProductCatalogSchemaWritePort::update_attribute_definition(
            &owner,
            definition_context(tenant, actor, &Uuid::new_v4().to_string(), "ru"),
            label,
        )
        .await?
        .applied
    );
    for action in [
        AttributeDefinitionAction::Archive,
        AttributeDefinitionAction::Restore,
    ] {
        let current = definition_workspace(&owner, tenant, color.id, Some(blue), "ru").await?;
        let input = definition_input(&current, Some(blue), "ru", action, None, None)?;
        assert!(
            ProductCatalogSchemaWritePort::update_attribute_definition(
                &owner,
                definition_context(tenant, actor, &Uuid::new_v4().to_string(), "ru"),
                input,
            )
            .await?
            .applied
        );
        let current = definition_workspace(&owner, tenant, color.id, Some(blue), "ru").await?;
        let row = current
            .options
            .as_ref()
            .unwrap()
            .iter()
            .find(|row| row.id == blue)
            .unwrap();
        assert_eq!(row.id, blue);
        assert_eq!(row.code, "blue");
        assert_eq!(row.label.as_deref(), Some("Синий"));
        assert_eq!(
            row.is_archived,
            action == AttributeDefinitionAction::Archive
        );
    }

    // Default metadata can precede option provisioning, without silently applying it to EAV.
    // Requiring a live option here would make every nonempty select default impossible to create.
    let mut hinted = attribute("deferred_default", "both");
    hinted.default_value = Some(json!("  future_code  "));
    let hinted = owner.create_attribute(tenant, actor, hinted).await?;
    let stored = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT default_value FROM product_attributes WHERE tenant_id = $1 AND id = $2",
            vec![tenant.into(), hinted.id.into()],
        ))
        .await?
        .ok_or("default metadata missing")?;
    assert_eq!(
        stored.try_get::<serde_json::Value>("", "default_value")?,
        json!("  future_code  ")
    );
    let hinted_option = option(&owner, tenant, actor, hinted.id, "future_code").await?;
    let current =
        definition_workspace(&owner, tenant, hinted.id, Some(hinted_option), "en").await?;
    let input = definition_input(
        &current,
        Some(hinted_option),
        "en",
        AttributeDefinitionAction::Archive,
        None,
        None,
    )?;
    let refused = ProductCatalogSchemaWritePort::update_attribute_definition(
        &owner,
        definition_context(tenant, actor, &Uuid::new_v4().to_string(), "en"),
        input,
    )
    .await?;
    assert_eq!(refused.refusal, Some(AttributeDefinitionRefusal::InUse));
    assert_eq!(count(&db, tenant, "product_attribute_values").await?, 0);
    let mut foreign_default = attribute("foreign_default", "both");
    foreign_default.default_value = Some(json!(blue.to_string()));
    assert!(matches!(
        owner.create_attribute(tenant, actor, foreign_default).await,
        Err(CommerceError::Validation(_))
    ));

    // Archive/restore do not cascade. A child restore refusal is itself a replayable receipt.
    let unused = owner
        .create_attribute(tenant, actor, attribute("lifecycle_unused", "both"))
        .await?;
    let child = option(&owner, tenant, actor, unused.id, "child").await?;
    for target in [Some(child), None] {
        let current = definition_workspace(&owner, tenant, unused.id, target, "en").await?;
        let input = definition_input(
            &current,
            target,
            "en",
            AttributeDefinitionAction::Archive,
            None,
            None,
        )?;
        assert!(
            ProductCatalogSchemaWritePort::update_attribute_definition(
                &owner,
                definition_context(tenant, actor, &Uuid::new_v4().to_string(), "en"),
                input,
            )
            .await?
            .applied
        );
    }
    let current = definition_workspace(&owner, tenant, unused.id, Some(child), "en").await?;
    let restore_child = definition_input(
        &current,
        Some(child),
        "en",
        AttributeDefinitionAction::Restore,
        None,
        None,
    )?;
    let refused_key = Uuid::new_v4().to_string();
    let refused = ProductCatalogSchemaWritePort::update_attribute_definition(
        &owner,
        definition_context(tenant, actor, &refused_key, "en"),
        restore_child.clone(),
    )
    .await?;
    assert_eq!(
        refused.refusal,
        Some(AttributeDefinitionRefusal::ParentArchived)
    );
    let restore_parent = definition_input(
        &current,
        None,
        "en",
        AttributeDefinitionAction::Restore,
        None,
        None,
    )?;
    assert!(
        ProductCatalogSchemaWritePort::update_attribute_definition(
            &owner,
            definition_context(tenant, actor, &Uuid::new_v4().to_string(), "en"),
            restore_parent,
        )
        .await?
        .applied
    );
    assert_eq!(
        ProductCatalogSchemaWritePort::update_attribute_definition(
            &owner,
            definition_context(tenant, actor, &refused_key, "en"),
            restore_child,
        )
        .await?,
        refused
    );
    let current = definition_workspace(&owner, tenant, unused.id, Some(child), "en").await?;
    assert!(
        current.options.as_ref().unwrap()[0].is_archived,
        "parent restore must not cascade"
    );
    let restore_child = definition_input(
        &current,
        Some(child),
        "en",
        AttributeDefinitionAction::Restore,
        None,
        None,
    )?;
    assert!(
        ProductCatalogSchemaWritePort::update_attribute_definition(
            &owner,
            definition_context(tenant, actor, &Uuid::new_v4().to_string(), "en"),
            restore_child,
        )
        .await?
        .applied
    );

    let category = owner
        .create_category(tenant, actor, category("lifecycle_product", None))
        .await?;
    owner
        .bind_category_attribute(
            tenant,
            actor,
            category_binding(category.id, color.id, VariantAxisPolicy::Allowed),
        )
        .await?;
    let product = catalog
        .create_product(
            tenant,
            actor,
            product(
                "lifecycle_sku",
                category.id,
                vec![VariantAxisInput {
                    attribute_id: color.id,
                    position: 0,
                    allowed_option_ids: vec![red],
                }],
                vec![variant("lifecycle_red", vec![(color.id, red)], 7)],
            ),
        )
        .await?;
    let links = (
        axis_links(&db, tenant, product.id).await?,
        value_links(&db, tenant, product.id).await?,
        stock_links(&db, tenant, product.id).await?,
    );
    let variant_ids = product
        .variants
        .iter()
        .map(|row| row.id)
        .collect::<Vec<_>>();
    let events = count(&db, tenant, "sys_events").await?;
    for target in [Some(red), None] {
        let current = definition_workspace(&owner, tenant, color.id, target, "ru").await?;
        let input = definition_input(
            &current,
            target,
            "ru",
            AttributeDefinitionAction::Archive,
            None,
            None,
        )?;
        let refused = ProductCatalogSchemaWritePort::update_attribute_definition(
            &owner,
            definition_context(tenant, actor, &Uuid::new_v4().to_string(), "ru"),
            input,
        )
        .await?;
        assert_eq!(refused.refusal, Some(AttributeDefinitionRefusal::InUse));
        assert!(
            refused
                .impact
                .as_ref()
                .is_some_and(|impact| impact.inspection_complete && !impact.references.is_empty())
        );
    }
    assert_eq!(count(&db, tenant, "sys_events").await?, events);
    assert_eq!(
        (
            axis_links(&db, tenant, product.id).await?,
            value_links(&db, tenant, product.id).await?,
            stock_links(&db, tenant, product.id).await?
        ),
        links
    );
    assert_eq!(
        catalog
            .get_product(tenant, product.id)
            .await?
            .variants
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        variant_ids
    );

    // Historical foreign-parent UUID defaults are still references; new writes cannot create
    // them, but lifecycle must not silently discard installed data merely because it is invalid.
    let carrier = owner
        .create_attribute(
            tenant,
            actor,
            attribute("legacy_default_carrier", "product"),
        )
        .await?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE product_attributes SET default_value = $3 WHERE tenant_id = $1 AND id = $2",
        vec![
            tenant.into(),
            carrier.id.into(),
            json!(blue.to_string()).into(),
        ],
    ))
    .await?;
    let current = definition_workspace(&owner, tenant, color.id, Some(blue), "ru").await?;
    let input = definition_input(
        &current,
        Some(blue),
        "ru",
        AttributeDefinitionAction::Archive,
        None,
        None,
    )?;
    let refused = ProductCatalogSchemaWritePort::update_attribute_definition(
        &owner,
        definition_context(tenant, actor, &Uuid::new_v4().to_string(), "ru"),
        input,
    )
    .await?;
    assert_eq!(refused.refusal, Some(AttributeDefinitionRefusal::InUse));
    assert!(
        refused
            .impact
            .as_ref()
            .unwrap()
            .references
            .iter()
            .any(|reference| reference.resource_id == carrier.id)
    );

    // Archiving an otherwise-unbound parent also checks foreign references to its child IDs.
    // Preserve surrounding whitespace in historical metadata; UUID comparison is structural.
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE product_attributes SET default_value = $3 WHERE tenant_id = $1 AND id = $2",
        vec![
            tenant.into(),
            carrier.id.into(),
            json!(format!("  {child}  ")).into(),
        ],
    ))
    .await?;
    let current = definition_workspace(&owner, tenant, unused.id, None, "en").await?;
    let input = definition_input(
        &current,
        None,
        "en",
        AttributeDefinitionAction::Archive,
        None,
        None,
    )?;
    let refused = ProductCatalogSchemaWritePort::update_attribute_definition(
        &owner,
        definition_context(tenant, actor, &Uuid::new_v4().to_string(), "en"),
        input,
    )
    .await?;
    assert_eq!(refused.refusal, Some(AttributeDefinitionRefusal::InUse));
    assert!(
        refused
            .impact
            .as_ref()
            .unwrap()
            .references
            .iter()
            .any(|reference| reference.resource_id == carrier.id)
    );
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE product_attributes SET default_value = $3 WHERE tenant_id = $1 AND id = $2",
        vec![
            tenant.into(),
            carrier.id.into(),
            json!(blue.to_string()).into(),
        ],
    ))
    .await?;

    // Unknown retained rules and the shared row budget are explicit inspection refusals.
    let current = definition_workspace(&owner, tenant, unused.id, Some(child), "en").await?;
    let input = definition_input(
        &current,
        Some(child),
        "en",
        AttributeDefinitionAction::Archive,
        None,
        None,
    )?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE product_attributes SET validation = $3 WHERE tenant_id = $1 AND id = $2",
        vec![
            tenant.into(),
            carrier.id.into(),
            json!({"options": "unknown historic shape"}).into(),
        ],
    ))
    .await?;
    let refused = ProductCatalogSchemaWritePort::update_attribute_definition(
        &owner,
        definition_context(tenant, actor, &Uuid::new_v4().to_string(), "en"),
        input.clone(),
    )
    .await?;
    assert_eq!(
        refused.refusal,
        Some(AttributeDefinitionRefusal::InspectionLimit)
    );
    assert!(!refused.impact.unwrap().inspection_complete);
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE product_attributes SET validation = '{}'::jsonb WHERE tenant_id = $1 AND id = $2",
        vec![tenant.into(), carrier.id.into()],
    ))
    .await?;
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO product_attributes (id, tenant_id, code, value_type, scope) SELECT gen_random_uuid(), $1, 'limit-' || n::text, 'select', 'both' FROM generate_series(1, 1001) AS seeded(n)",
        vec![tenant.into()],
    )).await?;
    let refused = ProductCatalogSchemaWritePort::update_attribute_definition(
        &owner,
        definition_context(tenant, actor, &Uuid::new_v4().to_string(), "en"),
        input,
    )
    .await?;
    assert_eq!(
        refused.refusal,
        Some(AttributeDefinitionRefusal::InspectionLimit)
    );
    assert!(
        refused
            .impact
            .as_ref()
            .is_some_and(|impact| !impact.inspection_complete && impact.has_more)
    );
    assert!(
        !definition_workspace(&owner, tenant, unused.id, Some(child), "en")
            .await?
            .options
            .unwrap()[0]
            .is_archived
    );

    // Independent connections: a new allowed-option rule cannot race past retirement.
    let race_attribute = owner
        .create_attribute(other_tenant, actor, attribute("race_definition", "both"))
        .await?;
    let race_option = option(
        &owner,
        other_tenant,
        actor,
        race_attribute.id,
        "race_option",
    )
    .await?;
    let race_category = owner
        .create_category(other_tenant, actor, category("race_definition", None))
        .await?;
    let current = definition_workspace(
        &owner,
        other_tenant,
        race_attribute.id,
        Some(race_option),
        "en",
    )
    .await?;
    let input = definition_input(
        &current,
        Some(race_option),
        "en",
        AttributeDefinitionAction::Archive,
        None,
        None,
    )?;
    let first = connect_postgres(url).await?;
    let second = connect_postgres(url).await?;
    let archive_owner = schema_owner(first.clone());
    let bind_owner = schema_owner(second.clone());
    let barrier = tokio::sync::Barrier::new(2);
    let archive = async {
        barrier.wait().await;
        ProductCatalogSchemaWritePort::update_attribute_definition(
            &archive_owner,
            definition_context(other_tenant, actor, &Uuid::new_v4().to_string(), "en"),
            input,
        )
        .await
    };
    let bind = async {
        barrier.wait().await;
        let mut binding = category_binding(
            race_category.id,
            race_attribute.id,
            VariantAxisPolicy::Allowed,
        );
        binding.validation_overrides = json!({"options": [race_option]});
        bind_owner
            .bind_category_attribute(other_tenant, actor, binding)
            .await
    };
    let (archive, bind) = tokio::time::timeout(Duration::from_secs(20), async {
        tokio::join!(archive, bind)
    })
    .await?;
    let archive = archive?;
    assert_ne!(
        archive.applied,
        bind.is_ok(),
        "exactly one conflicting change may commit"
    );
    if !archive.applied {
        assert_eq!(archive.refusal, Some(AttributeDefinitionRefusal::InUse));
    } else {
        assert!(
            matches!(&bind, Err(CommerceError::Validation(detail)) if detail.contains("active values")),
            "the losing binding must be refused for the archived option, not an unrelated failure: {bind:?}"
        );
    }
    let current = definition_workspace(
        &owner,
        other_tenant,
        race_attribute.id,
        Some(race_option),
        "en",
    )
    .await?;
    assert_eq!(current.options.unwrap()[0].is_archived, archive.applied);
    first.close().await?;
    second.close().await?;
    db.close().await?;
    Ok(())
}
