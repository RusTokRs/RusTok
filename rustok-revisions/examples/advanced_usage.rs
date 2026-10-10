//! Advanced usage example for rustok-revisions.
//!
//! This example demonstrates:
//! - Multiple content types
//! - Multi-tenant scenarios
//! - Complex retention policies
//! - Batch operations
//! - Advanced diff analysis

use chrono::Utc;
use rustok_revisions::{
    ChangeSource, Revisionable, RevisionConfig, RevisionEvent, RevisionService, RevisionTracker,
    RetentionPolicy, SeaOrmBackend,
};
use sea_orm::{Database, DatabaseConnection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Blog post content type
#[derive(Clone, Debug, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "blog_post")]
struct BlogPost {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub author_id: Uuid,
    pub tags: Vec<String>,
}

impl RevisionConfig for BlogPost {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast(50))
    }
}

/// Product content type
#[derive(Clone, Debug, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "product")]
struct Product {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub price: f64,
    pub stock: i32,
    pub categories: Vec<String>,
}

impl RevisionConfig for Product {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        // Keep revisions for 365 days
        Some(RetentionPolicy::KeepDays(365))
    }
}

/// User profile content type
#[derive(Clone, Debug, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "user_profile")]
struct UserProfile {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub bio: String,
    pub avatar_url: Option<String>,
}

impl RevisionConfig for UserProfile {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        // Keep all revisions for audit purposes
        Some(RetentionPolicy::KeepAll)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/rustok_revisions".to_string());

    println!("Connecting to database: {}", database_url);
    let db: DatabaseConnection = Database::connect(&database_url).await?;

    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));

    // Simulate multiple tenants
    let tenant_1 = Uuid::new_v4();
    let tenant_2 = Uuid::new_v4();
    let user_1 = Uuid::new_v4();
    let user_2 = Uuid::new_v4();

    println!("\n=== Multi-tenant scenario ===");
    println!("Tenant 1: {}", tenant_1);
    println!("Tenant 2: {}", tenant_2);

    // Tenant 1: Create blog posts
    println!("\n--- Tenant 1: Creating blog posts ---");
    let mut blog_post = BlogPost {
        id: Uuid::new_v4(),
        title: "Introduction to Rust".to_string(),
        content: "Rust is a systems programming language...".to_string(),
        author_id: user_1,
        tags: vec!["rust".to_string(), "programming".to_string()],
    };

    service
        .create_revision_for_create(
            tenant_1,
            blog_post.id,
            "en",
            &blog_post,
            user_1,
            ChangeSource::Web,
        )
        .await?;

    // Update blog post multiple times
    for i in 1..=5 {
        let old_post = blog_post.clone();
        blog_post.title = format!("Introduction to Rust - Edition {}", i);
        blog_post.tags.push(format!("edition-{}", i));

        let tracker = RevisionTracker::builder()
            .enabled(true)
            .source(ChangeSource::Web)
            .build();

        service
            .create_revision_with_tracker(
                tenant_1,
                blog_post.id,
                "en",
                &old_post,
                &blog_post,
                user_1,
                &tracker,
                RevisionEvent::Update,
            )
            .await?;

        println!("  Updated blog post to edition {}", i);
    }

    // Tenant 2: Create products
    println!("\n--- Tenant 2: Creating products ---");
    let mut product = Product {
        id: Uuid::new_v4(),
        name: "Rust Programming Book".to_string(),
        description: "Learn Rust from scratch".to_string(),
        price: 49.99,
        stock: 100,
        categories: vec!["books".to_string(), "programming".to_string()],
    };

    service
        .create_revision_for_create(
            tenant_2,
            product.id,
            "en",
            &product,
            user_2,
            ChangeSource::Admin,
        )
        .await?;

    // Update product price and stock
    for i in 1..=3 {
        let old_product = product.clone();
        product.price = 49.99 - (i as f64 * 5.0); // Discount
        product.stock = 100 - (i * 10); // Selling

        let tracker = RevisionTracker::builder()
            .enabled(true)
            .source(ChangeSource::Admin)
            .build();

        service
            .create_revision_with_tracker(
                tenant_2,
                product.id,
                "en",
                &old_product,
                &product,
                user_2,
                &tracker,
                RevisionEvent::Update,
            )
            .await?;

        println!("  Updated product price to ${:.2}, stock to {}", product.price, product.stock);
    }

    // Tenant 1: Create user profile
    println!("\n--- Tenant 1: Creating user profile ---");
    let mut profile = UserProfile {
        id: user_1,
        username: "rustacean".to_string(),
        email: "rust@example.com".to_string(),
        bio: "I love Rust!".to_string(),
        avatar_url: None,
    };

    service
        .create_revision_for_create(
            tenant_1,
            profile.id,
            "en",
            &profile,
            user_1,
            ChangeSource::Web,
        )
        .await?;

    // Update profile
    let old_profile = profile.clone();
    profile.bio = "I love Rust and systems programming!".to_string();
    profile.avatar_url = Some("https://example.com/avatar.png".to_string());

    let tracker = RevisionTracker::builder()
        .enabled(true)
        .source(ChangeSource::Web)
        .build();

    service
        .create_revision_with_tracker(
            tenant_1,
            profile.id,
            "en",
            &old_profile,
            &profile,
            user_1,
            &tracker,
            RevisionEvent::Update,
        )
        .await?;

    println!("  Updated user profile");

    // Analyze revisions across tenants
    println!("\n=== Analyzing revisions across tenants ===");

    // Tenant 1 stats
    let tenant_1_blog_count = service
        .count_revisions(tenant_1, blog_post.id, "en")
        .await?;
    let tenant_1_profile_count = service
        .count_revisions(tenant_1, profile.id, "en")
        .await?;

    println!("Tenant 1:");
    println!("  Blog post revisions: {}", tenant_1_blog_count);
    println!("  Profile revisions: {}", tenant_1_profile_count);

    // Tenant 2 stats
    let tenant_2_product_count = service
        .count_revisions(tenant_2, product.id, "en")
        .await?;

    println!("Tenant 2:");
    println!("  Product revisions: {}", tenant_2_product_count);

    // Advanced diff analysis
    println!("\n=== Advanced diff analysis ===");
    let blog_revisions = service
        .list_revisions(tenant_1, blog_post.id, "en", None, None)
        .await?;

    if blog_revisions.len() >= 2 {
        let first = blog_revisions.last().unwrap();
        let last = blog_revisions.first().unwrap();

        let diff = service.compare_revisions(first.id, last.id).await?;

        println!("Blog post changes from revision #1 to #{}:", blog_revisions.len());
        
        if let Some(changed) = diff.changed.as_object() {
            for (field, change) in changed {
                println!("  Field: {}", field);
                if let Some(from) = change.get("from") {
                    println!("    From: {}", from);
                }
                if let Some(to) = change.get("to") {
                    println!("    To: {}", to);
                }
            }
        }
    }

    // Product price history
    println!("\n=== Product price history ===");
    let product_revisions = service
        .list_revisions(tenant_2, product.id, "en", None, None)
        .await?;

    for rev in product_revisions.iter().rev() {
        if let Some(content) = rev.content.as_object() {
            if let Some(price) = content.get("price") {
                println!(
                    "  Revision #{} - Price: ${:.2} - {}",
                    rev.revision_number,
                    price.as_f64().unwrap_or(0.0),
                    rev.created_at
                );
            }
        }
    }

    // Create named versions for important milestones
    println!("\n=== Creating named versions ===");
    
    // Blog post final version
    if let Some(last_blog_rev) = blog_revisions.first() {
        service
            .create_named_version(
                tenant_1,
                blog_post.id,
                "en",
                last_blog_rev.id,
                "blog-final",
            )
            .await?;
        println!("  Created named version: blog-final");
    }

    // Product final version
    if let Some(last_product_rev) = product_revisions.first() {
        service
            .create_named_version(
                tenant_2,
                product.id,
                "en",
                last_product_rev.id,
                "product-final",
            )
            .await?;
        println!("  Created named version: product-final");
    }

    // Apply retention policies
    println!("\n=== Applying retention policies ===");

    // Blog posts: keep last 3 (instead of 50)
    let deleted_blog = service
        .apply_retention_policy_for_type::<BlogPost>(
            tenant_1,
            blog_post.id,
            "en",
            &RetentionPolicy::KeepLast(3),
        )
        .await?;
    println!("  Deleted {} old blog post revisions", deleted_blog);

    // Products: keep all (365 days policy, nothing old enough)
    let deleted_product = service
        .apply_retention_policy_for_type::<Product>(
            tenant_2,
            product.id,
            "en",
            &RetentionPolicy::KeepDays(365),
        )
        .await?;
    println!("  Deleted {} old product revisions", deleted_product);

    // User profiles: keep all
    let deleted_profile = service
        .apply_retention_policy_for_type::<UserProfile>(
            tenant_1,
            profile.id,
            "en",
            &RetentionPolicy::KeepAll,
        )
        .await?;
    println!("  Deleted {} old profile revisions", deleted_profile);

    // Final stats
    println!("\n=== Final statistics ===");
    let final_blog_count = service
        .count_revisions(tenant_1, blog_post.id, "en")
        .await?;
    let final_product_count = service
        .count_revisions(tenant_2, product.id, "en")
        .await?;
    let final_profile_count = service
        .count_revisions(tenant_1, profile.id, "en")
        .await?;

    println!("Blog post revisions: {} (was {})", final_blog_count, tenant_1_blog_count);
    println!("Product revisions: {} (was {})", final_product_count, tenant_2_product_count);
    println!("Profile revisions: {} (was {})", final_profile_count, tenant_1_profile_count);

    println!("\n=== Advanced example completed successfully! ===");
    Ok(())
}
