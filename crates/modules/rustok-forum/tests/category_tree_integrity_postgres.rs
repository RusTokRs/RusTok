mod support;

use uuid::Uuid;

use support::TestResult;
use support::postgres::PostgresForumTestDb;

#[tokio::test]
async fn postgres_rejects_self_parent_and_category_cycles() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("category_tree").await? else {
        return Ok(());
    };

    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let service = rustok_forum::CategoryService::new(context.db.clone());
        let security = rustok_core::SecurityContext::system();

        let root = service
            .create(
                tenant_id,
                security.clone(),
                rustok_forum::CreateCategoryInput {
                    name: "Root".to_string(),
                    slug: "root".to_string(),
                    locale: "en".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await?;

        let child = service
            .create(
                tenant_id,
                security.clone(),
                rustok_forum::CreateCategoryInput {
                    name: "Child".to_string(),
                    slug: "child".to_string(),
                    locale: "en".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: Some(root.id),
                    position: Some(0),
                    moderated: false,
                },
            )
            .await?;

        let grandchild = service
            .create(
                tenant_id,
                security.clone(),
                rustok_forum::CreateCategoryInput {
                    name: "Grandchild".to_string(),
                    slug: "grandchild".to_string(),
                    locale: "en".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: Some(child.id),
                    position: Some(0),
                    moderated: false,
                },
            )
            .await?;

        let self_parent_err = service
            .move_category(
                tenant_id,
                root.id,
                security.clone(),
                rustok_forum::MoveCategoryInput {
                    parent_id: Some(root.id),
                    position: 0,
                },
            )
            .await;
        assert!(
            self_parent_err.is_err(),
            "self-parent category must be rejected"
        );

        let cycle_err = service
            .move_category(
                tenant_id,
                root.id,
                security.clone(),
                rustok_forum::MoveCategoryInput {
                    parent_id: Some(grandchild.id),
                    position: 0,
                },
            )
            .await;
        assert!(
            cycle_err.is_err(),
            "three-level category cycle must be rejected"
        );

        service
            .move_category(
                tenant_id,
                grandchild.id,
                security.clone(),
                rustok_forum::MoveCategoryInput {
                    parent_id: Some(root.id),
                    position: 1,
                },
            )
            .await?;

        service
            .move_category(
                tenant_id,
                grandchild.id,
                security.clone(),
                rustok_forum::MoveCategoryInput {
                    parent_id: None,
                    position: 1,
                },
            )
            .await?;

        service
            .archive_subtree(tenant_id, root.id, security.clone())
            .await?;

        let move_under_archived_err = service
            .move_category(
                tenant_id,
                grandchild.id,
                security.clone(),
                rustok_forum::MoveCategoryInput {
                    parent_id: Some(root.id),
                    position: 0,
                },
            )
            .await;
        assert!(
            move_under_archived_err.is_err(),
            "moving active category under archived parent must be rejected"
        );

        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}
