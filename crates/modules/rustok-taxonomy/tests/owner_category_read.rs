use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    ModuleTermCreateInput, ModuleTermUpdateInput, SetTaxonomyCategoryPlacementInput,
    SetTaxonomyCategoryPresentationInput, TaxonomyModule, TaxonomyOwnerCategoryReader,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
    entities::taxonomy_term_translation, update_module_term_in_tx,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

async fn setup() -> (sea_orm::DatabaseConnection, TaxonomyService) {
    let db = setup_test_db().await;
    let schema_manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema_manager)
            .await
            .expect("failed to run taxonomy migration");
    }
    let service = TaxonomyService::new(db.clone());
    (db, service)
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn create_term(
    service: &TaxonomyService,
    tenant_id: Uuid,
    kind: TaxonomyTermKind,
    scope_value: &str,
    name: &str,
    slug: &str,
    description: Option<&str>,
) -> Uuid {
    let txn = service
        .database()
        .begin()
        .await
        .expect("transaction should start");
    let term_id = service
        .create_module_term_in_tx(
            &txn,
            tenant_id,
            kind,
            scope_value,
            ModuleTermCreateInput {
                locale: "en".to_owned(),
                name: name.to_owned(),
                slug: Some(slug.to_owned()),
                canonical_key: Some(slug.to_owned()),
            },
        )
        .await
        .expect("taxonomy term should be created");

    if let Some(desc) = description {
        taxonomy_term_translation::Entity::update_many()
            .filter(taxonomy_term_translation::Column::TermId.eq(term_id))
            .filter(taxonomy_term_translation::Column::Locale.eq("en"))
            .col_expr(
                taxonomy_term_translation::Column::Description,
                sea_orm::sea_query::Expr::value(Some(desc.to_string())),
            )
            .exec(&txn)
            .await
            .expect("description should be written");
    }
    txn.commit().await.expect("transaction should commit");
    term_id
}

#[tokio::test]
async fn category_owner_reader_batches_canonical_copy_hierarchy_and_presentation() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();

    let parent_id = create_term(
        &service,
        tenant_id,
        TaxonomyTermKind::Category,
        "forum",
        "General",
        "general",
        Some("General discussion"),
    )
    .await;
    let child_id = create_term(
        &service,
        tenant_id,
        TaxonomyTermKind::Category,
        "forum",
        "Support",
        "support",
        Some("Support in English"),
    )
    .await;
    let tag_id = create_term(
        &service,
        tenant_id,
        TaxonomyTermKind::Tag,
        "forum",
        "Noise",
        "noise",
        None,
    )
    .await;
    let foreign_category_id = create_term(
        &service,
        foreign_tenant_id,
        TaxonomyTermKind::Category,
        "forum",
        "Foreign",
        "foreign",
        None,
    )
    .await;

    let txn = service
        .database()
        .begin()
        .await
        .expect("transaction should start");
    update_module_term_in_tx(
        &txn,
        tenant_id,
        child_id,
        &admin(),
        TaxonomyTermKind::Category,
        "forum",
        ModuleTermUpdateInput {
            locale: "ar".to_owned(),
            name: Some("الدعم".to_owned()),
            slug: Some("support-ar".to_owned()),
        },
    )
    .await
    .expect("Arabic category copy should be added");

    taxonomy_term_translation::Entity::update_many()
        .filter(taxonomy_term_translation::Column::TermId.eq(child_id))
        .filter(taxonomy_term_translation::Column::Locale.eq("ar"))
        .col_expr(
            taxonomy_term_translation::Column::Description,
            sea_orm::sea_query::Expr::value(Some("الدعم بالعربية".to_string())),
        )
        .exec(&txn)
        .await
        .expect("Arabic category description should be written");
    txn.commit().await.expect("transaction should commit");
    service
        .set_category_placement(
            tenant_id,
            admin(),
            child_id,
            SetTaxonomyCategoryPlacementInput {
                parent_id: Some(parent_id),
                position: 4,
            },
        )
        .await
        .expect("Category placement should be written");
    service
        .set_category_presentation(
            tenant_id,
            admin(),
            child_id,
            SetTaxonomyCategoryPresentationInput {
                icon_key: Some("support-badge".to_owned()),
                color: Some("#F0A".to_owned()),
                image_media_id: None,
                cover_media_id: None,
                expected_revision: Some(0),
            },
            None,
        )
        .await
        .expect("Category presentation should be written");

    let reader = TaxonomyOwnerCategoryReader::new(db);
    let categories = reader
        .load_scoped_categories(
            tenant_id,
            TaxonomyScopeType::Module,
            Some(" Forum! "),
            Some(&[child_id, parent_id, tag_id, foreign_category_id]),
            "ar",
            Some("en"),
        )
        .await
        .expect("Category owner projection should succeed");

    assert_eq!(categories.len(), 2);
    let child = categories
        .iter()
        .find(|category| category.id == child_id)
        .expect("child Category should be projected");
    assert_eq!(child.scope_value.as_deref(), Some("forum"));
    assert_eq!(child.requested_locale, "ar");
    assert_eq!(child.effective_locale, "ar");
    assert_eq!(child.available_locales, vec!["ar", "en"]);
    assert_eq!(child.name, "الدعم");
    assert_eq!(child.slug, "support-ar");
    assert_eq!(child.description.as_deref(), Some("الدعم بالعربية"));
    assert_eq!(child.parent_id, Some(parent_id));
    assert_eq!(child.position, 4);
    assert_eq!(child.icon_key.as_deref(), Some("support-badge"));
    assert_eq!(child.color.as_deref(), Some("#ff00aa"));
    assert_eq!(child.image_media_id, None);
    assert_eq!(child.cover_media_id, None);
    assert_eq!(child.presentation_revision, 1);

    let parent = categories
        .iter()
        .find(|category| category.id == parent_id)
        .expect("parent Category should be projected");
    assert_eq!(parent.requested_locale, "ar");
    assert_eq!(parent.effective_locale, "en");
    assert_eq!(parent.available_locales, vec!["en"]);
    assert_eq!(parent.name, "General");
    assert_eq!(parent.description.as_deref(), Some("General discussion"));
    assert_eq!(parent.parent_id, None);
    assert_eq!(parent.position, 0);
    assert_eq!(parent.icon_key, None);
    assert_eq!(parent.color, None);
    assert_eq!(parent.presentation_revision, 0);

    assert!(!categories.iter().any(|category| category.id == tag_id));
    assert!(
        !categories
            .iter()
            .any(|category| category.id == foreign_category_id)
    );
}

#[tokio::test]
async fn category_owner_reader_keeps_empty_identity_page_empty() {
    let (db, _service) = setup().await;
    let reader = TaxonomyOwnerCategoryReader::new(db);

    let categories = reader
        .load_scoped_categories(
            Uuid::new_v4(),
            TaxonomyScopeType::Module,
            Some("forum"),
            Some(&[]),
            "en",
            None,
        )
        .await
        .expect("empty Category identity page should be accepted");

    assert!(categories.is_empty());
}
