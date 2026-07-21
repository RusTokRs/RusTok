use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CategoryTreeQuery, ForumError, MAX_FORUM_CATEGORY_TREE_DEPTH,
    MAX_FORUM_CATEGORY_TREE_NODES,
};
use sea_orm::{ConnectionTrait, DatabaseConnection};
use uuid::Uuid;

use super::{test_error, TestResult};

pub async fn exercise_category_tree_read_model(db: &DatabaseConnection) -> TestResult<()> {
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();

    let root_primary = seed_category(db, tenant_a, None, 0, "Primary", "primary", false).await?;
    seed_translation(db, tenant_a, root_primary, "ru", "Главная", "primary-ru").await?;
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

    let service = CategoryService::new(db.clone());
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
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
    let deterministic_category =
        seed_category_without_translation(db, deterministic_fallback_tenant).await?;
    seed_translation(
        db,
        deterministic_fallback_tenant,
        deterministic_category,
        "fr",
        "Français",
        "francais",
    )
    .await?;
    seed_translation(
        db,
        deterministic_fallback_tenant,
        deterministic_category,
        "de",
        "Deutsch",
        "deutsch",
    )
    .await?;
    let deterministic_fallback = service
        .tree(
            deterministic_fallback_tenant,
            security.clone(),
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

    let untranslated_tenant = Uuid::new_v4();
    seed_category_without_translation(db, untranslated_tenant).await?;
    let untranslated_error = service
        .tree(
            untranslated_tenant,
            security.clone(),
            CategoryTreeQuery {
                locale: Some("en".to_string()),
                ..Default::default()
            },
        )
        .await;
    assert_validation_contains(untranslated_error, "no localized translation")?;

    let deep_tenant = Uuid::new_v4();
    seed_deep_tree(db, deep_tenant, MAX_FORUM_CATEGORY_TREE_DEPTH + 2).await?;
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
    let mut sql = String::new();
    for position in 0..=MAX_FORUM_CATEGORY_TREE_NODES {
        let category_id = Uuid::new_v4();
        sql.push_str(&format!(
            "INSERT INTO forum_categories \
                (id, tenant_id, position, moderated, topic_count, reply_count) \
             VALUES ('{category_id}', '{tenant_id}', {position}, FALSE, 0, 0);"
        ));
    }
    db.execute_unprepared(&sql).await?;
    Ok(())
}

async fn seed_category_without_translation(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Uuid> {
    let category_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO forum_categories
            (id, tenant_id, position, moderated, topic_count, reply_count)
         VALUES
            ('{category_id}', '{tenant_id}', 0, FALSE, 0, 0);"
    ))
    .await?;
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
    let category_id = Uuid::new_v4();
    let parent_sql = parent_id
        .map(|parent_id| format!("'{parent_id}'"))
        .unwrap_or_else(|| "NULL".to_string());
    let moderated_sql = if moderated { "TRUE" } else { "FALSE" };
    let translation_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO forum_categories
            (id, tenant_id, parent_id, position, moderated, topic_count, reply_count)
         VALUES
            ('{category_id}', '{tenant_id}', {parent_sql}, {position}, {moderated_sql}, 0, 0);
         INSERT INTO forum_category_translations
            (id, category_id, tenant_id, locale, name, slug)
         VALUES
            ('{translation_id}', '{category_id}', '{tenant_id}', 'en', '{name}', '{slug}');"
    ))
    .await?;
    Ok(category_id)
}

async fn seed_translation(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    locale: &str,
    name: &str,
    slug: &str,
) -> TestResult<()> {
    let translation_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO forum_category_translations
            (id, category_id, tenant_id, locale, name, slug)
         VALUES
            ('{translation_id}', '{category_id}', '{tenant_id}', '{locale}', '{name}', '{slug}');"
    ))
    .await?;
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
