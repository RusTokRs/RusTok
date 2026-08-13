use rust_decimal::Decimal;
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_product::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput, UpdateProductInput,
};
use rustok_product::entities::product_tag;
use rustok_product::{CatalogService, CommerceError};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
    entities::taxonomy_term,
};
use rustok_test_utils::{db::setup_test_db, helpers::unique_slug};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use sea_orm_migration::MigrationTrait;
use sea_orm_migration::prelude::SchemaManager;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

mod support;

async fn setup() -> (DatabaseConnection, CatalogService) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let schema_manager = SchemaManager::new(&db);
    SysEventsMigration
        .up(&schema_manager)
        .await
        .expect("outbox schema should be available for product tag tests");
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = CatalogService::new(db.clone(), event_bus);
    (db, service)
}

fn admin() -> rustok_core::SecurityContext {
    rustok_core::SecurityContext::new(rustok_core::UserRole::Admin, Some(Uuid::new_v4()))
}

fn create_test_product_input(tags: &[&str]) -> CreateProductInput {
    CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Tagged Product".to_string(),
            description: Some("A product with taxonomy tags".to_string()),
            handle: Some(unique_slug("tagged-product")),
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
                compare_at_amount: None,
            }],
            inventory_quantity: 0,
            inventory_policy: "deny".to_string(),
            weight: None,
            weight_unit: None,
        }],
        seller_id: None,
        vendor: Some("Acme".to_string()),
        product_type: Some("Physical".to_string()),
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        metadata: serde_json::json!({
            "featured": true,
        }),
        publish: false,
    }
}

#[tokio::test]
async fn product_tags_are_synced_into_product_tags_without_metadata_mirror() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let product = service
        .create_product(
            tenant_id,
            actor_id,
            create_test_product_input(&["sale", "new", "sale"]),
        )
        .await
        .expect("product should be created");

    assert_eq!(product.metadata["featured"], true);
    assert_eq!(
        {
            let mut tags = product.tags.clone();
            tags.sort();
            tags
        },
        vec!["new".to_string(), "sale".to_string()]
    );
    assert!(product.metadata.get("tags").is_none());

    let relations = product_tag::Entity::find()
        .filter(product_tag::Column::ProductId.eq(product.id))
        .all(&db)
        .await
        .expect("product tag relations should load");
    assert_eq!(relations.len(), 2);

    let terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("taxonomy terms should load");
    assert_eq!(terms.len(), 2);
    assert!(
        terms
            .iter()
            .all(|term| term.scope_type == TaxonomyScopeType::Module)
    );
    assert!(terms.iter().all(|term| term.scope_value == "product"));
}

#[tokio::test]
async fn product_tag_sync_reuses_existing_global_taxonomy_term() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let global_sale_term_id = TaxonomyService::new(db.clone())
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: "sale".to_string(),
                slug: None,
                canonical_key: None,
                description: None,
                aliases: vec![],
            },
        )
        .await
        .expect("global sale term should be created");

    let product = service
        .create_product(
            tenant_id,
            actor_id,
            create_test_product_input(&["sale", "new"]),
        )
        .await
        .expect("product should be created");

    let attached_term_ids = product_tag::Entity::find()
        .filter(product_tag::Column::ProductId.eq(product.id))
        .all(&db)
        .await
        .expect("product tag relations should load")
        .into_iter()
        .map(|relation| relation.term_id)
        .collect::<Vec<_>>();
    assert!(attached_term_ids.contains(&global_sale_term_id));

    let product_terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("taxonomy terms should load");
    assert_eq!(product_terms.len(), 2);
    assert_eq!(
        product_terms
            .iter()
            .filter(|term| {
                term.scope_type == TaxonomyScopeType::Module && term.scope_value == "product"
            })
            .count(),
        1
    );
}

#[tokio::test]
async fn update_product_tags_resyncs_product_tag_relations_without_metadata_mirror() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let product = service
        .create_product(
            tenant_id,
            actor_id,
            create_test_product_input(&["sale", "new"]),
        )
        .await
        .expect("product should be created");

    let updated = service
        .update_product(
            tenant_id,
            actor_id,
            product.id,
            UpdateProductInput {
                translations: None,
                seller_id: None,
                vendor: None,
                product_type: None,
                shipping_profile_slug: None,
                primary_category_id: None,
                tags: Some(vec![
                    "featured".to_string(),
                    "sale".to_string(),
                    "featured".to_string(),
                ]),
                metadata: Some(serde_json::json!({
                    "featured": false,
                })),
                status: None,
            },
        )
        .await
        .expect("product should be updated");

    assert_eq!(updated.metadata["featured"], false);
    assert_eq!(
        {
            let mut tags = updated.tags.clone();
            tags.sort();
            tags
        },
        vec!["featured".to_string(), "sale".to_string()]
    );
    assert!(updated.metadata.get("tags").is_none());

    let attached_terms = product_tag::Entity::find()
        .filter(product_tag::Column::ProductId.eq(product.id))
        .all(&db)
        .await
        .expect("product tag relations should load");
    assert_eq!(attached_terms.len(), 2);

    let tag_names = TaxonomyService::new(db.clone())
        .resolve_term_names(
            tenant_id,
            &attached_terms
                .iter()
                .map(|relation| relation.term_id)
                .collect::<Vec<_>>(),
            "en",
            Some("en"),
        )
        .await
        .expect("term names should resolve");
    let mut names = attached_terms
        .into_iter()
        .filter_map(|relation| tag_names.get(&relation.term_id).cloned())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, vec!["featured".to_string(), "sale".to_string()]);
}

#[tokio::test]
async fn update_product_tags_only_preserves_existing_non_tag_metadata() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let product = service
        .create_product(
            tenant_id,
            actor_id,
            create_test_product_input(&["sale", "new"]),
        )
        .await
        .expect("product should be created");

    let updated = service
        .update_product(
            tenant_id,
            actor_id,
            product.id,
            UpdateProductInput {
                translations: None,
                seller_id: None,
                vendor: None,
                product_type: None,
                shipping_profile_slug: None,
                primary_category_id: None,
                tags: Some(vec!["featured".to_string()]),
                metadata: None,
                status: None,
            },
        )
        .await
        .expect("product should be updated");

    assert_eq!(updated.metadata["featured"], true);
    assert!(updated.metadata.get("tags").is_none());
    assert_eq!(updated.tags, vec!["featured".to_string()]);
}

#[tokio::test]
async fn metadata_tags_are_rejected_on_product_create() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let mut input = create_test_product_input(&[]);
    input.metadata = serde_json::json!({
        "featured": true,
        "tags": ["legacy", "sale"]
    });

    let error = service
        .create_product(tenant_id, actor_id, input)
        .await
        .expect_err("metadata.tags must not remain a Product tag write contract");

    assert!(matches!(error, CommerceError::Validation(_)));
    assert!(error.to_string().contains("typed tags field"));
}

#[tokio::test]
async fn metadata_tags_are_rejected_on_update_without_mutating_canonical_tags() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let product = service
        .create_product(tenant_id, actor_id, create_test_product_input(&["sale"]))
        .await
        .expect("product should be created");

    let error = service
        .update_product(
            tenant_id,
            actor_id,
            product.id,
            UpdateProductInput {
                translations: None,
                seller_id: None,
                vendor: None,
                product_type: None,
                shipping_profile_slug: None,
                primary_category_id: None,
                tags: None,
                metadata: Some(serde_json::json!({
                    "featured": false,
                    "tags": ["legacy", "new"]
                })),
                status: None,
            },
        )
        .await
        .expect_err("metadata.tags must be rejected on Product update");

    assert!(matches!(error, CommerceError::Validation(_)));
    assert!(error.to_string().contains("typed tags field"));

    let reloaded = service
        .get_product(tenant_id, product.id)
        .await
        .expect("failed update must leave the Product unchanged");
    assert_eq!(reloaded.tags, vec!["sale".to_string()]);
    assert_eq!(reloaded.metadata["featured"], true);
    assert!(reloaded.metadata.get("tags").is_none());
}
