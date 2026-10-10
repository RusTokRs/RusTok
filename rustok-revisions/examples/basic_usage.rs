//! Basic usage example for rustok-revisions.
//!
//! This example demonstrates how to:
//! - Define a revisionable content type
//! - Create revisions
//! - List revisions
//! - Compare revisions
//! - Restore to a previous revision

use chrono::Utc;
use rustok_revisions::{
    ChangeSource, Revisionable, RevisionConfig, RevisionEvent, RevisionService, RevisionTracker,
    RetentionPolicy, SeaOrmBackend,
};
use sea_orm::{Database, DatabaseConnection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A simple blog post that can be tracked for revisions.
#[derive(Clone, Debug, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "blog_post")]
struct Post {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub author: String,
    pub published: bool,
    pub views: i32,
}

impl RevisionConfig for Post {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        // Keep the last 100 revisions
        Some(RetentionPolicy::KeepLast(100))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Connect to the database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/rustok_revisions".to_string());

    println!("Connecting to database: {}", database_url);
    let db: DatabaseConnection = Database::connect(&database_url).await?;

    // Create the revision service
    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));

    // Create tenant and user IDs
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let locale = "en";

    println!("\n=== Creating initial post ===");
    let post = Post {
        id: Uuid::new_v4(),
        title: "My First Blog Post".to_string(),
        content: "This is the initial content of my blog post.".to_string(),
        author: "John Doe".to_string(),
        published: false,
        views: 0,
    };

    // Create the first revision (Create event)
    let first_revision = service
        .create_revision_for_create(tenant_id, post.id, locale, &post, user_id, ChangeSource::Web)
        .await?;

    println!("Created revision #{}: {:?}", first_revision.revision_number, first_revision.id);

    // Update the post
    println!("\n=== Updating post ===");
    let old_post = post.clone();
    let mut updated_post = post.clone();
    updated_post.title = "My First Blog Post (Updated)".to_string();
    updated_post.content = "This is the updated content with more details.".to_string();

    // Create a tracker for the update
    let tracker = RevisionTracker::builder()
        .enabled(true)
        .source(ChangeSource::Web)
        .retention(RetentionPolicy::KeepLast(100))
        .build();

    let second_revision = service
        .create_revision_with_tracker(
            tenant_id,
            post.id,
            locale,
            &old_post,
            &updated_post,
            user_id,
            &tracker,
            RevisionEvent::Update,
        )
        .await?;

    if let Some(rev) = second_revision {
        println!("Created revision #{}: {:?}", rev.revision_number, rev.id);
    }

    // Publish the post
    println!("\n=== Publishing post ===");
    let old_post = updated_post.clone();
    let mut published_post = updated_post.clone();
    published_post.published = true;

    let third_revision = service
        .create_revision_with_tracker(
            tenant_id,
            post.id,
            locale,
            &old_post,
            &published_post,
            user_id,
            &tracker,
            RevisionEvent::Update,
        )
        .await?;

    if let Some(rev) = third_revision {
        println!("Created revision #{}: {:?}", rev.revision_number, rev.id);
        
        // Create a named version for this published state
        service
            .create_named_version(tenant_id, post.id, locale, rev.id, "v1.0-published")
            .await?;
        println!("Created named version: v1.0-published");
    }

    // List all revisions
    println!("\n=== Listing all revisions ===");
    let revisions = service
        .list_revisions(tenant_id, post.id, locale, None, None)
        .await?;

    println!("Total revisions: {}", revisions.len());
    for rev in &revisions {
        println!(
            "  #{} - {} - {:?} - {}",
            rev.revision_number,
            rev.event,
            rev.created_at,
            rev.version_name.as_deref().unwrap_or("")
        );
    }

    // Compare first and last revision
    println!("\n=== Comparing revisions ===");
    if revisions.len() >= 2 {
        let first_id = revisions.last().unwrap().id;
        let last_id = revisions.first().unwrap().id;

        let diff = service.compare_revisions(first_id, last_id).await?;

        println!("Changes from revision #1 to #{}:", revisions.len());
        println!("Added fields: {:?}", diff.added);
        println!("Removed fields: {:?}", diff.removed);
        println!("Changed fields: {:?}", diff.changed);
    }

    // Get named versions
    println!("\n=== Named versions ===");
    let named_versions = service
        .get_named_versions(tenant_id, post.id, locale)
        .await?;

    println!("Named versions: {}", named_versions.len());
    for rev in &named_versions {
        println!(
            "  {} - revision #{} - {}",
            rev.version_name.as_deref().unwrap_or(""),
            rev.revision_number,
            rev.created_at
        );
    }

    // Restore to the first revision
    println!("\n=== Restoring to first revision ===");
    let first_revision_id = revisions.last().unwrap().id;
    let (restored_post, restore_revision) = service
        .restore_revision::<Post>(
            tenant_id,
            post.id,
            locale,
            first_revision_id,
            user_id,
            ChangeSource::Admin,
        )
        .await?;

    println!(
        "Restored to revision #{} (new revision #{})",
        revisions.last().unwrap().revision_number,
        restore_revision.revision_number
    );
    println!("Restored post: {:?}", restored_post);

    // Count total revisions
    let total = service
        .count_revisions(tenant_id, post.id, locale)
        .await?;
    println!("\nTotal revisions after restore: {}", total);

    // Apply retention policy
    println!("\n=== Applying retention policy ===");
    let deleted = service
        .apply_retention_policy_for_type::<Post>(
            tenant_id,
            post.id,
            locale,
            &RetentionPolicy::KeepLast(2),
        )
        .await?;

    println!("Deleted {} old revisions", deleted);

    let remaining = service
        .count_revisions(tenant_id, post.id, locale)
        .await?;
    println!("Remaining revisions: {}", remaining);

    println!("\n=== Example completed successfully! ===");
    Ok(())
}
