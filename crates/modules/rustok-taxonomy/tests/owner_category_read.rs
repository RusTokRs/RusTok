use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, SetTaxonomyCategoryPlacementInput,
    SetTaxonomyCategoryPresentationInput, TaxonomyModule, TaxonomyOwnerCategoryReader,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind, UpdateTaxonomyTermInput,
};
use rustok_test_utils::db::setup_test_db;
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
    service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind,
                scope_type: TaxonomyScopeType::Module,
                scope_value: Some(scope_value.to_owned()),
                locale: "en".to_owned(),
                name: name.to_owned(),
                slug: Some(slug.to_owned()),
                canonical_key: Some(slug.to_owned()),
                description: description.map(ToOwned::to_owned),
                aliases: vec![],
            },
        )
        .await
        .expect("taxonomy term should be created")
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

    service
        .update_term(
            tenant_id,
            child_id,
            admin(),
            UpdateTaxonomyTermInput {
                locale: "ar".to_owned(),
                name: Some("الدعم".to_owned()),
                slug: Some("support-ar".to_owned()),
                description: Some("الدعم بالعربية".to_owned()),
                aliases: None,
            },
        )
        .await
        .expect("Arabic category copy should be added");
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
