use rustok_core::MigrationSource;
use rustok_product::CommerceError;
use rustok_product::ports::ProductTaxonomyCategoryTranslationInput;
use rustok_taxonomy::{
    SyncModuleCategoryInput, TaxonomyModule, TaxonomyOwnerCategoryReader, TaxonomyScopeType,
    TaxonomyService, TaxonomyTermKind,
    entities::{taxonomy_category_presentation, taxonomy_term_alias, translation_change},
    in_process_product_taxonomy_port, sync_module_category_in_tx,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
};
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

async fn setup() -> sea_orm::DatabaseConnection {
    let db = setup_test_db().await;
    let schema_manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema_manager)
            .await
            .expect("failed to run taxonomy migration");
    }
    db
}

#[tokio::test]
async fn product_category_copy_preserves_taxonomy_presentation_and_routes() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let canonical_key = format!("product-category-{category_id}");
    let mut initial = snapshot(
        category_id,
        &canonical_key,
        "en",
        "Shoes",
        "shoes",
        &["boots"],
        None,
        0,
        Some("headphones"),
        Some("#123456"),
    );
    initial.module_scope = "product".into();
    let txn = db.begin().await.unwrap();
    sync_module_category_in_tx(&txn, tenant_id, initial)
        .await
        .unwrap();
    txn.commit().await.unwrap();

    let presentation = taxonomy_category_presentation::Entity::find_by_id((tenant_id, category_id));
    let before = presentation.clone().one(&db).await.unwrap().unwrap();
    let port = in_process_product_taxonomy_port(db.clone());
    let input = ProductTaxonomyCategoryTranslationInput {
        category_id,
        module_scope: "product".into(),
        canonical_key,
        locale: "en".into(),
        name: "Footwear".into(),
        slug: "footwear".into(),
        aliases: vec![],
        description: None,
        parent_id: None,
        position: 0,
    };
    let txn = db.begin().await.unwrap();
    port.sync_category_translation_in_tx(&txn, tenant_id, input.clone())
        .await
        .unwrap();
    txn.commit().await.unwrap();

    let txn = db.begin().await.unwrap();
    let reclaim = port
        .sync_category_translation_in_tx(
            &txn,
            tenant_id,
            ProductTaxonomyCategoryTranslationInput {
                slug: "shoes".into(),
                ..input
            },
        )
        .await;
    assert!(matches!(reclaim, Err(CommerceError::Conflict(_))));
    txn.rollback().await.unwrap();

    assert_eq!(presentation.one(&db).await.unwrap(), Some(before));
    let service = TaxonomyService::new(db.clone());
    for (slug, is_alias) in [("boots", true), ("footwear", false), ("shoes", true)] {
        let route = service
            .resolve_term_route_for_module(
                tenant_id,
                TaxonomyTermKind::Category,
                "product",
                "en",
                None,
                slug,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(route.term_id, category_id);
        assert_eq!(route.alias_id.is_some(), is_alias);
    }
    let categories = TaxonomyOwnerCategoryReader::new(db)
        .load_scoped_categories(
            tenant_id,
            TaxonomyScopeType::Module,
            Some("product"),
            Some(&[category_id]),
            "en",
            None,
        )
        .await
        .unwrap();
    assert_eq!(categories.len(), 1);
    assert_eq!(categories[0].slug, "footwear");
    assert_eq!(categories[0].name, "Footwear");
}

fn snapshot(
    category_id: Uuid,
    canonical_key: &str,
    locale: &str,
    name: &str,
    slug: &str,
    aliases: &[&str],
    parent_id: Option<Uuid>,
    position: i32,
    icon_key: Option<&str>,
    color: Option<&str>,
) -> SyncModuleCategoryInput {
    SyncModuleCategoryInput {
        category_id,
        module_scope: "forum".to_owned(),
        canonical_key: canonical_key.to_owned(),
        locale: locale.to_owned(),
        name: name.to_owned(),
        slug: slug.to_owned(),
        aliases: aliases.iter().map(|value| (*value).to_owned()).collect(),
        description: Some(format!("{name} description")),
        parent_id,
        position,
        icon_key: icon_key.map(ToOwned::to_owned),
        color: color.map(ToOwned::to_owned),
    }
}

#[tokio::test]
async fn owner_sync_creates_updates_and_replays_module_category_atomically() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let child_id = Uuid::new_v4();

    let txn = db.begin().await.expect("sync transaction should start");
    let parent = sync_module_category_in_tx(
        &txn,
        tenant_id,
        snapshot(
            parent_id,
            &format!("forum-category-{parent_id}"),
            "en",
            "General",
            "general",
            &[],
            None,
            0,
            None,
            None,
        ),
    )
    .await
    .expect("parent Category should sync");
    let child = sync_module_category_in_tx(
        &txn,
        tenant_id,
        snapshot(
            child_id,
            &format!("forum-category-{child_id}"),
            "en",
            "Support",
            "support",
            &[],
            Some(parent_id),
            4,
            Some("life-buoy"),
            Some("#F0A"),
        ),
    )
    .await
    .expect("child Category should sync");
    txn.commit().await.expect("initial sync should commit");

    assert_eq!(parent.resource_revision, 1);
    assert_eq!(parent.translation_revision, 1);
    assert_eq!(parent.presentation_revision, 0);
    assert_eq!(child.resource_revision, 1);
    assert_eq!(child.translation_revision, 1);
    assert_eq!(child.presentation_revision, 1);

    // Simulate Media-owned canonical presentation added outside the legacy Forum
    // compatibility surface. Owner sync may update icon/color, but must preserve
    // these typed Media identities and their independent presentation revision.
    let image_media_id = Uuid::new_v4();
    let cover_media_id = Uuid::new_v4();
    let existing = taxonomy_category_presentation::Entity::find_by_id((tenant_id, child_id))
        .one(&db)
        .await
        .expect("presentation lookup")
        .expect("presentation should exist");
    let mut presentation: taxonomy_category_presentation::ActiveModel = existing.into();
    presentation.image_media_id = Set(Some(image_media_id));
    presentation.cover_media_id = Set(Some(cover_media_id));
    presentation.revision = Set(2);
    presentation
        .update(&db)
        .await
        .expect("test Media presentation seed should update");

    let updated_snapshot = snapshot(
        child_id,
        &format!("forum-category-{child_id}"),
        "en",
        "Help & Support",
        "help-support",
        &["support"],
        Some(parent_id),
        2,
        Some("headphones"),
        Some("#123456"),
    );
    let txn = db.begin().await.expect("update transaction should start");
    let updated = sync_module_category_in_tx(&txn, tenant_id, updated_snapshot.clone())
        .await
        .expect("updated Category should sync");
    txn.commit().await.expect("update sync should commit");

    assert_eq!(updated.resource_revision, 2);
    assert_eq!(updated.translation_revision, 2);
    assert_eq!(updated.presentation_revision, 3);

    let reader = TaxonomyOwnerCategoryReader::new(db.clone());
    let projected = reader
        .load_scoped_categories(
            tenant_id,
            TaxonomyScopeType::Module,
            Some("forum"),
            Some(&[child_id]),
            "en",
            None,
        )
        .await
        .expect("owner projection should load")
        .pop()
        .expect("child Category projection");
    assert_eq!(projected.name, "Help & Support");
    assert_eq!(projected.slug, "help-support");
    assert_eq!(projected.parent_id, Some(parent_id));
    assert_eq!(projected.position, 2);
    assert_eq!(projected.icon_key.as_deref(), Some("headphones"));
    assert_eq!(projected.color.as_deref(), Some("#123456"));
    assert_eq!(
        projected.image_media_id.map(Into::<Uuid>::into),
        Some(image_media_id)
    );
    assert_eq!(
        projected.cover_media_id.map(Into::<Uuid>::into),
        Some(cover_media_id)
    );
    assert_eq!(projected.presentation_revision, 3);

    let aliases = taxonomy_term_alias::Entity::find()
        .filter(taxonomy_term_alias::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_alias::Column::TermId.eq(child_id))
        .all(&db)
        .await
        .expect("aliases should load");
    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].slug, "support");

    let evidence_before_replay = translation_change::Entity::find()
        .filter(translation_change::Column::TenantId.eq(tenant_id))
        .filter(translation_change::Column::TermId.eq(child_id))
        .all(&db)
        .await
        .expect("change evidence should load")
        .len();

    let txn = db.begin().await.expect("replay transaction should start");
    let replayed = sync_module_category_in_tx(&txn, tenant_id, updated_snapshot)
        .await
        .expect("identical Category snapshot should replay");
    txn.commit().await.expect("replay should commit");
    assert_eq!(replayed.resource_revision, 2);
    assert_eq!(replayed.translation_revision, 2);
    assert_eq!(replayed.presentation_revision, 3);

    let evidence_after_replay = translation_change::Entity::find()
        .filter(translation_change::Column::TenantId.eq(tenant_id))
        .filter(translation_change::Column::TermId.eq(child_id))
        .all(&db)
        .await
        .expect("change evidence should load")
        .len();
    assert_eq!(evidence_after_replay, evidence_before_replay);
}

#[tokio::test]
async fn owner_sync_rejects_alias_removal_and_cross_scope_parent() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let other_scope_parent = Uuid::new_v4();

    let txn = db.begin().await.expect("sync transaction should start");
    sync_module_category_in_tx(
        &txn,
        tenant_id,
        snapshot(
            category_id,
            &format!("forum-category-{category_id}"),
            "en",
            "Support",
            "help",
            &["support"],
            None,
            0,
            None,
            None,
        ),
    )
    .await
    .expect("Category with historical alias should sync");
    sync_module_category_in_tx(
        &txn,
        tenant_id,
        SyncModuleCategoryInput {
            module_scope: "blog".to_owned(),
            category_id: other_scope_parent,
            canonical_key: format!("blog-category-{other_scope_parent}"),
            locale: "en".to_owned(),
            name: "Blog Parent".to_owned(),
            slug: "blog-parent".to_owned(),
            aliases: vec![],
            description: None,
            parent_id: None,
            position: 0,
            icon_key: None,
            color: None,
        },
    )
    .await
    .expect("other-scope Category should sync");
    txn.commit().await.expect("seed sync should commit");

    let txn = db.begin().await.expect("alias-removal transaction");
    let alias_error = sync_module_category_in_tx(
        &txn,
        tenant_id,
        snapshot(
            category_id,
            &format!("forum-category-{category_id}"),
            "en",
            "Support",
            "help",
            &[],
            None,
            0,
            None,
            None,
        ),
    )
    .await
    .expect_err("append-only alias removal must fail");
    assert!(alias_error.to_string().contains("append-only"));
    txn.rollback().await.expect("failed sync should roll back");

    let txn = db.begin().await.expect("parent transaction");
    let parent_error = sync_module_category_in_tx(
        &txn,
        tenant_id,
        snapshot(
            category_id,
            &format!("forum-category-{category_id}"),
            "en",
            "Support",
            "help",
            &["support"],
            Some(other_scope_parent),
            0,
            None,
            None,
        ),
    )
    .await
    .expect_err("cross-scope parent must fail");
    assert!(
        parent_error
            .to_string()
            .contains("same tenant and module scope")
    );
    txn.rollback()
        .await
        .expect("failed parent sync should roll back");
}
