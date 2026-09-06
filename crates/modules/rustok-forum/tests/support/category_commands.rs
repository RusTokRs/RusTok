use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumError, MoveCategoryInput,
    ReorderCategorySiblingsInput, UpdateCategoryInput,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::{TestResult, test_error};

pub async fn exercise_category_commands(db: &DatabaseConnection) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let root_a = seed_category(db, tenant_id, None, 0).await?;
    let root_b = seed_category(db, tenant_id, None, 1).await?;
    let root_c = seed_category(db, tenant_id, None, 2).await?;
    let child = seed_category(db, tenant_id, Some(root_a), 0).await?;
    let foreign_root = seed_category(db, foreign_tenant_id, None, 0).await?;

    let service = CategoryService::new(db.clone());
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let reordered = service
        .reorder_siblings(
            tenant_id,
            security.clone(),
            ReorderCategorySiblingsInput {
                parent_id: None,
                ordered_category_ids: vec![root_c, root_a, root_b],
            },
        )
        .await?;
    assert_eq!(
        reordered
            .siblings
            .iter()
            .map(|placement| (placement.id, placement.position))
            .collect::<Vec<_>>(),
        vec![(root_c, 0), (root_a, 1), (root_b, 2)]
    );

    let moved = service
        .move_category(
            tenant_id,
            root_b,
            security.clone(),
            MoveCategoryInput {
                parent_id: Some(root_a),
                position: 0,
            },
        )
        .await?;
    assert_eq!(moved.moved.parent_id, Some(root_a));
    assert_eq!(moved.moved.position, 0);
    assert_placement(db, tenant_id, root_c, None, 0).await?;
    assert_placement(db, tenant_id, root_a, None, 1).await?;
    assert_placement(db, tenant_id, root_b, Some(root_a), 0).await?;
    assert_placement(db, tenant_id, child, Some(root_a), 1).await?;

    assert_validation_contains(
        service
            .move_category(
                tenant_id,
                root_a,
                security.clone(),
                MoveCategoryInput {
                    parent_id: Some(child),
                    position: 0,
                },
            )
            .await,
        "cycle",
    )?;
    assert_validation_contains(
        service
            .move_category(
                tenant_id,
                root_c,
                security.clone(),
                MoveCategoryInput {
                    parent_id: Some(foreign_root),
                    position: 0,
                },
            )
            .await,
        "does not exist in the tenant",
    )?;
    assert_validation_contains(
        service
            .reorder_siblings(
                tenant_id,
                security.clone(),
                ReorderCategorySiblingsInput {
                    parent_id: Some(root_a),
                    ordered_category_ids: vec![root_b],
                },
            )
            .await,
        "every direct child exactly once",
    )?;

    assert_validation_contains(
        service
            .update(
                tenant_id,
                root_a,
                security.clone(),
                UpdateCategoryInput {
                    locale: "en".to_string(),
                    position: Some(99),
                    ..Default::default()
                },
            )
            .await,
        "move/reorder",
    )?;
    assert_placement(db, tenant_id, root_a, None, 1).await?;

    assert_error_contains(
        service
            .create(
                tenant_id,
                security.clone(),
                create_category_input("foreign-parent", Some(foreign_root)),
            )
            .await,
        "Category not found",
    )?;

    let mut deepest_parent = child;
    for depth in 2..=16 {
        deepest_parent = seed_category(db, tenant_id, Some(deepest_parent), depth).await?;
    }
    assert_error_contains(
        service
            .create(
                tenant_id,
                security,
                create_category_input("too-deep", Some(deepest_parent)),
            )
            .await,
        "depth 16",
    )?;

    assert_placement(db, foreign_tenant_id, foreign_root, None, 0).await?;
    Ok(())
}

fn create_category_input(slug: &str, parent_id: Option<Uuid>) -> CreateCategoryInput {
    CreateCategoryInput {
        locale: "en".to_string(),
        name: slug.replace('-', " "),
        slug: slug.to_string(),
        description: None,
        icon: None,
        color: None,
        parent_id,
        position: Some(0),
        moderated: false,
    }
}

async fn seed_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    position: i32,
) -> TestResult<Uuid> {
    let service = CategoryService::new(db.clone());
    let security = SecurityContext::system();
    let slug = format!("cat-{}", Uuid::new_v4().simple());
    let category = service
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".to_string(),
                name: slug.clone(),
                slug,
                description: None,
                icon: None,
                color: None,
                parent_id,
                position: Some(position),
                moderated: false,
            },
        )
        .await?;
    Ok(category.id)
}

async fn assert_placement(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    expected_parent_id: Option<Uuid>,
    expected_position: i32,
) -> TestResult<()> {
    use rustok_forum::entities::forum_category;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let model = forum_category::Entity::find_by_id(category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| test_error(format!("missing category {category_id}")))?;
    assert_eq!(model.parent_id, expected_parent_id);
    assert_eq!(model.position, expected_position);
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

fn assert_error_contains<T>(result: Result<T, ForumError>, expected: &str) -> TestResult<()> {
    match result {
        Err(error) => {
            let debug_repr = format!("{error:?}");
            let display_repr = error.to_string();
            if debug_repr.contains(expected) || display_repr.contains(expected) {
                Ok(())
            } else {
                Err(test_error(format!(
                    "expected error containing {expected:?}, got {debug_repr}"
                )))
            }
        }
        Ok(_) => Err(test_error(format!(
            "expected error containing {expected:?}, got success"
        ))),
    }
}
