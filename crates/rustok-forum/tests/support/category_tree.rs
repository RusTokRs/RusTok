use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CategoryTreeQuery, ForumError, MAX_FORUM_CATEGORY_TREE_DEPTH,
    MAX_FORUM_CATEGORY_TREE_NODES,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use uuid::Uuid;

use super::{TestResult, test_error};

pub async fn exercise_category_tree_read_model(db: &DatabaseConnection) -> TestResult<()> {
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();

    let service = CategoryService::new(db.clone());
    let admin_user_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
        [admin_user_id.into(), tenant_a.into()],
    ))
    .await?;
    let security = SecurityContext::new(UserRole::Admin, Some(admin_user_id));

    let root_primary = seed_category(db, tenant_a, None, 0, "Primary", "primary", false).await?;
    service
        .update(
            tenant_a,
            root_primary,
            security.clone(),
            rustok_forum::UpdateCategoryInput {
                locale: "ru".to_string(),
                name: Some("Главная".to_string()),
                slug: Some("primary-ru".to_string()),
                ..Default::default()
            },
        )
        .await?;
    let root_secondary =
        seed_category(db, tenant_a, None, 10, "Secondary", "secondary", true).await?;
    let child_later = seed_category(
        db,
        tenant_a,
        Some(root_primary),
        20,
        "Later child",
        "later-child",
        false,
    )
    .await?;
    let child_first = seed_category(
        db,
        tenant_a,
        Some(root_primary),
        10,
        "First child",
        "first-child",
        true,
    )
    .await?;
    let grandchild = seed_category(
        db,
        tenant_a,
        Some(child_first),
        0,
        "Grandchild",
        "grandchild",
        false,
    )
    .await?;
    let foreign_root = seed_category(db, tenant_b, None, 0, "Foreign", "primary", false).await?;
    let tree = service
        .tree(
            tenant_a,
            security.clone(),
            CategoryTreeQuery {
                locale: Some("ru".to_string()),
                fallback_locale: Some("en".to_string()),
            },
        )
        .await?;

    assert_eq!(tree.total_nodes, 5);
    assert_eq!(tree.max_depth, 2);
    assert_eq!(tree.roots.len(), 2);
    assert_eq!(tree.roots[0].id, root_primary);
    assert_eq!(tree.roots[1].id, root_secondary);
    assert!(tree.roots.iter().all(|node| node.id != foreign_root));

    let primary = &tree.roots[0];
    assert_eq!(primary.depth, 0);
    assert_eq!(primary.effective_locale, "ru");
    assert_eq!(primary.name, "Главная");
    assert!(primary.has_children);
    assert_eq!(primary.children_count, 2);
    assert_eq!(primary.children.len(), 2);
    assert_eq!(primary.children[0].id, child_first);
    assert_eq!(primary.children[1].id, child_later);
    assert_eq!(primary.breadcrumbs.len(), 1);
    assert_eq!(primary.breadcrumbs[0].id, root_primary);

    let first_child = &primary.children[0];
    assert_eq!(first_child.parent_id, Some(root_primary));
    assert_eq!(first_child.depth, 1);
    assert_eq!(first_child.position, 10);
    assert_eq!(first_child.effective_locale, "en");
    assert!(first_child.moderated);
    assert_eq!(first_child.children_count, 1);
    assert_eq!(first_child.breadcrumbs.len(), 2);
    assert_eq!(first_child.breadcrumbs[0].id, root_primary);
    assert_eq!(first_child.breadcrumbs[1].id, child_first);

    let nested = &first_child.children[0];
    assert_eq!(nested.id, grandchild);
    assert_eq!(nested.depth, 2);
    assert!(!nested.has_children);
    assert_eq!(nested.children_count, 0);
    assert_eq!(nested.breadcrumbs.len(), 3);
    assert_eq!(nested.breadcrumbs[2].id, grandchild);

    let empty = service
        .tree(
            Uuid::new_v4(),
            security.clone(),
            CategoryTreeQuery::default(),
        )
        .await?;
    assert!(empty.roots.is_empty());
    assert_eq!(empty.total_nodes, 0);

    let deterministic_fallback_tenant = Uuid::new_v4();
    let security_deterministic = SecurityContext::system();
    let deterministic_category = service
        .create(
            deterministic_fallback_tenant,
            security_deterministic.clone(),
            rustok_forum::CreateCategoryInput {
                locale: "de".to_string(),
                name: "Deutsch".to_string(),
                slug: "deutsch".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?
        .id;
    service
        .update(
            deterministic_fallback_tenant,
            deterministic_category,
            security_deterministic.clone(),
            rustok_forum::UpdateCategoryInput {
                locale: "fr".to_string(),
                name: Some("Français".to_string()),
                slug: Some("francais".to_string()),
                ..Default::default()
            },
        )
        .await?;
    let deterministic_fallback = service
        .tree(
            deterministic_fallback_tenant,
            security_deterministic,
            CategoryTreeQuery {
                locale: Some("zh".to_string()),
                fallback_locale: None,
            },
        )
        .await?;
    assert_eq!(deterministic_fallback.roots.len(), 1);
    assert_eq!(deterministic_fallback.roots[0].effective_locale, "de");
    assert_eq!(
        deterministic_fallback.roots[0].available_locales,
        vec!["de".to_string(), "fr".to_string()]
    );

    let unbound_tenant = Uuid::new_v4();
    let unbound_category_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO forum_categories \
            (id, tenant_id, position, moderated, topic_count, reply_count) \
         VALUES (?, ?, 0, FALSE, 0, 0)",
        [unbound_category_id.into(), unbound_tenant.into()],
    ))
    .await?;
    let unbound_error = service
        .tree(
            unbound_tenant,
            security.clone(),
            CategoryTreeQuery {
                locale: Some("en".to_string()),
                ..Default::default()
            },
        )
        .await;
    assert_validation_contains(unbound_error, "Taxonomy Category binding")?;

    let deep_tenant = Uuid::new_v4();
    let deep_seed = seed_deep_tree(db, deep_tenant, MAX_FORUM_CATEGORY_TREE_DEPTH + 2).await;
    match deep_seed {
        Ok(()) => {
            let depth_error = service
                .tree(
                    deep_tenant,
                    security.clone(),
                    CategoryTreeQuery {
                        locale: Some("en".to_string()),
                        ..Default::default()
                    },
                )
                .await;
            assert_validation_contains(depth_error, "maximum depth")?;
        }
        Err(err) => {
            let err_str = format!("{err:?}");
            if !err_str.contains("maximum depth") && !err_str.contains("exceeds maximum depth") {
                return Err(test_error(format!("unexpected deep tree error: {err_str}")));
            }
        }
    }

    let oversized_tenant = Uuid::new_v4();
    seed_oversized_tree(db, oversized_tenant).await?;
    let size_error = service
        .tree(
            oversized_tenant,
            security,
            CategoryTreeQuery {
                locale: Some("en".to_string()),
                ..Default::default()
            },
        )
        .await;
    assert_validation_contains(size_error, "bounded limit")?;

    Ok(())
}

async fn seed_deep_tree(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    node_count: usize,
) -> TestResult<()> {
    let mut parent_id = None;
    for position in 0..node_count {
        parent_id = Some(
            seed_category(
                db,
                tenant_id,
                parent_id,
                position as i32,
                &format!("Depth {position}"),
                &format!("depth-{position}"),
                false,
            )
            .await?,
        );
    }
    Ok(())
}

async fn seed_oversized_tree(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    for position in 0..=MAX_FORUM_CATEGORY_TREE_NODES {
        let category_id = Uuid::new_v4();
        db.execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO forum_categories \
                (id, tenant_id, position, moderated, topic_count, reply_count) \
             VALUES (?, ?, ?, FALSE, 0, 0)",
            [category_id.into(), tenant_id.into(), (position as i32).into()],
        ))
        .await?;
    }
    Ok(())
}

async fn seed_category_without_translation(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Uuid> {
    use rustok_forum::entities::{forum_category, forum_category_taxonomy_binding};
    use rustok_taxonomy::entities::taxonomy_term;
    use sea_orm::{ActiveModelTrait, Set};

    let category_id = Uuid::new_v4();
    let cat_model = forum_category::ActiveModel {
        id: Set(category_id),
        tenant_id: Set(tenant_id),
        parent_id: Set(None),
        position: Set(0),
        moderated: Set(false),
        topic_count: Set(0),
        reply_count: Set(0),
        ..Default::default()
    };
    cat_model.insert(db).await?;

    let binding = forum_category_taxonomy_binding::ActiveModel {
        tenant_id: Set(tenant_id),
        forum_category_id: Set(category_id),
        taxonomy_category_id: Set(category_id),
        ..Default::default()
    };
    binding.insert(db).await?;

    let term = taxonomy_term::ActiveModel {
        id: Set(category_id),
        tenant_id: Set(tenant_id),
        kind: Set(rustok_taxonomy::TaxonomyTermKind::Category),
        scope_type: Set(rustok_taxonomy::TaxonomyScopeType::Module),
        scope_value: Set("forum".to_string()),
        canonical_key: Set(format!("category-{category_id}")),
        ..Default::default()
    };
    term.insert(db).await?;

    Ok(category_id)
}

async fn seed_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    position: i32,
    name: &str,
    slug: &str,
    moderated: bool,
) -> TestResult<Uuid> {
    use rustok_forum::CreateCategoryInput;
    let service = CategoryService::new(db.clone());
    let security = SecurityContext::system();
    let category = service
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".to_string(),
                name: name.to_string(),
                slug: slug.to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id,
                position: Some(position),
                moderated,
            },
        )
        .await?;
    Ok(category.id)
}

async fn seed_translation(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    locale: &str,
    name: &str,
    slug: &str,
) -> TestResult<()> {
    use rustok_forum::entities::forum_category_translation;
    use sea_orm::{ActiveModelTrait, Set};

    let model = forum_category_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        category_id: Set(category_id),
        tenant_id: Set(tenant_id),
        locale: Set(locale.to_string()),
        name: Set(name.to_string()),
        slug: Set(slug.to_string()),
        description: Set(None),
    };
    model.insert(db).await?;
    Ok(())
}

fn assert_validation_contains<T>(result: Result<T, ForumError>, expected: &str) -> TestResult<()> {
    match result {
        Err(ForumError::Validation(message)) if message.contains(expected) => Ok(()),
        Err(error) => Err(test_error(format!(
            "expected validation containing {expected:?}, got {error}"
        ))),
        Ok(_) => Err(test_error(format!(
            "expected validation containing {expected:?}, got success"
        ))),
    }
}
