// Comprehensive unit tests for CatalogService
// These tests verify product CRUD, variants, translations,
// pricing, and publishing workflows.

use rust_decimal::Decimal;
use rustok_api::Patch;
use rustok_core::field_schema::FieldType;
use rustok_product::CatalogService;
use rustok_product::CommerceError;
use rustok_product::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput,
    UpdateProductInput, VariantAxisInput, VariantAxisValueInput,
};
use rustok_product::entities;
use rustok_product::entities::product::ProductStatus;
use rustok_product::services::{
    CatalogCategoryKind, CategoryTranslationInput, CreateCatalogCategoryInput,
    ProductAttributeValuePatch, ProductAttributeValuePatchValue, ProductCatalogSchemaService,
};
use rustok_test_utils::{db::setup_test_db, helpers::unique_slug, mock_transactional_event_bus};
use sea_orm::DatabaseConnection;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use std::str::FromStr;
use uuid::Uuid;

mod support;

mod product_field_definitions_storage {
    rustok_core::define_field_definitions_entity!("product_field_definitions");
}

mod attached_values_storage {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "flex_attached_localized_values")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub entity_type: String,
        pub entity_id: Uuid,
        pub field_key: String,
        pub locale: String,
        pub value: Json,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup() -> (DatabaseConnection, CatalogService) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let event_bus = mock_transactional_event_bus();
    let service = CatalogService::new(db.clone(), event_bus);
    (db, service)
}

async fn seed_test_variant_axis(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    code: &str,
    options: &[&str],
) -> (Uuid, Vec<Uuid>) {
    let attribute_id = Uuid::new_v4();
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        "INSERT INTO product_attributes (id, tenant_id, code, value_type, scope, created_at, updated_at)
         VALUES (?, ?, ?, 'select', 'variant', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            attribute_id.into(),
            tenant_id.into(),
            code.into(),
        ],
    ))
    .await
    .expect("failed to insert test attribute");

    let mut option_ids = Vec::new();
    for (position, opt_code) in options.iter().enumerate() {
        let opt_id = Uuid::new_v4();
        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO product_attribute_options (id, tenant_id, attribute_id, code, position, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                opt_id.into(),
                tenant_id.into(),
                attribute_id.into(),
                (*opt_code).into(),
                (position as i32).into(),
            ],
        ))
        .await
        .expect("failed to insert test attribute option");
        option_ids.push(opt_id);
    }

    (attribute_id, option_ids)
}

async fn seed_test_structural_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    code: &str,
) -> Uuid {
    let event_bus = mock_transactional_event_bus();
    let schema_service = ProductCatalogSchemaService::new(db.clone(), event_bus);
    let category = schema_service
        .create_category(
            tenant_id,
            Uuid::new_v4(),
            CreateCatalogCategoryInput {
                code: code.to_string(),
                slug: code.to_string(),
                kind: CatalogCategoryKind::Structural,
                parent_id: None,
                position: 0,
                rule_config: serde_json::json!({}),
                metadata: serde_json::json!({}),
                translations: vec![CategoryTranslationInput {
                    locale: "en".to_string(),
                    name: code.to_string(),
                    description: None,
                    meta_title: None,
                    meta_description: None,
                }],
            },
        )
        .await
        .expect("failed to create test structural category");
    category.id
}

async fn bind_test_category_attribute(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    attribute_id: Uuid,
) {
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        "INSERT INTO category_attributes (id, tenant_id, category_id, attribute_id, binding_kind, is_disabled, variant_axis_policy)
         VALUES (?, ?, ?, ?, 'addition', 0, 'allowed')",
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            category_id.into(),
            attribute_id.into(),
        ],
    ))
    .await
    .expect("failed to bind test category attribute");
}

fn create_test_product_input() -> CreateProductInput {
    CreateProductInput {
        attribute_values: Default::default(),
        translations: vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Test Product".to_string(),
            description: Some("A great test product".to_string()),
            handle: Some(unique_slug("test-product")),
            meta_title: None,
            meta_description: None,
        }],
        variant_axes: vec![],
        variants: vec![CreateVariantInput {
            sku: Some(format!(
                "SKU-{}",
                Uuid::new_v4().to_string().split('-').next().unwrap()
            )),
            barcode: None,
            shipping_profile_slug: None,
            axis_values: vec![],
            prices: vec![PriceInput {
                currency_code: "USD".to_string(),
                channel_id: None,
                channel_slug: None,
                amount: Decimal::from_str("99.99").unwrap(),
                compare_at_amount: Some(Decimal::from_str("149.99").unwrap()),
            }],
            inventory_quantity: 0,
            inventory_policy: "deny".to_string(),
            weight: Some(Decimal::from_str("1.5").unwrap()),
            weight_unit: Some("kg".to_string()),
        }],
        seller_id: None,
        vendor: Some("Test Vendor".to_string()),
        product_type: Some("Physical".to_string()),
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: vec![],
        publish: false,
        metadata: serde_json::json!({}),
    }
}

async fn insert_product_field_definition(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    field_key: &str,
    is_localized: bool,
    is_required: bool,
    default_value: Option<serde_json::Value>,
    position: i32,
) {
    let field_type = serde_json::to_value(FieldType::Text)
        .expect("field type should serialize")
        .as_str()
        .expect("field type should be string")
        .to_string();

    product_field_definitions_storage::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        field_key: Set(field_key.to_string()),
        field_type: Set(field_type),
        label: Set(serde_json::json!({ "en": field_key })),
        description: Set(None),
        is_localized: Set(is_localized),
        is_required: Set(is_required),
        default_value: Set(default_value),
        validation: Set(None),
        position: Set(position),
        is_active: Set(true),
        created_at: sea_orm::ActiveValue::NotSet,
        updated_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(db)
    .await
    .expect("product field definition should be created");
}

// =============================================================================
// Basic CRUD Tests
// =============================================================================

#[tokio::test]
async fn test_create_product_success() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let input = create_test_product_input();

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let product = result.unwrap();
    assert_eq!(product.translations.len(), 1);
    assert_eq!(product.translations[0].title, "Test Product");
    assert_eq!(product.variants.len(), 1);
    assert_eq!(product.status, ProductStatus::Draft);
}

#[tokio::test]
async fn test_shipping_profile_slug_round_trips_through_catalog_service() {
    let (_db, service) = setup().await;
    let service =
        service.with_shipping_profile_port(std::sync::Arc::new(ShippingAdmission::default()));
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let mut input = create_test_product_input();
    input.shipping_profile_slug = Some(" Bulky ".to_string());

    let created = service
        .create_product(tenant_id, actor_id, input)
        .await
        .expect("product should be created");
    assert_eq!(created.shipping_profile_slug.as_deref(), Some("bulky"));
    assert_eq!(created.metadata["shipping_profile"]["slug"], "bulky");

    let updated = service
        .update_product(
            tenant_id,
            actor_id,
            created.id,
            UpdateProductInput {
                attribute_values: Default::default(),
                translations: None,
                seller_id: Patch::Keep,
                vendor: Patch::Keep,
                product_type: Patch::Keep,
                shipping_profile_slug: Patch::Set("Cold-Chain".to_string()),
                primary_category_id: Patch::Keep,
                tags: None,
                status: None,
                metadata: None,
                expected_revision: None,
            },
        )
        .await
        .expect("product should be updated");
    assert_eq!(updated.shipping_profile_slug.as_deref(), Some("cold-chain"));
    assert_eq!(updated.metadata["shipping_profile"]["slug"], "cold-chain");

    let fetched = service
        .get_product(tenant_id, created.id)
        .await
        .expect("product should load");
    assert_eq!(fetched.shipping_profile_slug.as_deref(), Some("cold-chain"));
}

#[tokio::test]
async fn test_create_product_requires_translations() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.translations = vec![];

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(msg) => {
            assert!(msg.contains("translation"));
        }
        _ => panic!("Expected validation error"),
    }
}

#[tokio::test]
async fn test_create_product_requires_variants() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.variants = vec![];

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::NoVariants => {}
        _ => panic!("Expected NoVariants error"),
    }
}

#[tokio::test]
async fn test_get_product_success() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let input = create_test_product_input();
    let created = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();

    let result = service.get_product(tenant_id, created.id).await;

    assert!(result.is_ok());
    let product = result.unwrap();
    assert_eq!(product.id, created.id);
    assert_eq!(product.translations[0].title, "Test Product");
}

#[tokio::test]
async fn test_get_nonexistent_product() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let fake_id = Uuid::new_v4();

    let result = service.get_product(tenant_id, fake_id).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::ProductNotFound(_) => {}
        _ => panic!("Expected ProductNotFound error"),
    }
}

#[tokio::test]
async fn test_update_product_success() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let input = create_test_product_input();
    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();

    let update_input = UpdateProductInput {
        attribute_values: Default::default(),
        translations: Some(vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Updated Product".to_string(),
            description: Some("Updated description".to_string()),
            handle: None,
            meta_title: None,
            meta_description: None,
        }]),
        seller_id: Patch::Keep,
        vendor: Patch::Set("Updated Vendor".to_string()),
        product_type: Patch::Set("Digital".to_string()),
        shipping_profile_slug: Patch::Keep,
        primary_category_id: Patch::Keep,
        tags: None,
        status: Some(ProductStatus::Active),
        metadata: None,
        expected_revision: None,
    };

    let result = service
        .update_product(tenant_id, actor_id, product.id, update_input)
        .await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.translations[0].title, "Updated Product");
    assert_eq!(updated.vendor, Some("Updated Vendor".to_string()));
    assert_eq!(updated.product_type, Some("Digital".to_string()));
    assert_eq!(updated.status, ProductStatus::Active);
}

#[tokio::test]
async fn test_delete_product_success() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let input = create_test_product_input();
    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();

    let result = service
        .delete_product(tenant_id, actor_id, product.id)
        .await;
    assert!(result.is_ok());

    let get_result = service.get_product(tenant_id, product.id).await;
    assert!(get_result.is_err());
}

#[tokio::test]
async fn test_create_product_applies_custom_field_defaults_and_splits_localized_values() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    insert_product_field_definition(&db, tenant_id, "marketing_copy", true, false, None, 0).await;
    insert_product_field_definition(
        &db,
        tenant_id,
        "badge",
        false,
        false,
        Some(serde_json::json!("featured")),
        1,
    )
    .await;

    let mut input = create_test_product_input();
    input.metadata = serde_json::json!({
        "marketing_copy": "English promo",
        "unknown_custom": "keep-me"
    });

    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .expect("product should be created");

    assert_eq!(product.metadata["marketing_copy"], "English promo");
    assert_eq!(product.metadata["badge"], "featured");
    assert_eq!(product.metadata["unknown_custom"], "keep-me");

    let stored_product = entities::product::Entity::find_by_id(product.id)
        .one(&db)
        .await
        .expect("product query should succeed")
        .expect("stored product should exist");
    assert_eq!(
        stored_product.metadata,
        serde_json::json!({
            "unknown_custom": "keep-me",
            "badge": "featured"
        })
    );

    let localized_rows = attached_values_storage::Entity::find()
        .filter(attached_values_storage::Column::TenantId.eq(tenant_id))
        .filter(attached_values_storage::Column::EntityType.eq("product"))
        .filter(attached_values_storage::Column::EntityId.eq(product.id))
        .all(&db)
        .await
        .expect("localized rows query should succeed");
    assert_eq!(localized_rows.len(), 1);
    assert_eq!(localized_rows[0].field_key, "marketing_copy");
    assert_eq!(localized_rows[0].locale, "en");
    assert_eq!(localized_rows[0].value, serde_json::json!("English promo"));
}

#[tokio::test]
async fn test_create_product_rejects_missing_required_localized_custom_field() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    insert_product_field_definition(&db, tenant_id, "marketing_copy", true, true, None, 0).await;

    let input = create_test_product_input();
    let error = service
        .create_product(tenant_id, actor_id, input)
        .await
        .expect_err("product creation should fail without required localized custom field");

    match error {
        CommerceError::Validation(message) => {
            assert!(
                message.contains("Custom field validation failed"),
                "unexpected error: {message}"
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_product_locale_fallback_resolves_localized_flex_metadata_from_attached_values() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    insert_product_field_definition(&db, tenant_id, "marketing_copy", true, false, None, 0).await;
    insert_product_field_definition(&db, tenant_id, "badge", false, false, None, 1).await;

    let mut input = create_test_product_input();
    input.metadata = serde_json::json!({
        "marketing_copy": "English promo",
        "badge": "featured"
    });
    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .expect("product should be created");

    service
        .update_product(
            tenant_id,
            actor_id,
            product.id,
            UpdateProductInput {
                attribute_values: Default::default(),
                translations: Some(vec![
                    ProductTranslationInput {
                        locale: "ru".to_string(),
                        title: "Тестовый товар".to_string(),
                        description: Some("Русская версия".to_string()),
                        handle: Some(unique_slug("test-product-ru-flex")),
                        meta_title: None,
                        meta_description: None,
                    },
                    ProductTranslationInput {
                        locale: "en".to_string(),
                        title: "Test Product".to_string(),
                        description: Some("A great test product".to_string()),
                        handle: Some(unique_slug("test-product-en-updated")),
                        meta_title: None,
                        meta_description: None,
                    },
                ]),
                seller_id: Patch::Keep,
                vendor: Patch::Keep,
                product_type: Patch::Keep,
                shipping_profile_slug: Patch::Keep,
                primary_category_id: Patch::Keep,
                tags: None,
                status: None,
                metadata: Some(serde_json::json!({
                    "marketing_copy": "Русский промо текст",
                    "badge": "featured"
                })),
                expected_revision: None,
            },
        )
        .await
        .expect("product update should persist localized custom field");

    let en_product = service
        .get_product_with_locale_fallback(tenant_id, product.id, "en", Some("ru"))
        .await
        .expect("english product should load");
    assert_eq!(en_product.metadata["marketing_copy"], "English promo");
    assert_eq!(en_product.metadata["badge"], "featured");

    let fallback_product = service
        .get_product_with_locale_fallback(tenant_id, product.id, "fr-FR", Some("ru"))
        .await
        .expect("fallback product should load");
    assert_eq!(
        fallback_product.metadata["marketing_copy"],
        "Русский промо текст"
    );
    assert_eq!(fallback_product.metadata["badge"], "featured");

    let stored_product = entities::product::Entity::find_by_id(product.id)
        .one(&db)
        .await
        .expect("product query should succeed")
        .expect("stored product should exist");
    assert_eq!(
        stored_product.metadata,
        serde_json::json!({ "badge": "featured" })
    );

    let localized_rows = attached_values_storage::Entity::find()
        .filter(attached_values_storage::Column::TenantId.eq(tenant_id))
        .filter(attached_values_storage::Column::EntityType.eq("product"))
        .filter(attached_values_storage::Column::EntityId.eq(product.id))
        .all(&db)
        .await
        .expect("localized rows query should succeed");
    assert_eq!(localized_rows.len(), 2);
    assert!(localized_rows.iter().any(|row| {
        row.field_key == "marketing_copy"
            && row.locale == "en"
            && row.value == serde_json::json!("English promo")
    }));
    assert!(localized_rows.iter().any(|row| {
        row.field_key == "marketing_copy"
            && row.locale == "ru"
            && row.value == serde_json::json!("Русский промо текст")
    }));
}

// =============================================================================
// Multi-Language Translation Tests
// =============================================================================

#[tokio::test]
async fn test_create_product_with_multiple_translations() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.translations.push(ProductTranslationInput {
        locale: "ru".to_string(),
        title: "Р СћР ВµРЎРѓРЎвЂљР С•Р Р†РЎвЂ№Р в„– Р С—РЎР‚Р С•Р Т‘РЎС“Р С”РЎвЂљ".to_string(),
        description: Some("Р С›РЎвЂљР В»Р С‘РЎвЂЎР Р…РЎвЂ№Р в„– РЎвЂљР ВµРЎРѓРЎвЂљР С•Р Р†РЎвЂ№Р в„– Р С—РЎР‚Р С•Р Т‘РЎС“Р С”РЎвЂљ".to_string()),
        handle: Some(unique_slug("test-product-ru")),
        meta_title: None,
        meta_description: None,
    });
    input.translations.push(ProductTranslationInput {
        locale: "de".to_string(),
        title: "Testprodukt".to_string(),
        description: Some("Ein groР“Сџartiges Testprodukt".to_string()),
        handle: Some(unique_slug("test-product-de")),
        meta_title: None,
        meta_description: None,
    });

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let product = result.unwrap();
    assert_eq!(product.translations.len(), 3);

    let en_trans = product.translations.iter().find(|t| t.locale == "en");
    let ru_trans = product.translations.iter().find(|t| t.locale == "ru");
    let de_trans = product.translations.iter().find(|t| t.locale == "de");

    assert!(en_trans.is_some());
    assert!(ru_trans.is_some());
    assert!(de_trans.is_some());
    assert_eq!(en_trans.unwrap().title, "Test Product");
    assert_eq!(
        ru_trans.unwrap().title,
        "Р СћР ВµРЎРѓРЎвЂљР С•Р Р†РЎвЂ№Р в„– Р С—РЎР‚Р С•Р Т‘РЎС“Р С”РЎвЂљ"
    );
    assert_eq!(de_trans.unwrap().title, "Testprodukt");
}

// =============================================================================
// Product Variant Tests
// =============================================================================

#[tokio::test]
async fn test_create_product_populates_variant_translation_groups() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.translations.push(ProductTranslationInput {
        locale: "ru".to_string(),
        title: "Тестовый продукт".to_string(),
        description: Some("Русская локализация".to_string()),
        handle: Some(unique_slug("test-product-ru")),
        meta_title: None,
        meta_description: None,
    });

    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .expect("product with translation groups should be created");

    assert_eq!(product.translations.len(), 2);
    assert_eq!(product.variants.len(), 1);
    assert_eq!(product.variants[0].translations.len(), 2);
    assert!(
        product.variants[0]
            .translations
            .iter()
            .any(|item| item.locale == "en" && item.title.as_deref() == Some("Default"))
    );
    assert!(
        product.variants[0]
            .translations
            .iter()
            .any(|item| item.locale == "ru" && item.title.as_deref() == Some("Default"))
    );
}

#[tokio::test]
async fn test_get_product_reads_image_translation_groups() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let product = service
        .create_product(tenant_id, actor_id, create_test_product_input())
        .await
        .expect("product should be created");

    let image_id = Uuid::new_v4();
    let media_id = Uuid::new_v4();
    entities::product_image::ActiveModel {
        id: sea_orm::Set(image_id),
        product_id: sea_orm::Set(product.id),
        media_id: sea_orm::Set(media_id),
        position: sea_orm::Set(0),
    }
    .insert(&db)
    .await
    .expect("image should be inserted");

    entities::product_image_translation::ActiveModel {
        id: sea_orm::Set(Uuid::new_v4()),
        image_id: sea_orm::Set(image_id),
        locale: sea_orm::Set("en".to_string()),
        alt_text: sea_orm::Set(Some("Front image".to_string())),
    }
    .insert(&db)
    .await
    .expect("english image translation should be inserted");

    entities::product_image_translation::ActiveModel {
        id: sea_orm::Set(Uuid::new_v4()),
        image_id: sea_orm::Set(image_id),
        locale: sea_orm::Set("ru".to_string()),
        alt_text: sea_orm::Set(Some(
            "Р С›РЎРѓР Р…Р С•Р Р†Р Р…Р С•Р Вµ Р С‘Р В·Р С•Р В±РЎР‚Р В°Р В¶Р ВµР Р…Р С‘Р Вµ"
                .to_string(),
        )),
    }
    .insert(&db)
    .await
    .expect("russian image translation should be inserted");

    let fetched = service
        .get_product(tenant_id, product.id)
        .await
        .expect("product should be fetched");

    assert_eq!(fetched.images.len(), 1);
    assert_eq!(fetched.images[0].translations.len(), 2);
    assert!(
        fetched.images[0]
            .translations
            .iter()
            .any(|item| item.locale == "en" && item.alt_text.as_deref() == Some("Front image"))
    );
    assert!(fetched.images[0]
        .translations
        .iter()
        .any(|item| item.locale == "ru"
            && item.alt_text.as_deref()
                == Some(
                    "Р С›РЎРѓР Р…Р С•Р Р†Р Р…Р С•Р Вµ Р С‘Р В·Р С•Р В±РЎР‚Р В°Р В¶Р ВµР Р…Р С‘Р Вµ"
                )));
}

#[tokio::test]
async fn test_create_product_with_multiple_variants() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let category_id = seed_test_structural_category(&db, tenant_id, "size-apparel").await;
    let (attr_id, opt_ids) =
        seed_test_variant_axis(&db, tenant_id, "size", &["medium", "small", "large"]).await;
    bind_test_category_attribute(&db, tenant_id, category_id, attr_id).await;

    let mut input = create_test_product_input();
    input.primary_category_id = Some(category_id);
    input.variant_axes = vec![VariantAxisInput {
        attribute_id: attr_id,
        position: 0,
        allowed_option_ids: opt_ids.clone(),
    }];
    input.variants[0].axis_values = vec![VariantAxisValueInput {
        attribute_id: attr_id,
        option_id: opt_ids[0],
    }];
    input.variants.push(CreateVariantInput {
        sku: Some(format!(
            "SKU-{}",
            Uuid::new_v4().to_string().split('-').next().unwrap()
        )),
        barcode: None,
        shipping_profile_slug: None,
        axis_values: vec![VariantAxisValueInput {
            attribute_id: attr_id,
            option_id: opt_ids[1],
        }],
        prices: vec![PriceInput {
            currency_code: "USD".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: Decimal::from_str("79.99").unwrap(),
            compare_at_amount: None,
        }],
        inventory_quantity: 0,
        inventory_policy: "deny".to_string(),
        weight: Some(Decimal::from_str("1.0").unwrap()),
        weight_unit: Some("kg".to_string()),
    });
    input.variants.push(CreateVariantInput {
        sku: Some(format!(
            "SKU-{}",
            Uuid::new_v4().to_string().split('-').next().unwrap()
        )),
        barcode: None,
        shipping_profile_slug: None,
        axis_values: vec![VariantAxisValueInput {
            attribute_id: attr_id,
            option_id: opt_ids[2],
        }],
        prices: vec![PriceInput {
            currency_code: "USD".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: Decimal::from_str("119.99").unwrap(),
            compare_at_amount: Some(Decimal::from_str("169.99").unwrap()),
        }],
        inventory_quantity: 0,
        inventory_policy: "deny".to_string(),
        weight: Some(Decimal::from_str("2.0").unwrap()),
        weight_unit: Some("kg".to_string()),
    });

    let result = service.create_product(tenant_id, actor_id, input).await;

    let product = result.expect("create_product failed");
    assert_eq!(product.variants.len(), 3);

    let small = product.variants.iter().find(|v| {
        v.prices
            .iter()
            .any(|p| p.amount == Decimal::from_str("79.99").unwrap())
    });
    let large = product.variants.iter().find(|v| {
        v.prices
            .iter()
            .any(|p| p.amount == Decimal::from_str("119.99").unwrap())
    });

    assert!(small.is_some());
    assert!(large.is_some());
    assert_eq!(
        small.unwrap().prices[0].amount,
        Decimal::from_str("79.99").unwrap()
    );
    assert_eq!(
        large.unwrap().prices[0].amount,
        Decimal::from_str("119.99").unwrap()
    );
}

#[tokio::test]
async fn test_variant_pricing() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let input = create_test_product_input();

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let product = result.unwrap();
    let variant = &product.variants[0];

    assert!(!variant.prices.is_empty());
    let price = &variant.prices[0];
    assert_eq!(price.amount, Decimal::from_str("99.99").unwrap());
    assert_eq!(
        price.compare_at_amount,
        Some(Decimal::from_str("149.99").unwrap())
    );

    let discount = price.compare_at_amount.unwrap() - price.amount;
    let discount_percent =
        (discount / price.compare_at_amount.unwrap()) * Decimal::from_str("100.0").unwrap();
    let diff = (discount_percent - Decimal::from_str("33.34").unwrap()).abs();
    assert!(diff < Decimal::from_str("0.1").unwrap());
}

#[tokio::test]
async fn test_variant_shipping_properties() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.variants[0].weight = Some(Decimal::from_str("2.5").unwrap());
    input.variants[0].weight_unit = Some("kg".to_string());

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let product = result.unwrap();
    let variant = &product.variants[0];

    assert_eq!(variant.weight, Some(Decimal::from_str("2.5").unwrap()));
    assert_eq!(variant.weight_unit, Some("kg".to_string()));
}

#[tokio::test]
async fn test_variant_with_barcode() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.variants[0].barcode = Some("1234567890123".to_string());

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let product = result.unwrap();
    let variant = &product.variants[0];

    assert_eq!(variant.barcode, Some("1234567890123".to_string()));
}

// =============================================================================
// Publishing & Status Tests
// =============================================================================

#[tokio::test]
async fn test_create_product_with_publish() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.publish = true;

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let product = result.unwrap();
    assert_eq!(product.status, ProductStatus::Active);
    assert!(product.published_at.is_some());
}

#[tokio::test]
async fn test_publish_product() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.publish = false;
    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();

    assert_eq!(product.status, ProductStatus::Draft);
    assert!(product.published_at.is_none());

    let result = service
        .publish_product(tenant_id, actor_id, product.id)
        .await;

    assert!(result.is_ok());
    let published = result.unwrap();
    assert_eq!(published.status, ProductStatus::Active);
    assert!(published.published_at.is_some());
}

#[tokio::test]
async fn test_unpublish_product() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.publish = true;
    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();

    assert_eq!(product.status, ProductStatus::Active);

    let result = service
        .unpublish_product(tenant_id, actor_id, product.id)
        .await;

    assert!(result.is_ok());
    let unpublished = result.unwrap();
    assert_eq!(unpublished.status, ProductStatus::Draft);
}

// =============================================================================
// Metadata Tests
// =============================================================================

#[tokio::test]
async fn test_product_with_metadata() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.tags = vec!["new".to_string(), "sale".to_string(), "popular".to_string()];
    input.metadata = serde_json::json!({
        "featured": true,
        "color": "blue",
        "size_chart": "standard"
    });

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let product = result.unwrap();
    assert_eq!(product.metadata["featured"], true);
    assert!(product.metadata.get("tags").is_none());
    let mut tags = product.tags.clone();
    tags.sort();
    assert_eq!(tags, vec!["new", "popular", "sale"]);
    assert_eq!(product.metadata["color"], "blue");
    assert_eq!(product.metadata["size_chart"], "standard");
}

#[tokio::test]
async fn test_update_product_metadata() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let input = create_test_product_input();
    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();

    let update_input = UpdateProductInput {
        attribute_values: Default::default(),
        translations: None,
        seller_id: Patch::Keep,
        vendor: Patch::Keep,
        product_type: Patch::Keep,
        shipping_profile_slug: Patch::Keep,
        primary_category_id: Patch::Keep,
        tags: None,
        status: None,
        metadata: Some(serde_json::json!({
            "featured": true,
            "priority": "high",
            "badge": "bestseller"
        })),
        expected_revision: None,
    };

    let result = service
        .update_product(tenant_id, actor_id, product.id, update_input)
        .await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.metadata["featured"], true);
    assert_eq!(updated.metadata["priority"], "high");
    assert_eq!(updated.metadata["badge"], "bestseller");
}

// =============================================================================
// Vendor & Product Type Tests
// =============================================================================

#[tokio::test]
async fn test_product_with_vendor_and_type() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.vendor = Some("Acme Corp".to_string());
    input.product_type = Some("Electronics".to_string());

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let product = result.unwrap();
    assert_eq!(product.vendor, Some("Acme Corp".to_string()));
    assert_eq!(product.product_type, Some("Electronics".to_string()));
}

#[tokio::test]
async fn test_update_vendor() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let input = create_test_product_input();
    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();

    let update_input = UpdateProductInput {
        attribute_values: Default::default(),
        translations: None,
        seller_id: Patch::Keep,
        vendor: Patch::Set("New Vendor Inc".to_string()),
        product_type: Patch::Keep,
        shipping_profile_slug: Patch::Keep,
        primary_category_id: Patch::Keep,
        tags: None,
        status: None,
        metadata: None,
        expected_revision: None,
    };

    let result = service
        .update_product(tenant_id, actor_id, product.id, update_input)
        .await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.vendor, Some("New Vendor Inc".to_string()));
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[tokio::test]
async fn test_update_nonexistent_product() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let fake_id = Uuid::new_v4();

    let update_input = UpdateProductInput {
        attribute_values: Default::default(),
        translations: Some(vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Updated".to_string(),
            description: None,
            handle: None,
            meta_title: None,
            meta_description: None,
        }]),
        seller_id: Patch::Keep,
        vendor: Patch::Keep,
        product_type: Patch::Keep,
        shipping_profile_slug: Patch::Keep,
        primary_category_id: Patch::Keep,
        tags: None,
        status: None,
        metadata: None,
        expected_revision: None,
    };

    let result = service
        .update_product(tenant_id, actor_id, fake_id, update_input)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::ProductNotFound(_) => {}
        _ => panic!("Expected ProductNotFound error"),
    }
}

#[tokio::test]
async fn test_delete_nonexistent_product() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let fake_id = Uuid::new_v4();

    let result = service.delete_product(tenant_id, actor_id, fake_id).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_publish_nonexistent_product() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let fake_id = Uuid::new_v4();

    let result = service.publish_product(tenant_id, actor_id, fake_id).await;

    assert!(result.is_err());
}

// =============================================================================
// SKU & Handle Uniqueness Tests
// =============================================================================

#[tokio::test]
async fn test_unique_skus_per_product() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let input = create_test_product_input();
    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();

    let skus: Vec<Option<String>> = product.variants.iter().map(|v| v.sku.clone()).collect();
    let unique_skus: std::collections::HashSet<_> = skus.iter().collect();

    assert_eq!(skus.len(), unique_skus.len(), "All SKUs should be unique");
}

// =============================================================================
// Additional Edge Case Tests
// =============================================================================

#[tokio::test]
async fn test_product_with_empty_vendor() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.vendor = None;

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let product = result.unwrap();
    assert_eq!(product.vendor, None);
}

#[tokio::test]
async fn test_variant_digital_product() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.product_type = Some("Digital".to_string());
    input.variants[0].weight = None;
    input.variants[0].weight_unit = None;

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let product = result.unwrap();
    assert_eq!(product.product_type, Some("Digital".to_string()));
    assert_eq!(product.variants[0].weight, None);
}

#[tokio::test]
async fn test_create_archived_product() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let input = create_test_product_input();
    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();

    let update_input = UpdateProductInput {
        attribute_values: Default::default(),
        translations: None,
        seller_id: Patch::Keep,
        vendor: Patch::Keep,
        product_type: Patch::Keep,
        shipping_profile_slug: Patch::Keep,
        primary_category_id: Patch::Keep,
        tags: None,
        status: Some(ProductStatus::Archived),
        metadata: None,
        expected_revision: None,
    };

    let result = service
        .update_product(tenant_id, actor_id, product.id, update_input)
        .await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.status, ProductStatus::Archived);
}

#[tokio::test]
async fn test_variant_price_in_prices_vec() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.variants[0].prices = vec![PriceInput {
        currency_code: "USD".to_string(),
        channel_id: None,
        channel_slug: None,
        amount: Decimal::from_str("100.00").unwrap(),
        compare_at_amount: None,
    }];

    let result = service.create_product(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let product = result.unwrap();
    let variant = &product.variants[0];

    assert!(!variant.prices.is_empty());
    assert_eq!(
        variant.prices[0].amount,
        Decimal::from_str("100.00").unwrap()
    );
}

#[tokio::test]
async fn test_multiple_variants_different_prices() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let category_id = seed_test_structural_category(&db, tenant_id, "edition-category").await;
    let (attr_id, opt_ids) =
        seed_test_variant_axis(&db, tenant_id, "edition", &["standard", "deluxe"]).await;
    bind_test_category_attribute(&db, tenant_id, category_id, attr_id).await;

    let mut input = create_test_product_input();
    input.primary_category_id = Some(category_id);
    input.variant_axes = vec![VariantAxisInput {
        attribute_id: attr_id,
        position: 0,
        allowed_option_ids: opt_ids.clone(),
    }];
    input.variants[0].axis_values = vec![VariantAxisValueInput {
        attribute_id: attr_id,
        option_id: opt_ids[0],
    }];
    input.variants[0].prices = vec![PriceInput {
        currency_code: "USD".to_string(),
        channel_id: None,
        channel_slug: None,
        amount: Decimal::from_str("50.00").unwrap(),
        compare_at_amount: None,
    }];
    input.variants.push(CreateVariantInput {
        sku: Some(format!(
            "SKU-{}",
            Uuid::new_v4().to_string().split('-').next().unwrap()
        )),
        barcode: None,
        shipping_profile_slug: None,
        axis_values: vec![VariantAxisValueInput {
            attribute_id: attr_id,
            option_id: opt_ids[1],
        }],
        prices: vec![PriceInput {
            currency_code: "USD".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: Decimal::from_str("150.00").unwrap(),
            compare_at_amount: None,
        }],
        inventory_quantity: 0,
        inventory_policy: "deny".to_string(),
        weight: Some(Decimal::from_str("2.0").unwrap()),
        weight_unit: Some("kg".to_string()),
    });

    let result = service.create_product(tenant_id, actor_id, input).await;

    let product = result.expect("create_product failed");
    assert_eq!(product.variants.len(), 2);

    // Find variants by their price amount
    let cheap = product
        .variants
        .iter()
        .find(|v| v.prices[0].amount == Decimal::from_str("50.00").unwrap());
    let premium = product
        .variants
        .iter()
        .find(|v| v.prices[0].amount == Decimal::from_str("150.00").unwrap());

    assert!(cheap.is_some());
    assert!(premium.is_some());
}

#[tokio::test]
async fn update_product_refuses_a_stale_predecessor_revision() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let created = service
        .create_product(tenant_id, actor_id, create_test_product_input())
        .await
        .expect("product should be created");
    assert_eq!(created.revision, 1);

    let first = service
        .update_product(
            tenant_id,
            actor_id,
            created.id,
            UpdateProductInput {
                vendor: Patch::Set("First writer".to_string()),
                expected_revision: Some(1),
                ..Default::default()
            },
        )
        .await
        .expect("first writer wins");
    assert_eq!(first.revision, 2);
    assert_eq!(first.vendor.as_deref(), Some("First writer"));

    let stale = service
        .update_product(
            tenant_id,
            actor_id,
            created.id,
            UpdateProductInput {
                vendor: Patch::Set("Second writer".to_string()),
                expected_revision: Some(1),
                ..Default::default()
            },
        )
        .await
        .expect_err("a stale predecessor revision must be refused");
    assert!(matches!(stale, CommerceError::Validation(_)));
    assert!(stale.to_string().contains("product revision conflict"));

    let reloaded = service
        .get_product(tenant_id, created.id)
        .await
        .expect("product should load");
    assert_eq!(reloaded.revision, 2);
    assert_eq!(reloaded.vendor.as_deref(), Some("First writer"));

    let retried = service
        .update_product(
            tenant_id,
            actor_id,
            created.id,
            UpdateProductInput {
                vendor: Patch::Set("Second writer".to_string()),
                expected_revision: Some(reloaded.revision),
                ..Default::default()
            },
        )
        .await
        .expect("the reloaded revision is accepted");
    assert_eq!(retried.revision, 3);
    assert_eq!(retried.vendor.as_deref(), Some("Second writer"));
}

#[tokio::test]
async fn lifecycle_transitions_bump_the_revision_and_clear_patches_drop_shadows() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let created = service
        .create_product(tenant_id, actor_id, create_test_product_input())
        .await
        .expect("product should be created");

    let published = service
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("publish should succeed");
    assert_eq!(published.revision, created.revision + 1);

    let unpublished = service
        .unpublish_product(tenant_id, actor_id, created.id)
        .await
        .expect("unpublish should succeed");
    assert_eq!(unpublished.revision, published.revision + 1);

    let bound = service
        .update_product(
            tenant_id,
            actor_id,
            created.id,
            UpdateProductInput {
                shipping_profile_slug: Patch::Set("standard".to_string()),
                expected_revision: Some(unpublished.revision),
                ..Default::default()
            },
        )
        .await
        .expect("binding a shipping profile should succeed");
    assert_eq!(bound.shipping_profile_slug.as_deref(), Some("standard"));
    assert_eq!(bound.metadata["shipping_profile"]["slug"], "standard");

    let cleared = service
        .update_product(
            tenant_id,
            actor_id,
            created.id,
            UpdateProductInput {
                shipping_profile_slug: Patch::Clear,
                expected_revision: Some(bound.revision),
                ..Default::default()
            },
        )
        .await
        .expect("clearing a shipping profile should succeed");
    assert_eq!(cleared.shipping_profile_slug, None);
    assert!(cleared.metadata.get("shipping_profile").is_none());
    assert!(cleared.metadata.get("shipping_profile_slug").is_none());
}

#[tokio::test]
async fn repeated_lifecycle_commands_preserve_revision_and_timestamps() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let created = service
        .create_product(tenant_id, actor_id, create_test_product_input())
        .await
        .unwrap();

    let draft = service
        .unpublish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    assert_eq!(draft.revision, created.revision);
    assert_eq!(draft.updated_at, created.updated_at);

    let published = service
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    let retried = service
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    assert_eq!(retried.revision, published.revision);
    assert_eq!(retried.published_at, published.published_at);
    assert_eq!(retried.updated_at, published.updated_at);

    let archived = service
        .archive_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    let retried = service
        .archive_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    assert_eq!(retried.revision, archived.revision);
    assert_eq!(retried.updated_at, archived.updated_at);
    assert_eq!(retried.status, ProductStatus::Archived);
}

#[tokio::test]
async fn category_change_validates_candidate_before_publishing_or_remaining_active() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let category_id = seed_test_structural_category(&db, tenant_id, "required-category").await;
    let (attribute_id, _) = seed_test_variant_axis(&db, tenant_id, "material", &[]).await;
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        "UPDATE product_attributes SET scope = 'product', value_type = 'text' WHERE id = ?",
        vec![attribute_id.into()],
    ))
    .await
    .unwrap();
    bind_test_category_attribute(&db, tenant_id, category_id, attribute_id).await;
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        "UPDATE category_attributes SET is_required = 1 WHERE category_id = ? AND attribute_id = ?",
        vec![category_id.into(), attribute_id.into()],
    ))
    .await
    .unwrap();

    for initially_active in [false, true] {
        let mut input = create_test_product_input();
        input.publish = initially_active;
        let created = service
            .create_product(tenant_id, actor_id, input)
            .await
            .unwrap();
        let result = service
            .update_product(
                tenant_id,
                actor_id,
                created.id,
                UpdateProductInput {
                    primary_category_id: Patch::Set(category_id),
                    status: Some(ProductStatus::Active),
                    expected_revision: Some(created.revision),
                    ..Default::default()
                },
            )
            .await;
        assert!(matches!(result, Err(CommerceError::Validation(_))));
        let unchanged = service.get_product(tenant_id, created.id).await.unwrap();
        assert_eq!(unchanged.primary_category_id, created.primary_category_id);
        assert_eq!(unchanged.status, created.status);
        assert_eq!(unchanged.revision, created.revision);
        assert_eq!(unchanged.published_at, created.published_at);
    }
}

#[tokio::test]
async fn status_only_update_is_a_no_op_but_stale_revision_is_still_refused() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let created = service
        .create_product(tenant_id, actor_id, create_test_product_input())
        .await
        .unwrap();
    let unchanged = service
        .update_product(
            tenant_id,
            actor_id,
            created.id,
            UpdateProductInput {
                status: Some(ProductStatus::Draft),
                expected_revision: Some(created.revision),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(unchanged.revision, created.revision);
    assert_eq!(unchanged.updated_at, created.updated_at);
    let refused = service
        .update_product(
            tenant_id,
            actor_id,
            created.id,
            UpdateProductInput {
                status: Some(ProductStatus::Draft),
                expected_revision: Some(created.revision + 1),
                ..Default::default()
            },
        )
        .await;
    assert!(matches!(refused, Err(CommerceError::Validation(_))));
}

#[tokio::test]
async fn reattached_product_and_variant_values_cannot_be_cleared_as_detached() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let schema = ProductCatalogSchemaService::new(db.clone(), mock_transactional_event_bus());
    let category_id = seed_test_structural_category(&db, tenant_id, "value-category").await;
    let (attribute_id, _) = seed_test_variant_axis(&db, tenant_id, "material", &[]).await;
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        "UPDATE product_attributes SET scope = 'both', value_type = 'text' WHERE id = ?",
        vec![attribute_id.into()],
    ))
    .await
    .unwrap();
    bind_test_category_attribute(&db, tenant_id, category_id, attribute_id).await;
    let mut input = create_test_product_input();
    input.primary_category_id = Some(category_id);
    let created = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();
    let variant_id = created.variants[0].id;
    let patch = ProductAttributeValuePatch {
        attribute_id,
        value: ProductAttributeValuePatchValue::Text("cotton".to_string()),
    };
    schema
        .save_product_attribute_values(tenant_id, actor_id, created.id, "en", vec![patch.clone()])
        .await
        .unwrap();
    schema
        .save_variant_attribute_values(tenant_id, actor_id, variant_id, "en", vec![patch])
        .await
        .unwrap();

    for category in [Patch::Clear, Patch::Set(category_id)] {
        service
            .update_product(
                tenant_id,
                actor_id,
                created.id,
                UpdateProductInput {
                    primary_category_id: category,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }
    let product_clear = schema
        .clear_detached_product_attribute_values(
            tenant_id,
            actor_id,
            created.id,
            "en",
            vec![attribute_id],
        )
        .await;
    let variant_clear = schema
        .clear_detached_variant_attribute_values(
            tenant_id,
            actor_id,
            variant_id,
            "en",
            vec![attribute_id],
        )
        .await;
    assert!(matches!(product_clear, Err(CommerceError::Validation(_))));
    assert!(matches!(variant_clear, Err(CommerceError::Validation(_))));
    let product_values = schema
        .load_product_attribute_values(tenant_id, created.id, "en")
        .await
        .unwrap();
    let variant_values = schema
        .load_variant_attribute_values(tenant_id, variant_id, "en")
        .await
        .unwrap();
    assert_eq!(product_values.len(), 1);
    assert_eq!(variant_values.len(), 1);
    assert!(!product_values[0].detached);
    assert!(!variant_values[0].detached);
}

fn catalog_receipt_context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> rustok_api::PortContext {
    rustok_api::PortContext::new(
        tenant_id.to_string(),
        rustok_api::PortActor::user(actor_id.to_string()),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_claim(rustok_api::Permission::PRODUCTS_MANAGE.to_string())
    .with_idempotency_key(key)
    .with_deadline(std::time::Duration::from_secs(5))
}

#[tokio::test]
async fn catalog_receipt_replays_original_create_snapshot_not_current_product() {
    use rustok_product::ProductCatalogCommandPort;
    use sea_orm::PaginatorTrait;

    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let context = catalog_receipt_context(tenant_id, actor_id, "create-snapshot");
    let mut input = create_test_product_input();
    input.tags = vec!["New Transaction Tag".into()];
    let created =
        ProductCatalogCommandPort::create_product(&service, context.clone(), input.clone())
            .await
            .expect("receipt-bound create must project uncommitted tags and prices");
    assert_eq!(created.tags, input.tags);
    assert_eq!(
        created.variants[0].prices[0].amount,
        Decimal::from_str("99.99").unwrap()
    );
    service
        .update_product(
            tenant_id,
            actor_id,
            created.id,
            UpdateProductInput {
                vendor: Patch::Set("Changed after create".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let replay =
        ProductCatalogCommandPort::create_product(&service, context.clone(), input.clone())
            .await
            .expect("same request must replay without rerunning create");
    assert_eq!(
        serde_json::to_value(&replay).unwrap(),
        serde_json::to_value(&created).unwrap()
    );
    assert_eq!(
        entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .unwrap(),
        1
    );

    input.vendor = Some("Different request".into());
    let conflict = ProductCatalogCommandPort::create_product(&service, context, input)
        .await
        .unwrap_err();
    assert_eq!(conflict.code, "product.idempotency_conflict");
}

#[tokio::test]
async fn catalog_receipt_replays_delete_and_deterministic_failure() {
    use rustok_product::ProductCatalogCommandPort;

    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let created = service
        .create_product(tenant_id, actor_id, create_test_product_input())
        .await
        .unwrap();
    let delete_context = catalog_receipt_context(tenant_id, actor_id, "delete-replay");
    ProductCatalogCommandPort::delete_product(&service, delete_context.clone(), created.id)
        .await
        .unwrap();
    ProductCatalogCommandPort::delete_product(&service, delete_context, created.id)
        .await
        .expect("delete replay must succeed even though the Product no longer exists");

    let context = catalog_receipt_context(tenant_id, actor_id, "failed-create-replay");
    let mut invalid = create_test_product_input();
    invalid.variants.clear();
    let first =
        ProductCatalogCommandPort::create_product(&service, context.clone(), invalid.clone())
            .await
            .unwrap_err();
    assert!(!first.retryable);
    let replay = ProductCatalogCommandPort::create_product(&service, context, invalid)
        .await
        .unwrap_err();
    assert_eq!(first, replay);
}

#[tokio::test]
async fn catalog_projection_failure_rolls_back_owner_rows_before_returning() {
    use rustok_product::ProductCatalogCommandPort;
    use sea_orm::PaginatorTrait;

    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    // This table is read by the final axis-value projection even for variants
    // with no axis values, not by the empty-axis draft insertion path.
    db.execute_unprepared("DROP TABLE product_attribute_option_translations")
        .await
        .unwrap();
    let result = ProductCatalogCommandPort::create_product(
        &service,
        catalog_receipt_context(tenant_id, actor_id, "projection-failure"),
        create_test_product_input(),
    )
    .await;
    let error = result.expect_err("public projection failure must refuse the owner commit");
    assert!(error.retryable);
    assert_eq!(
        entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        entities::product_variant::Entity::find()
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn catalog_image_receipt_preserves_localized_snapshot_on_position_only_edit() {
    use rustok_product::ProductCatalogCommandPort;
    use rustok_product::dto::{AddProductImageInput, UpdateProductImageInput};

    let (_db, service) = setup().await;
    let service = service.with_media_reference_port(std::sync::Arc::new(
        support::product_media::TestMediaReferencePort::default(),
    ));
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let product = service
        .create_product(tenant_id, actor_id, create_test_product_input())
        .await
        .unwrap();
    let image = service
        .add_product_image(
            tenant_id,
            actor_id,
            product.id,
            AddProductImageInput {
                media_id: Uuid::new_v4(),
                position: None,
                alt_text: Some("Stored alt text".into()),
                locale: Some("en".into()),
            },
        )
        .await
        .unwrap();
    let input = UpdateProductImageInput {
        position: Some(0),
        alt_text: None,
        locale: Some("en".into()),
    };
    let context = catalog_receipt_context(tenant_id, actor_id, "image-position-snapshot");
    let updated = ProductCatalogCommandPort::update_product_image(
        &service,
        context.clone(),
        product.id,
        image.id,
        input.clone(),
    )
    .await
    .unwrap();
    assert_eq!(updated.alt_text.as_deref(), Some("Stored alt text"));
    service
        .update_product_image(
            tenant_id,
            actor_id,
            product.id,
            image.id,
            UpdateProductImageInput {
                position: None,
                alt_text: Some("Later alt text".into()),
                locale: Some("en".into()),
            },
        )
        .await
        .unwrap();
    let replay = ProductCatalogCommandPort::update_product_image(
        &service, context, product.id, image.id, input,
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(updated).unwrap(),
        serde_json::to_value(replay).unwrap()
    );
}

async fn seed_atomic_editor_attribute(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    localized: bool,
) -> (Uuid, Uuid) {
    let category_id = seed_test_structural_category(db, tenant_id, "atomic-editor-category").await;
    let (attribute_id, _) =
        seed_test_variant_axis(db, tenant_id, "atomic-editor-material", &[]).await;
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        "UPDATE product_attributes SET scope = 'product', value_type = 'text', is_localized = ? WHERE id = ?",
        vec![localized.into(), attribute_id.into()],
    )).await.unwrap();
    bind_test_category_attribute(db, tenant_id, category_id, attribute_id).await;
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        "UPDATE category_attributes SET is_required = 1 WHERE category_id = ? AND attribute_id = ?",
        vec![category_id.into(), attribute_id.into()],
    ))
    .await
    .unwrap();
    (category_id, attribute_id)
}

fn editor_value_batch(
    locale: &str,
    attribute_id: Uuid,
    text: &str,
) -> rustok_product::dto::ProductAttributeValueBatch {
    rustok_product::dto::ProductAttributeValueBatch {
        locale: locale.into(),
        patches: vec![ProductAttributeValuePatch {
            attribute_id,
            value: ProductAttributeValuePatchValue::Text(text.into()),
        }],
    }
}

#[tokio::test]
async fn atomic_editor_create_publishes_with_required_locales_and_replays_one_revision() {
    use rustok_product::ProductCatalogCommandPort;
    use rustok_product::services::ProductAttributeValue;
    use sea_orm::PaginatorTrait;

    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (category_id, attribute_id) = seed_atomic_editor_attribute(&db, tenant_id, true).await;
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        "UPDATE product_attributes SET validation = ? WHERE id = ?",
        vec![
            serde_json::json!({"requiredLocales": ["en", "fr"]})
                .to_string()
                .into(),
            attribute_id.into(),
        ],
    ))
    .await
    .unwrap();
    let mut input = create_test_product_input();
    input.primary_category_id = Some(category_id);
    input.publish = true;
    input.attribute_values = vec![
        editor_value_batch("en", attribute_id, "cotton"),
        editor_value_batch("fr", attribute_id, "coton"),
    ];
    let context = catalog_receipt_context(tenant_id, actor_id, "atomic-editor-create");
    let created =
        ProductCatalogCommandPort::create_product(&service, context.clone(), input.clone())
            .await
            .expect("all initial values must precede the final publication gate");
    assert_eq!(created.status, ProductStatus::Active);
    let schema = ProductCatalogSchemaService::new(db.clone(), mock_transactional_event_bus());
    for (locale, text) in [("en", "cotton"), ("fr", "coton")] {
        let values = schema
            .load_product_attribute_values(tenant_id, created.id, locale)
            .await
            .unwrap();
        assert!(values.iter().any(|value| value.attribute_id == attribute_id
            && value.value == Some(ProductAttributeValue::Text(text.into()))));
    }
    let replay = ProductCatalogCommandPort::create_product(&service, context, input)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&replay).unwrap(),
        serde_json::to_value(&created).unwrap()
    );
    assert_eq!(
        entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn atomic_editor_update_commits_category_document_values_and_publication_together() {
    use rustok_product::ProductCatalogCommandPort;
    use rustok_product::services::ProductAttributeValue;

    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (category_id, attribute_id) = seed_atomic_editor_attribute(&db, tenant_id, false).await;
    let created = service
        .create_product(tenant_id, actor_id, create_test_product_input())
        .await
        .unwrap();
    let input = UpdateProductInput {
        primary_category_id: Patch::Set(category_id),
        vendor: Patch::Set("Atomic vendor".into()),
        status: Some(ProductStatus::Active),
        expected_revision: Some(created.revision),
        attribute_values: vec![editor_value_batch("en", attribute_id, "cotton")],
        ..Default::default()
    };
    let context = catalog_receipt_context(tenant_id, actor_id, "atomic-editor-update");
    let updated = ProductCatalogCommandPort::update_product(
        &service,
        context.clone(),
        created.id,
        input.clone(),
    )
    .await
    .unwrap();
    assert_eq!(updated.status, ProductStatus::Active);
    assert_eq!(updated.primary_category_id, Some(category_id));
    assert_eq!(updated.vendor.as_deref(), Some("Atomic vendor"));
    assert_eq!(updated.revision, created.revision + 1);
    let replay = ProductCatalogCommandPort::update_product(&service, context, created.id, input)
        .await
        .unwrap();
    assert_eq!(replay.revision, updated.revision);
    let schema = ProductCatalogSchemaService::new(db, mock_transactional_event_bus());
    let values = schema
        .load_product_attribute_values(tenant_id, created.id, "en")
        .await
        .unwrap();
    assert!(
        values
            .iter()
            .any(|value| value.value == Some(ProductAttributeValue::Text("cotton".into())))
    );
}

#[tokio::test]
async fn atomic_editor_late_batch_failure_rolls_back_document_category_and_earlier_values() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (category_id, attribute_id) = seed_atomic_editor_attribute(&db, tenant_id, false).await;
    let created = service
        .create_product(tenant_id, actor_id, create_test_product_input())
        .await
        .unwrap();
    // Two locale batches must not silently overwrite one shared nonlocalized value.
    let failed = service
        .update_product(
            tenant_id,
            actor_id,
            created.id,
            UpdateProductInput {
                primary_category_id: Patch::Set(category_id),
                vendor: Patch::Set("Must roll back".into()),
                status: Some(ProductStatus::Active),
                expected_revision: Some(created.revision),
                attribute_values: vec![
                    editor_value_batch("en", attribute_id, "cotton"),
                    editor_value_batch("fr", attribute_id, "coton"),
                ],
                ..Default::default()
            },
        )
        .await;
    assert!(matches!(failed, Err(CommerceError::Validation(_))));
    let unchanged = service.get_product(tenant_id, created.id).await.unwrap();
    assert_eq!(
        serde_json::to_value(&unchanged).unwrap(),
        serde_json::to_value(&created).unwrap()
    );
    let schema = ProductCatalogSchemaService::new(db, mock_transactional_event_bus());
    assert!(
        schema
            .load_product_attribute_values(tenant_id, created.id, "en")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn atomic_editor_invalid_create_leaves_no_product_or_variant() {
    use sea_orm::PaginatorTrait;

    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (category_id, attribute_id) = seed_atomic_editor_attribute(&db, tenant_id, true).await;
    let mut input = create_test_product_input();
    input.primary_category_id = Some(category_id);
    input.attribute_values = vec![
        editor_value_batch("en", attribute_id, "first value"),
        editor_value_batch("en", attribute_id, "ambiguous duplicate"),
    ];
    assert!(matches!(
        service.create_product(tenant_id, actor_id, input).await,
        Err(CommerceError::Validation(_))
    ));
    assert_eq!(
        entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        entities::product_variant::Entity::find()
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn standalone_attribute_write_invalidates_an_older_atomic_document_draft() {
    use rustok_product::ProductCatalogCommandPort;

    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (category_id, attribute_id) = seed_atomic_editor_attribute(&db, tenant_id, false).await;
    let mut input = create_test_product_input();
    input.primary_category_id = Some(category_id);
    let created = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();
    let schema = ProductCatalogSchemaService::new(db.clone(), mock_transactional_event_bus());
    schema
        .save_product_attribute_values(
            tenant_id,
            actor_id,
            created.id,
            "en",
            editor_value_batch("en", attribute_id, "concurrent edit").patches,
        )
        .await
        .unwrap();
    let current = service.get_product(tenant_id, created.id).await.unwrap();
    assert_eq!(current.revision, created.revision + 1);
    let refused = ProductCatalogCommandPort::update_product(
        &service,
        catalog_receipt_context(tenant_id, actor_id, "atomic-stale-after-values"),
        created.id,
        UpdateProductInput {
            attribute_values: vec![editor_value_batch("en", attribute_id, "stale overwrite")],
            expected_revision: Some(created.revision),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert!(!refused.retryable);
    assert_eq!(
        service
            .get_product(tenant_id, created.id)
            .await
            .unwrap()
            .revision,
        current.revision
    );
    let values = schema
        .load_product_attribute_values(tenant_id, created.id, "en")
        .await
        .unwrap();
    assert!(values.iter().any(|value| value.value
        == Some(rustok_product::services::ProductAttributeValue::Text(
            "concurrent edit".into()
        ))));
}

fn media_image_input(media_id: Uuid) -> rustok_product::dto::AddProductImageInput {
    rustok_product::dto::AddProductImageInput {
        media_id,
        position: None,
        alt_text: None,
        locale: None,
    }
}

#[tokio::test]
async fn image_retention_receipt_replay_does_not_touch_unavailable_media() {
    use rustok_product::ProductCatalogCommandPort;
    use std::sync::{Arc, atomic::Ordering};
    let (db, service) = setup().await;
    let media = Arc::new(support::product_media::TestMediaReferencePort::default());
    let service = service.with_media_reference_port(media.clone());
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let product = service
        .create_product(tenant, actor, create_test_product_input())
        .await
        .unwrap();
    let context = catalog_receipt_context(tenant, actor, "retain-replay");
    let input = media_image_input(Uuid::new_v4());
    let image = ProductCatalogCommandPort::add_product_image(
        &service,
        context.clone(),
        product.id,
        input.clone(),
    )
    .await
    .unwrap();
    assert!(media.has(tenant, image.id));
    assert_eq!(media.calls.lock().unwrap()[0].1, context);
    media.unavailable.store(true, Ordering::SeqCst);
    let replay = ProductCatalogCommandPort::add_product_image(&service, context, product.id, input)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&image).unwrap(),
        serde_json::to_value(replay).unwrap()
    );
    assert_eq!(media.calls.lock().unwrap().len(), 1);
    assert!(
        entities::product_image_media_coverage::Entity::find_by_id(image.id)
            .one(&db)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn committed_image_delete_survives_release_failure_and_preserves_duplicate_asset_hold() {
    use rustok_product::ProductCatalogCommandPort;
    use sea_orm::PaginatorTrait;
    use std::sync::{Arc, atomic::Ordering};
    let (db, service) = setup().await;
    let media = Arc::new(support::product_media::TestMediaReferencePort::default());
    let service = service.with_media_reference_port(media.clone());
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let product = service
        .create_product(tenant, actor, create_test_product_input())
        .await
        .unwrap();
    let asset = Uuid::new_v4();
    let first = service
        .add_product_image(tenant, actor, product.id, media_image_input(asset))
        .await
        .unwrap();
    let second = service
        .add_product_image(tenant, actor, product.id, media_image_input(asset))
        .await
        .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(media.hold_count(), 2);
    let context = catalog_receipt_context(tenant, actor, "delete-image-release-failed");
    ProductCatalogCommandPort::delete_product_image(
        &service,
        context.clone(),
        product.id,
        first.id,
    )
    .await
    .unwrap();
    assert_eq!(
        entities::product_media_release::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(media.hold_count(), 2); // no remote IO inside the deletion transaction
    media.reject_release.store(true, Ordering::SeqCst);
    let failed = service.process_media_releases(100).await.unwrap();
    assert_eq!(failed.failed, 1);
    ProductCatalogCommandPort::delete_product_image(&service, context, product.id, first.id)
        .await
        .unwrap();
    assert!(
        entities::product_image::Entity::find_by_id(first.id)
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        entities::product_image_media_coverage::Entity::find_by_id(first.id)
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );
    media.reject_release.store(false, Ordering::SeqCst);
    entities::product_media_release::Entity::update_many()
        .col_expr(
            entities::product_media_release::Column::NextAttemptAt,
            sea_orm::sea_query::Expr::value(chrono::Utc::now()),
        )
        .exec(&db)
        .await
        .unwrap();
    assert_eq!(
        service.process_media_releases(100).await.unwrap().released,
        1
    );
    assert!(!media.has(tenant, first.id));
    assert!(media.has(tenant, second.id));
    assert_eq!(
        entities::product_media_release::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn product_delete_durably_enqueues_every_exact_image_reference() {
    use rustok_product::ProductCatalogCommandPort;
    use sea_orm::PaginatorTrait;
    use std::sync::Arc;
    let (db, service) = setup().await;
    let media = Arc::new(support::product_media::TestMediaReferencePort::default());
    let service = service.with_media_reference_port(media.clone());
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let product = service
        .create_product(tenant, actor, create_test_product_input())
        .await
        .unwrap();
    for _ in 0..2 {
        service
            .add_product_image(tenant, actor, product.id, media_image_input(Uuid::new_v4()))
            .await
            .unwrap();
    }
    ProductCatalogCommandPort::delete_product(
        &service,
        catalog_receipt_context(tenant, actor, "product-media-delete"),
        product.id,
    )
    .await
    .unwrap();
    assert_eq!(
        entities::product_media_release::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        2
    );
    assert_eq!(media.hold_count(), 2);
    assert_eq!(
        service.process_media_releases(100).await.unwrap().released,
        2
    );
    assert_eq!(media.hold_count(), 0);
}

#[tokio::test]
async fn failed_image_insert_keeps_conservative_hold_without_release_intent() {
    use rustok_product::ProductCatalogCommandPort;
    use sea_orm::PaginatorTrait;
    use std::sync::Arc;
    let (db, service) = setup().await;
    let media = Arc::new(support::product_media::TestMediaReferencePort::default());
    let service = service.with_media_reference_port(media.clone());
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let product = service
        .create_product(tenant, actor, create_test_product_input())
        .await
        .unwrap();
    db.execute_unprepared("CREATE TRIGGER fail_image_insert BEFORE INSERT ON product_images BEGIN SELECT RAISE(FAIL, 'injected image write failure'); END").await.unwrap();
    let result = ProductCatalogCommandPort::add_product_image(
        &service,
        catalog_receipt_context(tenant, actor, "retain-then-insert-failure"),
        product.id,
        media_image_input(Uuid::new_v4()),
    )
    .await;
    assert!(result.unwrap_err().retryable);
    assert_eq!(
        entities::product_image::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        entities::product_image_media_coverage::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        entities::product_media_release::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(media.hold_count(), 1); // NEVER compensate an ambiguous Product write
}

#[tokio::test]
async fn failed_image_delete_rolls_back_release_work_together_with_relation() {
    use rustok_product::ProductCatalogCommandPort;
    use sea_orm::PaginatorTrait;
    use std::sync::Arc;
    let (db, service) = setup().await;
    let media = Arc::new(support::product_media::TestMediaReferencePort::default());
    let service = service.with_media_reference_port(media.clone());
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let product = service
        .create_product(tenant, actor, create_test_product_input())
        .await
        .unwrap();
    let image = service
        .add_product_image(tenant, actor, product.id, media_image_input(Uuid::new_v4()))
        .await
        .unwrap();
    db.execute_unprepared("CREATE TRIGGER fail_image_delete BEFORE DELETE ON product_images BEGIN SELECT RAISE(FAIL, 'injected image removal failure'); END").await.unwrap();
    assert!(
        ProductCatalogCommandPort::delete_product_image(
            &service,
            catalog_receipt_context(tenant, actor, "delete-rollback"),
            product.id,
            image.id
        )
        .await
        .unwrap_err()
        .retryable
    );
    assert_eq!(
        entities::product_media_release::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert!(
        entities::product_image::Entity::find_by_id(image.id)
            .one(&db)
            .await
            .unwrap()
            .is_some()
    );
    assert!(media.has(tenant, image.id));
}

#[tokio::test]
async fn product_image_cross_tenant_media_is_rejected_before_local_commit() {
    use rustok_product::ProductCatalogCommandPort;
    use sea_orm::PaginatorTrait;
    use std::sync::Arc;
    let (db, service) = setup().await;
    let media = Arc::new(support::product_media::TestMediaReferencePort::default());
    let service = service.with_media_reference_port(media.clone());
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let asset = Uuid::new_v4();
    media
        .asset_owners
        .lock()
        .unwrap()
        .insert(asset, Uuid::new_v4());
    let product = service
        .create_product(tenant, actor, create_test_product_input())
        .await
        .unwrap();
    let error = ProductCatalogCommandPort::add_product_image(
        &service,
        catalog_receipt_context(tenant, actor, "foreign-media"),
        product.id,
        media_image_input(asset),
    )
    .await
    .unwrap_err();
    assert!(!error.retryable);
    assert_eq!(
        entities::product_image::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(media.hold_count(), 0);
}

#[tokio::test]
async fn legacy_image_backfill_preserves_ids_gates_admission_and_audit_never_releases() {
    use rustok_api::Permission;
    use std::sync::Arc;
    let (db, service) = setup().await;
    let media = Arc::new(support::product_media::TestMediaReferencePort::default());
    let service = service.with_media_reference_port(media.clone());
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let product = service
        .create_product(tenant, actor, create_test_product_input())
        .await
        .unwrap();
    let legacy_id = Uuid::new_v4();
    entities::product_image::ActiveModel {
        id: Set(legacy_id),
        product_id: Set(product.id),
        media_id: Set(Uuid::new_v4()),
        position: Set(0),
    }
    .insert(&db)
    .await
    .unwrap();
    assert!(
        service
            .add_product_image(tenant, actor, product.id, media_image_input(Uuid::new_v4()))
            .await
            .is_err()
    );
    assert_eq!(media.hold_count(), 0);
    let operator = catalog_receipt_context(tenant, actor, "legacy-backfill")
        .with_claim(Permission::PRODUCTS_MANAGE.to_string())
        .with_claim(Permission::PRODUCTS_READ.to_string());
    let initial = service
        .audit_media_references(operator.clone(), None, None, 1)
        .await
        .unwrap();
    assert!(
        initial
            .issues
            .iter()
            .any(|issue| issue.kind == "missing_media_hold")
    );
    let result = service
        .backfill_media_references(operator.clone(), None, 1)
        .await
        .unwrap();
    assert_eq!(result.retained, 1);
    assert!(!result.legacy_images_remaining);
    assert!(media.has(tenant, legacy_id));
    let orphan = Uuid::new_v4();
    media
        .holds
        .lock()
        .unwrap()
        .insert((tenant, orphan), Uuid::new_v4());
    let audit = service
        .audit_media_references(operator.clone(), None, None, 100)
        .await
        .unwrap();
    assert!(
        audit
            .issues
            .iter()
            .any(|issue| issue.kind == "orphan_media_hold" && issue.image_id == orphan)
    );
    assert!(media.has(tenant, orphan));
    assert!(
        service
            .audit_media_references(operator.clone(), None, None, 101)
            .await
            .is_err()
    );
    assert!(
        service
            .backfill_media_references(operator, None, 0)
            .await
            .is_err()
    );
    service
        .add_product_image(tenant, actor, product.id, media_image_input(Uuid::new_v4()))
        .await
        .unwrap();
}

#[derive(Default)]
struct ShippingAdmission {
    mode: std::sync::atomic::AtomicUsize,
    calls: std::sync::atomic::AtomicUsize,
    selections: std::sync::Mutex<Vec<Vec<String>>>,
}
#[async_trait::async_trait]
impl rustok_product::ProductShippingProfilePort for ShippingAdmission {
    async fn validate(
        &self,
        context: rustok_api::PortContext,
        slugs: Vec<String>,
    ) -> Result<(), rustok_api::PortError> {
        use std::sync::atomic::Ordering;
        context.require_policy(rustok_api::PortCallPolicy::read())?;
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.selections.lock().unwrap().push(slugs);
        match self.mode.load(Ordering::SeqCst) {
            1 => Err(rustok_api::PortError::not_found(
                "shipping.missing",
                "Unknown profile",
            )),
            2 => Err(rustok_api::PortError::unavailable(
                "shipping.outage",
                "Unavailable",
            )),
            _ => Ok(()),
        }
    }
}

#[tokio::test]
async fn product_shipping_admission_checks_initial_variant_overrides_but_not_completed_replay() {
    use rustok_product::ProductCatalogCommandPort;
    use std::sync::{Arc, atomic::Ordering};
    let (_, service) = setup().await;
    let policy = Arc::new(ShippingAdmission::default());
    let service = service.with_shipping_profile_port(policy.clone());
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let context = catalog_receipt_context(tenant, actor, "shipping-create");
    let mut input = create_test_product_input();
    input.shipping_profile_slug = Some(" Standard ".into());
    input.variants[0].shipping_profile_slug = Some(" EXPRESS ".into());
    let created =
        ProductCatalogCommandPort::create_product(&service, context.clone(), input.clone())
            .await
            .unwrap();
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        policy.selections.lock().unwrap()[0],
        vec!["express".to_string(), "standard".to_string()]
    );
    policy.mode.store(2, Ordering::SeqCst);
    let replay = ProductCatalogCommandPort::create_product(&service, context, input)
        .await
        .unwrap();
    assert_eq!(created.id, replay.id);
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn shipping_admission_fails_closed_and_authoritative_rejection_is_receipted() {
    use rustok_product::ProductCatalogCommandPort;
    use sea_orm::PaginatorTrait;
    use std::sync::{Arc, atomic::Ordering};
    let (db, service) = setup().await;
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let mut input = create_test_product_input();
    input.variants[0].shipping_profile_slug = Some("unknown".into());
    let absent = ProductCatalogCommandPort::create_product(
        &service,
        catalog_receipt_context(tenant, actor, "shipping-no-provider"),
        input.clone(),
    )
    .await
    .unwrap_err();
    assert!(absent.retryable);
    let policy = Arc::new(ShippingAdmission::default());
    policy.mode.store(1, Ordering::SeqCst);
    let service = service.with_shipping_profile_port(policy.clone());
    let context = catalog_receipt_context(tenant, actor, "shipping-refused");
    let denied =
        ProductCatalogCommandPort::create_product(&service, context.clone(), input.clone())
            .await
            .unwrap_err();
    assert!(!denied.retryable);
    assert_eq!(
        entities::product::Entity::find().count(&db).await.unwrap(),
        0
    );
    policy.mode.store(0, Ordering::SeqCst);
    let replay = ProductCatalogCommandPort::create_product(&service, context, input)
        .await
        .unwrap_err();
    assert!(!replay.retryable);
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn nullable_variant_patches_preserve_clear_set_and_replay_without_shipping_io() {
    use rustok_product::{ProductCatalogCommandPort, dto::UpdateVariantInput};
    use std::sync::{Arc, atomic::Ordering};
    let (db, service) = setup().await;
    let policy = Arc::new(ShippingAdmission::default());
    let service = service.with_shipping_profile_port(policy.clone());
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let mut seed = create_test_product_input();
    seed.variants[0].barcode = Some("0123".into());
    seed.variants[0].shipping_profile_slug = Some("parcel".into());
    let created = service.create_product(tenant, actor, seed).await.unwrap();
    let id = created.variants[0].id;
    let initial = created.variants[0].clone();
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
    policy.mode.store(2, Ordering::SeqCst);
    let kept = ProductCatalogCommandPort::update_variant(
        &service,
        catalog_receipt_context(tenant, actor, "variant-keep"),
        id,
        UpdateVariantInput::default(),
    )
    .await
    .unwrap();
    assert_eq!(kept.sku, initial.sku);
    assert_eq!(kept.barcode, initial.barcode);
    assert_eq!(kept.shipping_profile_slug, initial.shipping_profile_slug);
    assert_eq!(kept.weight, initial.weight);
    assert_eq!(kept.weight_unit, initial.weight_unit);
    let clear = UpdateVariantInput {
        sku: Patch::Clear,
        barcode: Patch::Clear,
        shipping_profile_slug: Patch::Clear,
        weight: Patch::Clear,
        weight_unit: Patch::Clear,
        ..Default::default()
    };
    let context = catalog_receipt_context(tenant, actor, "variant-clear");
    let cleared =
        ProductCatalogCommandPort::update_variant(&service, context.clone(), id, clear.clone())
            .await
            .unwrap();
    assert!(cleared.sku.is_none() && cleared.barcode.is_none());
    assert!(
        cleared.shipping_profile_slug.is_none()
            && cleared.weight.is_none()
            && cleared.weight_unit.is_none()
    );
    let row = entities::product_variant::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(row.sku.is_none() && row.barcode.is_none());
    assert!(
        row.shipping_profile_slug.is_none() && row.weight.is_none() && row.weight_unit.is_none()
    );
    assert_eq!(
        policy.calls.load(Ordering::SeqCst),
        1,
        "Keep and Clear must not call Shipping"
    );
    policy.mode.store(0, Ordering::SeqCst);
    let set = UpdateVariantInput {
        sku: Patch::Set("NEW-SKU".into()),
        barcode: Patch::Set("0456".into()),
        shipping_profile_slug: Patch::Set(" EXPRESS ".into()),
        weight: Patch::Set(Decimal::from_str("0.000000001").unwrap()),
        weight_unit: Patch::Set("kg".into()),
        ..Default::default()
    };
    let set_context = catalog_receipt_context(tenant, actor, "variant-set");
    let updated =
        ProductCatalogCommandPort::update_variant(&service, set_context.clone(), id, set.clone())
            .await
            .unwrap();
    assert_eq!(updated.sku.as_deref(), Some("NEW-SKU"));
    assert_eq!(updated.barcode.as_deref(), Some("0456"));
    assert_eq!(updated.shipping_profile_slug.as_deref(), Some("express"));
    assert_eq!(
        updated.weight,
        Some(Decimal::from_str("0.000000001").unwrap())
    );
    assert_eq!(updated.weight_unit.as_deref(), Some("kg"));
    assert_eq!(policy.calls.load(Ordering::SeqCst), 2);
    policy.mode.store(2, Ordering::SeqCst);
    let replay = ProductCatalogCommandPort::update_variant(&service, set_context, id, set)
        .await
        .unwrap();
    assert_eq!(replay.sku, updated.sku);
    let old_clear = ProductCatalogCommandPort::update_variant(&service, context.clone(), id, clear)
        .await
        .unwrap();
    assert!(
        old_clear.sku.is_none(),
        "Receipt replays the original result, not current state"
    );
    let row = entities::product_variant::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        row.sku.as_deref(),
        Some("NEW-SKU"),
        "Replay must not reapply the clear"
    );
    assert_eq!(policy.calls.load(Ordering::SeqCst), 2);
    let changed = ProductCatalogCommandPort::update_variant(
        &service,
        context,
        id,
        UpdateVariantInput::default(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        changed.kind,
        rustok_api::PortErrorKind::Conflict,
        "Keep and Clear cannot share one receipt payload"
    );
}

#[tokio::test]
async fn nullable_variant_text_limits_are_checked_before_shipping_or_variant_lookup() {
    use rustok_product::dto::UpdateVariantInput;
    use std::sync::{Arc, atomic::Ordering};
    let (_, service) = setup().await;
    let policy = Arc::new(ShippingAdmission::default());
    let service = service.with_shipping_profile_port(policy.clone());
    for field in ["sku", "barcode", "shipping_profile_slug", "weight_unit"] {
        let limit = if field == "weight_unit" { 16 } else { 100 };
        let mut json = serde_json::json!({"shipping_profile_slug": "parcel"});
        json[field] = serde_json::json!("я".repeat(limit + 1));
        let input: UpdateVariantInput = serde_json::from_value(json).unwrap();
        let error = service
            .update_variant(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), input)
            .await
            .unwrap_err();
        assert!(matches!(error, CommerceError::Validation(_)));
    }
    assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
}
