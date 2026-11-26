use super::*;
use rustok_core::{MigrationSource, ModuleRegistry, ModuleRuntimeExtensions};
use rustok_taxonomy::{TaxonomyModule, sync_module_category_in_tx};
use rustok_test_utils::db::setup_test_db;
use sea_orm::TransactionTrait;
use sea_orm_migration::prelude::SchemaManager;

async fn setup() -> (DatabaseConnection, Arc<dyn ForumTaxonomyPort>) {
    let db = setup_test_db().await;
    let manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await.expect("Taxonomy migration");
    }
    let port = ServerForumTaxonomyPort::shared(db.clone());
    (db, port)
}

fn category(
    id: Uuid,
    parent: Option<Uuid>,
    slug: &str,
    position: i32,
) -> ForumTaxonomySyncCategoryInput {
    ForumTaxonomySyncCategoryInput {
        category_id: id,
        parent_id: parent,
        position,
        icon_key: Some("life-buoy".to_owned()),
        color: Some("#aabbcc".to_owned()),
        locale: "en".to_owned(),
        name: slug.to_owned(),
        slug: slug.to_owned(),
        description: Some("Canonical copy".to_owned()),
    }
}

#[tokio::test]
async fn real_owner_sync_route_alias_and_tags_share_transaction_and_roll_back() {
    let (db, port) = setup().await;
    let tenant = Uuid::new_v4();
    let root = Uuid::new_v4();
    let child = Uuid::new_v4();
    let txn = db.begin().await.unwrap();
    port.lock_category_hierarchy_writer_in_tx(&txn, tenant)
        .await
        .unwrap();
    port.sync_category_copy_in_tx(&txn, tenant, category(root, None, "general", 0))
        .await
        .unwrap();
    port.sync_category_copy_in_tx(&txn, tenant, category(child, Some(root), "help", 0))
        .await
        .unwrap();
    // Both route-key and alias provenance must use this uncommitted transaction.
    port.sync_category_copy_in_tx(&txn, tenant, category(child, Some(root), "support", 0))
        .await
        .unwrap();
    let parents = port
        .load_category_parents((&txn).into(), tenant, &[root, child])
        .await
        .unwrap();
    assert_eq!(parents, HashMap::from([(root, None), (child, Some(root))]));
    let snapshots = port
        .load_scoped_categories((&txn).into(), tenant, Some(&[child]), "en", None)
        .await
        .unwrap();
    assert_eq!(snapshots.len(), 1);
    let copy = &snapshots[0];
    assert_eq!(copy.slug, "support");
    assert_eq!(copy.parent_id, Some(root));
    assert_eq!(copy.canonical_key, format!("forum-category-{child}"));
    assert_eq!(copy.icon_key.as_deref(), Some("life-buoy"));
    assert_eq!(copy.color.as_deref(), Some("#aabbcc"));
    assert_eq!(
        port.load_category_locale_copy_in_tx(&txn, tenant, child, "en")
            .await
            .unwrap()
            .unwrap()
            .slug,
        "support"
    );
    assert!(
        port.load_category_locale_copy_in_tx(&txn, tenant, child, "fr")
            .await
            .unwrap()
            .is_none()
    );
    let alias = port
        .resolve_category_route((&txn).into(), tenant, "en", "help", None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(alias.term_id, child);
    assert!(alias.alias_id.is_some());
    let canonical = port
        .resolve_category_route((&txn).into(), tenant, "fr", "support", Some("en"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(canonical.matched_locale, "en");
    assert!(canonical.alias_id.is_none());
    let tags = port
        .ensure_tags_in_tx(&txn, tenant, "en", &["Rust".to_owned(), "Rust".to_owned()])
        .await
        .unwrap();
    assert_eq!(tags.len(), 1);
    let names = port
        .resolve_term_names((&txn).into(), tenant, &tags, "en", None)
        .await
        .unwrap();
    assert_eq!(names.get(&tags[0]).map(String::as_str), Some("Rust"));
    txn.rollback().await.unwrap();
    assert!(
        port.resolve_category_route((&db).into(), tenant, "en", "support", None)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        port.resolve_category_route((&db).into(), tenant, "en", "help", None)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        port.load_scoped_categories((&db).into(), tenant, Some(&[child]), "en", None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        port.resolve_term_names((&db).into(), tenant, &tags, "en", None)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn committed_projection_keeps_hierarchy_scope_tenant_and_empty_selection() {
    let (db, port) = setup().await;
    let tenant = Uuid::new_v4();
    let other_tenant = Uuid::new_v4();
    let id = Uuid::new_v4();
    let txn = db.begin().await.unwrap();
    port.sync_category_copy_in_tx(&txn, tenant, category(id, None, "support", 0))
        .await
        .unwrap();
    // Another module's category must not be accepted as a Forum parent or route.
    let other = Uuid::new_v4();
    sync_module_category_in_tx(
        &txn,
        tenant,
        SyncModuleCategoryInput {
            category_id: other,
            module_scope: "blog".to_owned(),
            canonical_key: "news".to_owned(),
            locale: "en".to_owned(),
            name: "News".to_owned(),
            slug: "news".to_owned(),
            aliases: vec![],
            description: None,
            parent_id: None,
            position: 0,
            icon_key: None,
            color: None,
        },
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();
    assert_eq!(
        port.load_category_parents((&db).into(), tenant, &[id])
            .await
            .unwrap(),
        HashMap::from([(id, None)])
    );
    assert!(
        port.load_category_parents((&db).into(), other_tenant, &[id])
            .await
            .is_err()
    );
    assert!(
        port.load_category_parents((&db).into(), tenant, &[other])
            .await
            .is_err()
    );
    assert!(
        port.load_category_parents((&db).into(), tenant, &[id, id])
            .await
            .is_err()
    );
    assert!(
        port.load_category_parents((&db).into(), tenant, &[Uuid::new_v4()])
            .await
            .is_err()
    );
    assert!(
        port.load_category_parents((&db).into(), tenant, &[])
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        port.load_scoped_categories((&db).into(), tenant, Some(&[]), "en", None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        port.resolve_category_route((&db).into(), other_tenant, "en", "support", None)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        port.resolve_category_route((&db).into(), tenant, "en", "news", None)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        port.resolve_category_route((&db).into(), tenant, "en", "support", None)
            .await
            .unwrap()
            .unwrap()
            .term_id,
        id
    );
}

#[tokio::test]
async fn hierarchy_mutations_delegate_to_owner_and_reject_cycle() {
    let (db, port) = setup().await;
    let tenant = Uuid::new_v4();
    let root = Uuid::new_v4();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let txn = db.begin().await.unwrap();
    for input in [
        category(root, None, "root", 0),
        category(first, Some(root), "first", 0),
        category(second, Some(root), "second", 1),
    ] {
        port.sync_category_copy_in_tx(&txn, tenant, input)
            .await
            .unwrap();
    }
    port.reorder_category_siblings_in_tx(&txn, tenant, Some(root), &[second, first])
        .await
        .unwrap();
    assert_eq!(
        port.load_category_siblings_in_tx(&txn, tenant, Some(root))
            .await
            .unwrap(),
        vec![second, first]
    );
    let inserted = Uuid::new_v4();
    port.sync_category_copy_in_tx(&txn, tenant, category(inserted, Some(root), "inserted", 0))
        .await
        .unwrap();
    port.shift_category_siblings_for_insert_in_tx(&txn, tenant, inserted, Some(root), 0)
        .await
        .unwrap();
    assert_eq!(
        port.load_category_siblings_in_tx(&txn, tenant, Some(root))
            .await
            .unwrap(),
        vec![inserted, second, first]
    );
    let placements = port
        .move_category_in_tx(&txn, tenant, second, None, 1)
        .await
        .unwrap();
    assert!(
        placements
            .iter()
            .any(|p| p.term_id == second && p.parent_id.is_none())
    );
    assert!(
        port.move_category_in_tx(&txn, tenant, root, Some(first), 0)
            .await
            .is_err()
    );
    txn.rollback().await.unwrap();
}

#[tokio::test]
async fn configured_adapter_rejects_missing_connection_and_propagates_storage_failure() {
    let db = setup_test_db().await; // Deliberately no Taxonomy schema.
    let port = ServerForumTaxonomyPort::shared(db.clone());
    let tenant = Uuid::new_v4();
    let error = port
        .resolve_category_route(ForumTaxonomyConnection::None, tenant, "en", "help", None)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        ForumError::CapabilityUnavailable {
            code: "FORUM_TAXONOMY_CONNECTION_REQUIRED",
            ..
        }
    ));
    assert!(matches!(
        port.resolve_category_route((&db).into(), tenant, "en", "help", None)
            .await,
        Err(ForumError::Database(_))
    ));
    assert!(matches!(
        port.load_category_parents((&db).into(), tenant, &[Uuid::new_v4()])
            .await,
        Err(ForumError::Database(_))
    ));
}

#[tokio::test]
async fn host_registration_selects_only_joint_distribution() {
    let db = setup_test_db().await;
    for (registry, expected) in [
        (ModuleRegistry::new(), false),
        (
            ModuleRegistry::new().register(rustok_forum::ForumModule),
            false,
        ),
        (ModuleRegistry::new().register(TaxonomyModule), false),
        (
            ModuleRegistry::new()
                .register(rustok_forum::ForumModule)
                .register(TaxonomyModule),
            true,
        ),
    ] {
        let mut extensions = ModuleRuntimeExtensions::default();
        register(&mut extensions, &registry, db.clone());
        assert_eq!(
            extensions.get::<Arc<dyn ForumTaxonomyPort>>().is_some(),
            expected
        );
    }
}

#[test]
fn provider_failures_keep_classification_and_redaction() {
    assert!(matches!(
        map_error(TaxonomyError::Validation("bad placement".into())),
        ForumError::Validation(_)
    ));
    assert!(matches!(
        map_error(TaxonomyError::Forbidden("denied".into())),
        ForumError::Forbidden(_)
    ));
    let error = map_error(TaxonomyError::Internal("postgres://private/secret".into()));
    match error {
        ForumError::CapabilityFailure {
            source_code,
            message,
            retryable,
            ..
        } => {
            assert_eq!(source_code, "taxonomy.internal");
            assert!(!message.contains("private"));
            assert!(!retryable);
        }
        _ => panic!("provider error must not turn into route absence"),
    }
    assert!(matches!(
        map_error(TaxonomyError::TermNotFound(Uuid::new_v4())),
        ForumError::CapabilityFailure { .. }
    ));
}
