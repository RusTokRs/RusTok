use chrono::Utc;
use rustok_comments::entities::{comment, comment_thread};
use rustok_comments::migrations;
use rustok_comments::{CommentStatus, CommentThreadStatus};
use sea_orm::{ActiveModelTrait, Database, EntityTrait, Set};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_database() -> sea_orm::DatabaseConnection {
    let database_url = format!(
        "sqlite:file:comments_thread_write_invariants_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let db = Database::connect(database_url)
        .await
        .expect("SQLite connection should succeed");
    let manager = SchemaManager::new(&db);
    for migration in migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("comments migration should apply");
    }
    db
}

#[tokio::test]
async fn active_model_hooks_override_stale_positions_and_counts() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let thread_id = Uuid::new_v4();
    let now = Utc::now();

    comment_thread::ActiveModel {
        id: Set(thread_id),
        tenant_id: Set(tenant_id),
        target_type: Set("blog_post".to_string()),
        target_id: Set(Uuid::new_v4()),
        status: Set(CommentThreadStatus::Open),
        comment_count: Set(0),
        last_commented_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .expect("thread should insert");

    let first = comment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        thread_id: Set(thread_id),
        author_id: Set(Uuid::new_v4()),
        parent_comment_id: Set(None),
        status: Set(CommentStatus::Approved),
        position: Set(99),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        deleted_at: Set(None),
    }
    .insert(&db)
    .await
    .expect("first comment should insert");
    assert_eq!(first.position, 1);

    let second = comment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        thread_id: Set(thread_id),
        author_id: Set(Uuid::new_v4()),
        parent_comment_id: Set(None),
        status: Set(CommentStatus::Approved),
        position: Set(-5),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        deleted_at: Set(None),
    }
    .insert(&db)
    .await
    .expect("second comment should insert");
    assert_eq!(second.position, 2);

    let mut deleted_first: comment::ActiveModel = first.into();
    deleted_first.deleted_at = Set(Some(Utc::now().into()));
    deleted_first.updated_at = Set(Utc::now().into());
    deleted_first
        .update(&db)
        .await
        .expect("first comment should soft-delete");

    let thread = comment_thread::Entity::find_by_id(thread_id)
        .one(&db)
        .await
        .expect("thread lookup should succeed")
        .expect("thread should exist");
    let mut stale_thread: comment_thread::ActiveModel = thread.into();
    stale_thread.comment_count = Set(999);
    stale_thread.updated_at = Set(Utc::now().into());
    let repaired = stale_thread
        .update(&db)
        .await
        .expect("thread update should recompute its count");

    assert_eq!(repaired.comment_count, 1);
}

#[tokio::test]
async fn unique_position_index_rejects_active_model_bypass() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let thread_id = Uuid::new_v4();
    let now = Utc::now();

    comment_thread::ActiveModel {
        id: Set(thread_id),
        tenant_id: Set(tenant_id),
        target_type: Set("blog_post".to_string()),
        target_id: Set(Uuid::new_v4()),
        status: Set(CommentThreadStatus::Open),
        comment_count: Set(0),
        last_commented_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .expect("thread should insert");

    let direct_comment = |id| comment::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        thread_id: Set(thread_id),
        author_id: Set(Uuid::new_v4()),
        parent_comment_id: Set(None),
        status: Set(CommentStatus::Pending),
        position: Set(1),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        deleted_at: Set(None),
    };

    comment::Entity::insert(direct_comment(Uuid::new_v4()))
        .exec(&db)
        .await
        .expect("first direct insert should succeed");
    let duplicate = comment::Entity::insert(direct_comment(Uuid::new_v4()))
        .exec(&db)
        .await;

    assert!(duplicate.is_err(), "unique thread position must reject bypass");
}
