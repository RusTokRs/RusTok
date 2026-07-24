// Comprehensive unit tests for CatalogService
// These tests verify product CRUD, variants, translations,
// pricing, and publishing workflows.

use rust_decimal::Decimal;
use rustok_commerce::CommerceError;
use rustok_core::field_schema::FieldType;
use rustok_product::CatalogService;
use rustok_product::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput, UpdateProductInput,
};
use rustok_product::entities;
use rustok_product::entities::product::ProductStatus;
use rustok_test_utils::{db::setup_test_db, helpers::unique_slug, mock_transactional_event_bus};
use sea_orm::DatabaseConnection;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use std::str::FromStr;
use uuid::Uuid;

mod support;

mod product_field_definitions_storage {
    rustok_core::define_field_definitions_entity!("product_field_definitions");
}

mod attached_values_storage {
    use sea_orm::entity::prelude::*;
    use std::iter::FusedIterator;

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

fn create_test_product_input() -> CreateProductInput {
    CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Test Product".to_string(),
            description: Some("A great test product".to_string()),
            handle: Some(unique_slug("test-product")),
            meta_title: None,
            meta_description: None,
        }],
        options: vec![],
        variants: vec![CreateVariantInput {
            sku: Some(format!(
                "SKU-{}",
                Uuid::new_v4().to_string().split('-').next().unwrap()
            )),
            barcode: None,
            shipping_profile_slug: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
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
                translations: None,
                seller_id: None,
                vendor: None,
                product_type: None,
                shipping_profile_slug: Some("Cold-Chain".to_string()),
                primary_category_id: None,
                tags: None,
                status: None,
                metadata: None,
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
        translations: Some(vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Updated Product".to_string(),
            description: Some("Updated description".to_string()),
            handle: None,
            meta_title: None,
            meta_description: None,
        }]),
        seller_id: None,
        vendor: Some("Updated Vendor".to_string()),
        product_type: Some("Digital".to_string()),
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: None,
        status: Some(ProductStatus::Active),
        metadata: None,
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
                seller_id: None,
                vendor: None,
                product_type: None,
                shipping_profile_slug: None,
                primary_category_id: None,
                tags: None,
                status: None,
                metadata: Some(serde_json::json!({
                    "marketing_copy": "Русский промо текст",
                    "badge": "featured"
                })),
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
async fn test_create_product_populates_option_and_variant_translation_groups() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.translations.push(ProductTranslationInput {
        locale: "ru".to_string(),
        title: "Р СћР ВµРЎРѓРЎвЂљР С•Р Р†РЎвЂ№Р в„– Р С—РЎР‚Р С•Р Т‘РЎС“Р С”РЎвЂљ".to_string(),
        description: Some(
            "Р В РЎС“РЎРѓРЎРѓР С”Р В°РЎРЏ Р В»Р С•Р С”Р В°Р В»Р С‘Р В·Р В°РЎвЂ Р С‘РЎРЏ"
                .to_string(),
        ),
        handle: Some(unique_slug("test-product-ru")),
        meta_title: None,
        meta_description: None,
    });
    input.options = vec![rustok_product::dto::ProductOptionInput {
        translations: vec![rustok_product::dto::ProductOptionTranslationInput {
            locale: "en".to_string(),
            name: "Size".to_string(),
            values: vec!["S".to_string(), "M".to_string()],
        }],
    }];

    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .expect("product with translation groups should be created");

    assert_eq!(product.options.len(), 1);
    assert_eq!(product.options[0].translations.len(), 2);
    assert!(
        product.options[0]
            .translations
            .iter()
            .any(|item| item.locale == "en"
                && item.name == "Size"
                && item.values == vec!["S", "M"])
    );
    assert!(
        product.options[0]
            .translations
            .iter()
            .any(|item| item.locale == "ru"
                && item.name == "Size"
                && item.values == vec!["S", "M"])
    );

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
        alt_text: sea_orm::Set(Some("Default image".to_string())),
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
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
    input.variants.push(CreateVariantInput {
        sku: Some(format!(
            "SKU-{}",
            Uuid::new_v4().to_string().split('-').next().unwrap()
        )),
        barcode: None,
        shipping_profile_slug: None,
        option1: Some("Small".to_string()),
        option2: None,
        option3: None,
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
        option1: Some("Large".to_string()),
        option2: None,
        option3: None,
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

    assert!(result.is_ok());
    let product = result.unwrap();
    assert_eq!(product.variants.len(), 3);

    let small = product.variants.iter().find(|v| v.title == "Small");
    let large = product.variants.iter().find(|v| v.title == "Large");

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
    input.metadata = serde_json::json!({
        "featured": true,
        "tags": ["new", "sale", "popular"],
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
        translations: None,
        seller_id: None,
        vendor: None,
        product_type: None,
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: None,
        status: None,
        metadata: Some(serde_json::json!({
            "featured": true,
            "priority": "high",
            "badge": "bestseller"
        })),
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
        translations: None,
        seller_id: None,
        vendor: Some("New Vendor Inc".to_string()),
        product_type: None,
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: None,
        status: None,
        metadata: None,
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
        translations: Some(vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Updated".to_string(),
            description: None,
            handle: None,
            meta_title: None,
            meta_description: None,
        }]),
        seller_id: None,
        vendor: None,
        product_type: None,
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: None,
        status: None,
        metadata: None,
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
        translations: None,
        seller_id: None,
        vendor: None,
        product_type: None,
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: None,
        status: Some(ProductStatus::Archived),
        metadata: None,
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
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_test_product_input();
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
        option1: Some("Premium".to_string()),
        option2: None,
        option3: None,
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

    assert!(result.is_ok());
    let product = result.unwrap();
    assert_eq!(product.variants.len(), 2);

    // Find variants by their option1 value (used as title)
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
