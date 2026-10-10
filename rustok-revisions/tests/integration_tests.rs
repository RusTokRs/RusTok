//! Integration tests for rustok-revisions.

use chrono::Utc;
use rustok_revisions::{
    ChangeSource, Revisionable, RevisionConfig, RevisionEvent, RevisionService, RevisionTracker,
    RetentionPolicy, SeaOrmBackend,
};
use sea_orm::{Database, DatabaseConnection, DbErr};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "test_post")]
struct TestPost {
    pub id: Uuid,
    pub title: String,
    pub content: String,
}

impl RevisionConfig for TestPost {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast(10))
    }
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/rustok_revisions_test".to_string());
    
    Database::connect(&database_url).await
}

#[tokio::test]
async fn test_create_revision() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let post = TestPost {
        id: Uuid::new_v4(),
        title: "Test Post".to_string(),
        content: "Test content".to_string(),
    };

    let revision = service
        .create_revision_for_create(tenant_id, post.id, "en", &post, user_id, ChangeSource::Web)
        .await?;

    assert_eq!(revision.revision_number, 1);
    assert_eq!(revision.content_type, "test_post");
    assert_eq!(revision.locale, "en");
    assert_eq!(revision.event, RevisionEvent::Create);

    Ok(())
}

#[tokio::test]
async fn test_update_revision() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    let old_post = TestPost {
        id: Uuid::new_v4(),
        title: "Old Title".to_string(),
        content: "Old content".to_string(),
    };

    // Create initial revision
    service
        .create_revision_for_create(tenant_id, old_post.id, "en", &old_post, user_id, ChangeSource::Web)
        .await?;

    // Update the post
    let mut new_post = old_post.clone();
    new_post.title = "New Title".to_string();

    let tracker = RevisionTracker::builder()
        .enabled(true)
        .source(ChangeSource::Web)
        .build();

    let revision = service
        .create_revision_with_tracker(
            tenant_id,
            old_post.id,
            "en",
            &old_post,
            &new_post,
            user_id,
            &tracker,
            RevisionEvent::Update,
        )
        .await?;

    assert!(revision.is_some());
    let revision = revision.unwrap();
    assert_eq!(revision.revision_number, 2);
    assert_eq!(revision.event, RevisionEvent::Update);

    Ok(())
}

#[tokio::test]
async fn test_list_revisions() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    let post = TestPost {
        id: Uuid::new_v4(),
        title: "Test Post".to_string(),
        content: "Test content".to_string(),
    };

    // Create multiple revisions
    service
        .create_revision_for_create(tenant_id, post.id, "en", &post, user_id, ChangeSource::Web)
        .await?;

    let tracker = RevisionTracker::builder().enabled(true).build();

    for i in 1..=3 {
        let old_post = post.clone();
        let mut new_post = post.clone();
        new_post.title = format!("Test Post {}", i);

        service
            .create_revision_with_tracker(
                tenant_id,
                post.id,
                "en",
                &old_post,
                &new_post,
                user_id,
                &tracker,
                RevisionEvent::Update,
            )
            .await?;
    }

    // List all revisions
    let revisions = service
        .list_revisions(tenant_id, post.id, "en", None, None)
        .await?;

    assert_eq!(revisions.len(), 4); // 1 create + 3 updates

    // List with limit
    let revisions = service
        .list_revisions(tenant_id, post.id, "en", Some(2), None)
        .await?;

    assert_eq!(revisions.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_compare_revisions() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    let post = TestPost {
        id: Uuid::new_v4(),
        title: "Original Title".to_string(),
        content: "Original content".to_string(),
    };

    let first_revision = service
        .create_revision_for_create(tenant_id, post.id, "en", &post, user_id, ChangeSource::Web)
        .await?;

    let old_post = post.clone();
    let mut new_post = post.clone();
    new_post.title = "Updated Title".to_string();
    new_post.content = "Updated content".to_string();

    let tracker = RevisionTracker::builder().enabled(true).build();

    let second_revision = service
        .create_revision_with_tracker(
            tenant_id,
            post.id,
            "en",
            &old_post,
            &new_post,
            user_id,
            &tracker,
            RevisionEvent::Update,
        )
        .await?
        .unwrap();

    // Compare revisions
    let diff = service
        .compare_revisions(first_revision.id, second_revision.id)
        .await?;

    // Check that title and content changed
    let changed = diff.changed.as_object().unwrap();
    assert!(changed.contains_key("title"));
    assert!(changed.contains_key("content"));

    Ok(())
}

#[tokio::test]
async fn test_restore_revision() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    let post = TestPost {
        id: Uuid::new_v4(),
        title: "Original Title".to_string(),
        content: "Original content".to_string(),
    };

    let first_revision = service
        .create_revision_for_create(tenant_id, post.id, "en", &post, user_id, ChangeSource::Web)
        .await?;

    // Update the post
    let old_post = post.clone();
    let mut new_post = post.clone();
    new_post.title = "Updated Title".to_string();

    let tracker = RevisionTracker::builder().enabled(true).build();

    service
        .create_revision_with_tracker(
            tenant_id,
            post.id,
            "en",
            &old_post,
            &new_post,
            user_id,
            &tracker,
            RevisionEvent::Update,
        )
        .await?;

    // Restore to first revision
    let (restored_post, restore_revision) = service
        .restore_revision::<TestPost>(
            tenant_id,
            post.id,
            "en",
            first_revision.id,
            user_id,
            ChangeSource::Admin,
        )
        .await?;

    assert_eq!(restored_post.title, "Original Title");
    assert_eq!(restore_revision.event, RevisionEvent::Restore);
    assert_eq!(restore_revision.revision_number, 3);

    Ok(())
}

#[tokio::test]
async fn test_named_versions() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    let post = TestPost {
        id: Uuid::new_v4(),
        title: "Test Post".to_string(),
        content: "Test content".to_string(),
    };

    let revision = service
        .create_revision_for_create(tenant_id, post.id, "en", &post, user_id, ChangeSource::Web)
        .await?;

    // Create named version
    service
        .create_named_version(tenant_id, post.id, "en", revision.id, "v1.0")
        .await?;

    // Get named versions
    let named_versions = service
        .get_named_versions(tenant_id, post.id, "en")
        .await?;

    assert_eq!(named_versions.len(), 1);
    assert_eq!(named_versions[0].version_name, Some("v1.0".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_retention_policy() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    let post = TestPost {
        id: Uuid::new_v4(),
        title: "Test Post".to_string(),
        content: "Test content".to_string(),
    };

    // Create initial revision
    service
        .create_revision_for_create(tenant_id, post.id, "en", &post, user_id, ChangeSource::Web)
        .await?;

    let tracker = RevisionTracker::builder().enabled(true).build();

    // Create 10 more revisions
    for i in 1..=10 {
        let old_post = post.clone();
        let mut new_post = post.clone();
        new_post.title = format!("Test Post {}", i);

        service
            .create_revision_with_tracker(
                tenant_id,
                post.id,
                "en",
                &old_post,
                &new_post,
                user_id,
                &tracker,
                RevisionEvent::Update,
            )
            .await?;
    }

    // Should have 11 revisions total
    let count = service.count_revisions(tenant_id, post.id, "en").await?;
    assert_eq!(count, 11);

    // Apply retention policy: keep last 5
    let deleted = service
        .apply_retention_policy_for_type::<TestPost>(
            tenant_id,
            post.id,
            "en",
            &RetentionPolicy::KeepLast(5),
        )
        .await?;

    assert_eq!(deleted, 6); // 11 - 5 = 6 deleted

    // Should have 5 revisions now
    let count = service.count_revisions(tenant_id, post.id, "en").await?;
    assert_eq!(count, 5);

    Ok(())
}

#[tokio::test]
async fn test_multi_tenant_isolation() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));

    let tenant_1 = Uuid::new_v4();
    let tenant_2 = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    let post = TestPost {
        id: Uuid::new_v4(),
        title: "Test Post".to_string(),
        content: "Test content".to_string(),
    };

    // Create revision for tenant 1
    service
        .create_revision_for_create(tenant_1, post.id, "en", &post, user_id, ChangeSource::Web)
        .await?;

    // Create revision for tenant 2
    service
        .create_revision_for_create(tenant_2, post.id, "en", &post, user_id, ChangeSource::Web)
        .await?;

    // Tenant 1 should only see their revision
    let tenant_1_revisions = service
        .list_revisions(tenant_1, post.id, "en", None, None)
        .await?;
    assert_eq!(tenant_1_revisions.len(), 1);

    // Tenant 2 should only see their revision
    let tenant_2_revisions = service
        .list_revisions(tenant_2, post.id, "en", None, None)
        .await?;
    assert_eq!(tenant_2_revisions.len(), 1);

    Ok(())
}
