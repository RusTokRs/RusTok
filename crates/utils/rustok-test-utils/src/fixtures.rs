//! Test fixtures for common data types
//!
//! Provides builder patterns for creating test data with sensible defaults.

use chrono::{DateTime, Utc};
use rustok_core::{SecurityContext, UserRole};
use serde_json::Value;
use uuid::Uuid;

/// Fixture builder for creating test users.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::fixtures::UserFixture;
///
/// let admin = UserFixture::admin().build();
/// let customer = UserFixture::customer()
///     .with_email("test@example.com")
///     .build();
/// ```
pub struct UserFixture {
    id: Uuid,
    email: String,
    role: UserRole,
    status: String,
    email_verified: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl UserFixture {
    /// Creates a new user fixture with default values.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            email: format!("user-{}@test.com", Uuid::new_v4()),
            role: UserRole::Customer,
            status: "active".to_string(),
            email_verified: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Creates an admin user fixture.
    pub fn admin() -> Self {
        Self {
            id: Uuid::new_v4(),
            email: format!("admin-{}@test.com", Uuid::new_v4()),
            role: UserRole::Admin,
            status: "active".to_string(),
            email_verified: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Creates a super admin user fixture.
    pub fn super_admin() -> Self {
        Self {
            id: Uuid::new_v4(),
            email: format!("superadmin-{}@test.com", Uuid::new_v4()),
            role: UserRole::SuperAdmin,
            status: "active".to_string(),
            email_verified: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Creates a customer user fixture.
    pub fn customer() -> Self {
        Self {
            id: Uuid::new_v4(),
            email: format!("customer-{}@test.com", Uuid::new_v4()),
            role: UserRole::Customer,
            status: "active".to_string(),
            email_verified: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Creates a manager user fixture.
    pub fn manager() -> Self {
        Self {
            id: Uuid::new_v4(),
            email: format!("manager-{}@test.com", Uuid::new_v4()),
            role: UserRole::Manager,
            status: "active".to_string(),
            email_verified: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Sets the user ID.
    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Sets the email address.
    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = email.into();
        self
    }

    /// Sets the user role.
    pub fn with_role(mut self, role: UserRole) -> Self {
        self.role = role;
        self
    }

    /// Sets the user status.
    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    /// Sets whether the email is verified.
    pub fn with_email_verified(mut self, verified: bool) -> Self {
        self.email_verified = verified;
        self
    }

    /// Builds the user fixture.
    pub fn build(self) -> TestUser {
        TestUser {
            id: self.id,
            email: self.email,
            role: self.role,
            status: self.status,
            email_verified: self.email_verified,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl Default for UserFixture {
    fn default() -> Self {
        Self::new()
    }
}

/// A test user with all fields.
#[derive(Debug, Clone)]
pub struct TestUser {
    pub id: Uuid,
    pub email: String,
    pub role: UserRole,
    pub status: String,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TestUser {
    /// Creates a security context for this user.
    pub fn security_context(&self) -> SecurityContext {
        SecurityContext::new(self.role.clone(), Some(self.id))
    }
}

/// Fixture builder for creating test tenants.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::fixtures::TenantFixture;
///
/// let tenant = TenantFixture::new()
///     .with_name("Test Tenant")
///     .with_slug("test-tenant")
///     .build();
/// ```
pub struct TenantFixture {
    id: Uuid,
    name: String,
    slug: String,
    status: String,
    settings: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TenantFixture {
    /// Creates a new tenant fixture with default values.
    pub fn new() -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            name: "Test Tenant".to_string(),
            slug: format!("tenant-{}", id.to_string().split('-').next().unwrap()),
            status: "active".to_string(),
            settings: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Sets the tenant ID.
    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Sets the tenant name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Sets the tenant slug.
    pub fn with_slug(mut self, slug: impl Into<String>) -> Self {
        self.slug = slug.into();
        self
    }

    /// Sets the tenant status.
    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    /// Sets the tenant settings.
    pub fn with_settings(mut self, settings: Value) -> Self {
        self.settings = settings;
        self
    }

    /// Builds the tenant fixture.
    pub fn build(self) -> TestTenant {
        TestTenant {
            id: self.id,
            name: self.name,
            slug: self.slug,
            status: self.status,
            settings: self.settings,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl Default for TenantFixture {
    fn default() -> Self {
        Self::new()
    }
}

/// A test tenant with all fields.
#[derive(Debug, Clone)]
pub struct TestTenant {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub status: String,
    pub settings: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Fixture builder for creating test nodes (content).
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::fixtures::NodeFixture;
///
/// let post = NodeFixture::post()
///     .with_title("Test Post")
///     .build();
/// ```
pub struct NodeFixture {
    id: Uuid,
    kind: String,
    status: String,
    author_id: Option<Uuid>,
    parent_id: Option<Uuid>,
    category_id: Option<Uuid>,
    position: i32,
    depth: i32,
    metadata: Value,
    translations: Vec<NodeTranslationFixture>,
}

impl NodeFixture {
    /// Creates a new node fixture with default values.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            kind: "post".to_string(),
            status: "draft".to_string(),
            author_id: None,
            parent_id: None,
            category_id: None,
            position: 0,
            depth: 0,
            metadata: serde_json::json!({}),
            translations: vec![NodeTranslationFixture::new()],
        }
    }

    /// Creates a post node fixture.
    pub fn post() -> Self {
        Self {
            id: Uuid::new_v4(),
            kind: "post".to_string(),
            status: "published".to_string(),
            author_id: Some(Uuid::new_v4()),
            parent_id: None,
            category_id: None,
            position: 0,
            depth: 0,
            metadata: serde_json::json!({}),
            translations: vec![NodeTranslationFixture::new().with_title("Test Post")],
        }
    }

    /// Creates a page node fixture.
    pub fn page() -> Self {
        Self {
            id: Uuid::new_v4(),
            kind: "page".to_string(),
            status: "published".to_string(),
            author_id: Some(Uuid::new_v4()),
            parent_id: None,
            category_id: None,
            position: 0,
            depth: 0,
            metadata: serde_json::json!({}),
            translations: vec![NodeTranslationFixture::new().with_title("Test Page")],
        }
    }

    /// Sets the node ID.
    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Sets the node kind.
    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = kind.into();
        self
    }

    /// Sets the node status.
    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    /// Sets the author ID.
    pub fn with_author(mut self, author_id: Uuid) -> Self {
        self.author_id = Some(author_id);
        self
    }

    /// Sets the parent ID.
    pub fn with_parent(mut self, parent_id: Uuid) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Sets the category ID.
    pub fn with_category(mut self, category_id: Uuid) -> Self {
        self.category_id = Some(category_id);
        self
    }

    /// Sets the position.
    pub fn with_position(mut self, position: i32) -> Self {
        self.position = position;
        self
    }

    /// Sets the depth.
    pub fn with_depth(mut self, depth: i32) -> Self {
        self.depth = depth;
        self
    }

    /// Sets the metadata.
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Sets the title on the first translation (or creates one when missing).
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        let title = title.into();
        if self.translations.is_empty() {
            self.translations
                .push(NodeTranslationFixture::new().with_title(title));
        } else {
            self.translations[0] = self.translations[0].clone().with_title(title);
        }
        self
    }

    /// Adds a translation.
    pub fn with_translation(mut self, translation: NodeTranslationFixture) -> Self {
        self.translations.push(translation);
        self
    }

    /// Builds the node fixture.
    pub fn build(self) -> TestNode {
        TestNode {
            id: self.id,
            kind: self.kind,
            status: self.status,
            author_id: self.author_id,
            parent_id: self.parent_id,
            category_id: self.category_id,
            position: self.position,
            depth: self.depth,
            metadata: self.metadata,
            translations: self.translations.into_iter().map(|t| t.build()).collect(),
        }
    }
}

impl Default for NodeFixture {
    fn default() -> Self {
        Self::new()
    }
}

/// A test node with all fields.
#[derive(Debug, Clone)]
pub struct TestNode {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub author_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub position: i32,
    pub depth: i32,
    pub metadata: Value,
    pub translations: Vec<TestNodeTranslation>,
}

/// Fixture builder for node translations.
#[derive(Debug, Clone)]
pub struct NodeTranslationFixture {
    locale: String,
    title: String,
    slug: String,
    excerpt: Option<String>,
    body: Option<String>,
}

impl NodeTranslationFixture {
    /// Creates a new translation fixture with default values.
    pub fn new() -> Self {
        Self {
            locale: "en".to_string(),
            title: "Test Title".to_string(),
            slug: "test-title".to_string(),
            excerpt: Some("Test excerpt".to_string()),
            body: Some("Test body content".to_string()),
        }
    }

    /// Sets the locale.
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = locale.into();
        self
    }

    /// Sets the title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets the slug.
    pub fn with_slug(mut self, slug: impl Into<String>) -> Self {
        self.slug = slug.into();
        self
    }

    /// Sets the excerpt.
    pub fn with_excerpt(mut self, excerpt: impl Into<String>) -> Self {
        self.excerpt = Some(excerpt.into());
        self
    }

    /// Sets the body.
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Builds the translation fixture.
    pub fn build(self) -> TestNodeTranslation {
        TestNodeTranslation {
            locale: self.locale,
            title: self.title,
            slug: self.slug,
            excerpt: self.excerpt,
            body: self.body,
        }
    }
}

impl Default for NodeTranslationFixture {
    fn default() -> Self {
        Self::new()
    }
}

/// A test node translation with all fields.
#[derive(Debug, Clone)]
pub struct TestNodeTranslation {
    pub locale: String,
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub body: Option<String>,
}

/// Fixture builder for creating test products.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::fixtures::ProductFixture;
///
/// let product = ProductFixture::new()
///     .with_name("Test Product")
///     .with_price(99.99)
///     .build();
/// ```
pub struct ProductFixture {
    id: Uuid,
    sku: String,
    name: String,
    description: Option<String>,
    price: f64,
    compare_at_price: Option<f64>,
    status: String,
    inventory_quantity: i32,
    track_inventory: bool,
    metadata: Value,
}

impl ProductFixture {
    /// Creates a new product fixture with default values.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            sku: format!(
                "SKU-{}",
                Uuid::new_v4().to_string().split('-').next().unwrap()
            ),
            name: "Test Product".to_string(),
            description: Some("A test product description".to_string()),
            price: 99.99,
            compare_at_price: None,
            status: "active".to_string(),
            inventory_quantity: 100,
            track_inventory: true,
            metadata: serde_json::json!({}),
        }
    }

    /// Sets the product ID.
    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Sets the SKU.
    pub fn with_sku(mut self, sku: impl Into<String>) -> Self {
        self.sku = sku.into();
        self
    }

    /// Sets the name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Sets the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the price.
    pub fn with_price(mut self, price: f64) -> Self {
        self.price = price;
        self
    }

    /// Sets the compare at price.
    pub fn with_compare_at_price(mut self, price: f64) -> Self {
        self.compare_at_price = Some(price);
        self
    }

    /// Sets the status.
    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    /// Sets the inventory quantity.
    pub fn with_inventory(mut self, quantity: i32) -> Self {
        self.inventory_quantity = quantity;
        self
    }

    /// Sets whether to track inventory.
    pub fn with_track_inventory(mut self, track: bool) -> Self {
        self.track_inventory = track;
        self
    }

    /// Sets the metadata.
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Builds the product fixture.
    pub fn build(self) -> TestProduct {
        TestProduct {
            id: self.id,
            sku: self.sku,
            name: self.name,
            description: self.description,
            price: self.price,
            compare_at_price: self.compare_at_price,
            status: self.status,
            inventory_quantity: self.inventory_quantity,
            track_inventory: self.track_inventory,
            metadata: self.metadata,
        }
    }
}

impl Default for ProductFixture {
    fn default() -> Self {
        Self::new()
    }
}

/// A test product with all fields.
#[derive(Debug, Clone)]
pub struct TestProduct {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub price: f64,
    pub compare_at_price: Option<f64>,
    pub status: String,
    pub inventory_quantity: i32,
    pub track_inventory: bool,
    pub metadata: Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_fixture() {
        let user = UserFixture::admin().with_email("admin@test.com").build();

        assert_eq!(user.email, "admin@test.com");
        assert!(matches!(user.role, UserRole::Admin));
    }

    #[test]
    fn test_tenant_fixture() {
        let tenant = TenantFixture::new()
            .with_name("My Tenant")
            .with_slug("my-tenant")
            .build();

        assert_eq!(tenant.name, "My Tenant");
        assert_eq!(tenant.slug, "my-tenant");
    }

    #[test]
    fn test_node_fixture() {
        let node = NodeFixture::post().with_title("My Post").build();

        assert_eq!(node.kind, "post");
        assert_eq!(node.translations[0].title, "My Post");
    }

    #[test]
    fn test_product_fixture() {
        let product = ProductFixture::new()
            .with_name("My Product")
            .with_price(49.99)
            .build();

        assert_eq!(product.name, "My Product");
        assert_eq!(product.price, 49.99);
    }
}
