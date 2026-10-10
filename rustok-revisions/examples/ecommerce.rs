//! E-commerce example demonstrating real-world usage of rustok-revisions.
//!
//! This example shows how to track revisions for:
//! - Products with pricing history
//! - Orders with status changes
//! - Customer profiles with address history

use rustok_revisions::{
    ChangeSource, Revisionable, RevisionConfig, RevisionEvent, RevisionService, RevisionTracker,
    RetentionPolicy, SeaOrmBackend,
};
use sea_orm::{Database, DatabaseConnection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Product with pricing and inventory
#[derive(Clone, Debug, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "product")]
struct Product {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub price: f64,
    pub stock: i32,
    pub categories: Vec<String>,
    pub is_active: bool,
}

impl RevisionConfig for Product {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        // Keep pricing history for 2 years
        Some(RetentionPolicy::KeepDays(730))
    }
}

/// Order with status tracking
#[derive(Clone, Debug, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "order")]
struct Order {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub status: OrderStatus,
    pub items: Vec<OrderItem>,
    pub total: f64,
    pub shipping_address: Address,
    pub tracking_number: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum OrderStatus {
    Pending,
    Processing,
    Shipped,
    Delivered,
    Cancelled,
    Refunded,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OrderItem {
    pub product_id: Uuid,
    pub quantity: i32,
    pub price: f64,
}

impl RevisionConfig for Order {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        // Keep all order history for audit
        Some(RetentionPolicy::KeepAll)
    }
}

/// Customer profile
#[derive(Clone, Debug, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "customer")]
struct Customer {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub phone: String,
    pub addresses: Vec<Address>,
    pub loyalty_points: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Address {
    pub street: String,
    pub city: String,
    pub country: String,
    pub postal_code: String,
}

impl RevisionConfig for Customer {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        // Keep customer history for GDPR compliance
        Some(RetentionPolicy::KeepLast(50))
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

    let tenant_id = Uuid::new_v4();
    let admin_user = Uuid::new_v4();
    let customer_user = Uuid::new_v4();

    println!("\n=== E-commerce Revision Tracking Demo ===\n");

    // Scenario 1: Product pricing history
    println!("--- Scenario 1: Product Pricing History ---\n");
    
    let mut product = Product {
        id: Uuid::new_v4(),
        name: "Wireless Headphones".to_string(),
        description: "High-quality wireless headphones with noise cancellation".to_string(),
        price: 199.99,
        stock: 100,
        categories: vec!["electronics".to_string(), "audio".to_string()],
        is_active: true,
    };

    // Create product
    let initial_revision = service
        .create_revision_for_create(
            tenant_id,
            product.id,
            "en",
            &product,
            admin_user,
            ChangeSource::Admin,
        )
        .await?;

    println!("Created product: {} (${:.2})", product.name, product.price);
    println!("Revision #{}", initial_revision.revision_number);

    // Price changes over time
    let price_changes = vec![
        (179.99, "Holiday sale - 10% off"),
        (159.99, "Black Friday special"),
        (199.99, "Back to regular price"),
        (189.99, "Spring promotion"),
    ];

    let tracker = RevisionTracker::builder()
        .enabled(true)
        .source(ChangeSource::Admin)
        .build();

    for (new_price, reason) in price_changes {
        let old_product = product.clone();
        product.price = new_price;

        let revision = service
            .create_revision_with_tracker(
                tenant_id,
                product.id,
                "en",
                &old_product,
                &product,
                admin_user,
                &tracker,
                RevisionEvent::Update,
            )
            .await?;

        if let Some(rev) = revision {
            println!(
                "Price updated: ${:.2} → ${:.2} ({}) - Revision #{}",
                old_product.price, new_price, reason, rev.revision_number
            );
        }
    }

    // Create named version for current price
    let revisions = service
        .list_revisions(tenant_id, product.id, "en", None, None)
        .await?;
    
    if let Some(latest) = revisions.first() {
        service
            .create_named_version(tenant_id, product.id, "en", latest.id, "current-price")
            .await?;
        println!("\n✓ Created named version: current-price");
    }

    // Scenario 2: Order status tracking
    println!("\n--- Scenario 2: Order Status Tracking ---\n");

    let mut order = Order {
        id: Uuid::new_v4(),
        customer_id: customer_user,
        status: OrderStatus::Pending,
        items: vec![
            OrderItem {
                product_id: product.id,
                quantity: 2,
                price: product.price,
            },
        ],
        total: product.price * 2.0,
        shipping_address: Address {
            street: "123 Main St".to_string(),
            city: "New York".to_string(),
            country: "USA".to_string(),
            postal_code: "10001".to_string(),
        },
        tracking_number: None,
    };

    // Create order
    service
        .create_revision_for_create(
            tenant_id,
            order.id,
            "en",
            &order,
            customer_user,
            ChangeSource::Web,
        )
        .await?;

    println!("Order created: {} - Status: Pending", order.id);

    // Order status progression
    let status_changes = vec![
        (OrderStatus::Processing, None, "Order confirmed"),
        (OrderStatus::Shipped, Some("TRACK123456".to_string()), "Shipped via FedEx"),
        (OrderStatus::Delivered, None, "Delivered to customer"),
    ];

    let order_tracker = RevisionTracker::builder()
        .enabled(true)
        .source(ChangeSource::BackgroundJob)
        .build();

    for (new_status, tracking, reason) in status_changes {
        let old_order = order.clone();
        order.status = new_status.clone();
        if let Some(tracking_num) = tracking {
            order.tracking_number = Some(tracking_num);
        }

        let revision = service
            .create_revision_with_tracker(
                tenant_id,
                order.id,
                "en",
                &old_order,
                &order,
                admin_user,
                &order_tracker,
                RevisionEvent::Update,
            )
            .await?;

        if let Some(rev) = revision {
            println!("Order status: {:?} - {} (Revision #{})", order.status, reason, rev.revision_number);
        }
    }

    // Scenario 3: Customer profile updates
    println!("\n--- Scenario 3: Customer Profile Updates ---\n");

    let mut customer = Customer {
        id: customer_user,
        email: "customer@example.com".to_string(),
        name: "John Doe".to_string(),
        phone: "+1-555-0100".to_string(),
        addresses: vec![Address {
            street: "123 Main St".to_string(),
            city: "New York".to_string(),
            country: "USA".to_string(),
            postal_code: "10001".to_string(),
        }],
        loyalty_points: 0,
    };

    // Create customer
    service
        .create_revision_for_create(
            tenant_id,
            customer.id,
            "en",
            &customer,
            customer_user,
            ChangeSource::Web,
        )
        .await?;

    println!("Customer created: {}", customer.name);

    // Customer updates
    let customer_tracker = RevisionTracker::builder()
        .enabled(true)
        .source(ChangeSource::Web)
        .build();

    // Add new address
    let old_customer = customer.clone();
    customer.addresses.push(Address {
        street: "456 Oak Ave".to_string(),
        city: "Boston".to_string(),
        country: "USA".to_string(),
        postal_code: "02101".to_string(),
    });

    service
        .create_revision_with_tracker(
            tenant_id,
            customer.id,
            "en",
            &old_customer,
            &customer,
            customer_user,
            &customer_tracker,
            RevisionEvent::Update,
        )
        .await?;

    println!("Added new address for customer");

    // Update phone
    let old_customer = customer.clone();
    customer.phone = "+1-555-0199".to_string();

    service
        .create_revision_with_tracker(
            tenant_id,
            customer.id,
            "en",
            &old_customer,
            &customer,
            customer_user,
            &customer_tracker,
            RevisionEvent::Update,
        )
        .await?;

    println!("Updated phone number");

    // Add loyalty points
    let old_customer = customer.clone();
    customer.loyalty_points = 500;

    service
        .create_revision_with_tracker(
            tenant_id,
            customer.id,
            "en",
            &old_customer,
            &customer,
            admin_user,
            &customer_tracker,
            RevisionEvent::Update,
        )
        .await?;

    println!("Added 500 loyalty points");

    // Analysis
    println!("\n=== Analysis ===\n");

    // Product pricing analysis
    println!("Product Pricing History:");
    let product_revisions = service
        .list_revisions(tenant_id, product.id, "en", None, None)
        .await?;

    for rev in product_revisions.iter().rev() {
        if let Some(content) = rev.content.as_object() {
            if let Some(price) = content.get("price") {
                println!(
                    "  Revision #{} - ${:.2} - {} - {}",
                    rev.revision_number,
                    price.as_f64().unwrap_or(0.0),
                    format!("{:?}", rev.metadata.source),
                    rev.created_at.format("%Y-%m-%d %H:%M:%S")
                );
            }
        }
    }

    // Order status timeline
    println!("\nOrder Status Timeline:");
    let order_revisions = service
        .list_revisions(tenant_id, order.id, "en", None, None)
        .await?;

    for rev in order_revisions.iter().rev() {
        if let Some(content) = rev.content.as_object() {
            if let Some(status) = content.get("status") {
                let tracking = content
                    .get("tracking_number")
                    .and_then(|t| t.as_str())
                    .unwrap_or("N/A");
                
                println!(
                    "  Revision #{} - Status: {} - Tracking: {} - {}",
                    rev.revision_number,
                    status,
                    tracking,
                    rev.created_at.format("%Y-%m-%d %H:%M:%S")
                );
            }
        }
    }

    // Customer change count
    let customer_revision_count = service
        .count_revisions(tenant_id, customer.id, "en")
        .await?;

    println!("\nCustomer Profile:");
    println!("  Total changes: {}", customer_revision_count);
    println!("  Current loyalty points: {}", customer.loyalty_points);
    println!("  Addresses on file: {}", customer.addresses.len());

    // Cleanup demo
    println!("\n=== Cleanup ===\n");

    // Apply retention policies
    let deleted_products = service
        .apply_retention_policy_for_type::<Product>(
            tenant_id,
            product.id,
            "en",
            &RetentionPolicy::KeepLast(3),
        )
        .await?;

    println!("Deleted {} old product revisions (kept last 3)", deleted_products);

    println!("\n=== E-commerce Demo Complete ===");

    Ok(())
}
