# Интеграция Demo Data с Content Portability

**Дата:** 2026-10-09  
**Статус:** ⚠️ Требуется интеграция  
**Автор:** AI Assistant

## Текущая ситуация

### ✅ Что есть

**1. Seed Infrastructure (`rustok-installer`)**
- Создает tenant, admin user, demo customer
- Включает/выключает modules
- `SeedProfile` enum: `None`, `Minimal`, `Dev`
- `execute_seed_profile()` orchestrator

**2. Content Portability Platform**
- `ContentImporter<Source, Target>` trait
- `ImportService` для JSON/CSV
- Format handlers (JSON, CSV)
- Validation framework
- Progress tracking

**3. Module Structure**
- Newsletter module готов
- Blog enhancements готовы
- Все модули имеют services для CRUD

### ❌ Что отсутствует

**1. Demo Content Loaders**
- Нет загрузчиков демо-контента для модулей
- Нет интеграции между seed и content portability
- Нет seed data файлов (JSON/CSV)

**2. Integration Layer**
- Нет `SeedContentPort` trait
- Нет mechanism для загрузки контента после создания tenant
- Нет orchestration между infrastructure seed и content seed

**3. Seed Data**
- Нет демо-постов для blog
- Нет демо-товаров для commerce
- Нет демо-подписчиков для newsletter
- Нет демо-тем для forum

## Архитектурное решение

### Design Goals

1. **Separation of concerns**: Infrastructure seed (tenant/users) separate from content seed
2. **Modular**: Each module provides its own content loader
3. **Optional**: Content seeding is optional, controlled by `SeedProfile`
4. **Idempotent**: Can run multiple times safely
5. **Tenant-scoped**: All content is tenant-isolated
6. **Uses Content Portability**: Reuses existing import infrastructure

### Architecture

```
rustok-installer
  ├─ seed.rs (existing)
  │   ├─ SeedTenantPort
  │   ├─ SeedPrincipalPort
  │   ├─ SeedModulePort
  │   └─ execute_seed_profile()
  │
  └─ seed_content.rs (NEW)
      ├─ SeedContentPort trait
      ├─ SeedContentRequest
      ├─ SeedContentOutcome
      └─ execute_seed_content()

Module Content Loaders (NEW)
  ├─ rustok-blog/src/seed.rs
  │   └─ BlogSeedContentLoader
  ├─ rustok-commerce/src/seed.rs
  │   └─ CommerceSeedContentLoader
  ├─ rustok-newsletter/src/seed.rs
  │   └─ NewsletterSeedContentLoader
  └─ rustok-forum/src/seed.rs
      └─ ForumSeedContentLoader

Seed Data Files (NEW)
  ├─ seeds/blog/posts.json
  ├─ seeds/blog/categories.json
  ├─ seeds/commerce/products.json
  ├─ seeds/commerce/categories.json
  ├─ seeds/newsletter/subscribers.json
  ├─ seeds/newsletter/campaigns.json
  └─ seeds/forum/topics.json
```

### Integration Flow

```
1. execute_seed_profile()
   ├─ Create tenant
   ├─ Create admin user
   ├─ Create demo customer (if Dev profile)
   └─ Enable/disable modules

2. execute_seed_content() [NEW]
   ├─ For each enabled module:
   │   ├─ Load seed data from files
   │   ├─ Use ContentImporter to import
   │   └─ Track results
   └─ Return aggregated outcome

3. Installer orchestration
   ├─ Run execute_seed_profile()
   ├─ If SeedProfile::Dev:
   │   └─ Run execute_seed_content()
   └─ Return complete outcome
```

## Реализация

### Phase 1: Core Integration Layer

#### 1.1 SeedContentPort Trait

**File:** `crates/utils/rustok-installer/src/seed_content.rs`

```rust
use async_trait::async_trait;
use rustok_content_portability_api::{ImportContext, BatchImportResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedContentRequest {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub module_slug: String,
    pub seed_data_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedContentOutcome {
    pub module_slug: String,
    pub imported_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SeedContentError {
    #[error("seed content loading failed: {0}")]
    Loading(String),
    
    #[error("seed content import failed: {0}")]
    Import(String),
    
    #[error("seed data not found: {0}")]
    NotFound(String),
}

#[async_trait]
pub trait SeedContentPort: Send + Sync {
    /// Load seed content for a specific module
    async fn load_seed_content(
        &self,
        request: SeedContentRequest,
    ) -> Result<SeedContentOutcome, SeedContentError>;
    
    /// Check if seed data exists for a module
    fn has_seed_data(&self, module_slug: &str) -> bool;
}

/// Orchestrate seed content loading for all enabled modules
pub async fn execute_seed_content(
    tenant_id: Uuid,
    user_id: Uuid,
    enabled_modules: &[String],
    seed_data_base_path: &str,
    content_ports: &dyn SeedContentPort,
) -> Result<Vec<SeedContentOutcome>, SeedContentError> {
    let mut outcomes = Vec::new();
    
    for module_slug in enabled_modules {
        if !content_ports.has_seed_data(module_slug) {
            continue;
        }
        
        let request = SeedContentRequest {
            tenant_id,
            user_id,
            module_slug: module_slug.clone(),
            seed_data_path: format!("{}/{}", seed_data_base_path, module_slug),
        };
        
        let outcome = content_ports.load_seed_content(request).await?;
        outcomes.push(outcome);
    }
    
    Ok(outcomes)
}
```

#### 1.2 Integration with execute_seed_profile

**File:** `crates/utils/rustok-installer/src/seed.rs` (update)

```rust
pub async fn execute_seed_profile(
    request: SeedExecutionRequest,
    tenant_port: &dyn SeedTenantPort,
    principal_port: &dyn SeedPrincipalPort,
    module_port: &dyn SeedModulePort,
    content_port: Option<&dyn SeedContentPort>,  // NEW
) -> Result<SeedExecutionOutcome, SeedExecutionError> {
    // ... existing code ...
    
    let outcome = SeedExecutionOutcome {
        tenant,
        enabled_modules,
        disabled_modules,
        admin,
        demo_customer,
        content_outcomes: Vec::new(),  // NEW
    };
    
    // Load seed content for Dev profile
    if request.profile == SeedProfile::Dev {
        if let Some(content_port) = content_port {
            let admin_user_id = outcome.admin.as_ref().map(|u| u.id).unwrap_or(Uuid::nil());
            let content_outcomes = execute_seed_content(
                tenant.id,
                admin_user_id,
                &outcome.enabled_modules,
                "seeds",
                content_port,
            ).await.map_err(|e| SeedExecutionError::Dependency(e.to_string()))?;
            
            outcome.content_outcomes = content_outcomes;
        }
    }
    
    Ok(outcome)
}
```

### Phase 2: Module Content Loaders

#### 2.1 Blog Seed Content Loader

**File:** `crates/modules/rustok-blog/src/seed.rs`

```rust
use async_trait::async_trait;
use rustok_content_portability::{ImportService, JsonFormatHandler};
use rustok_content_portability_api::{ImportContext, Format};
use rustok_installer::{SeedContentPort, SeedContentRequest, SeedContentOutcome, SeedContentError};
use crate::dto::{CreatePostInput, PostResponse};
use crate::services::PostService;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

pub struct BlogSeedContentLoader {
    db: DatabaseConnection,
}

impl BlogSeedContentLoader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SeedContentPort for BlogSeedContentLoader {
    async fn load_seed_content(
        &self,
        request: SeedContentRequest,
    ) -> Result<SeedContentOutcome, SeedContentError> {
        let context = ImportContext::new(
            request.tenant_id,
            request.user_id,
            Format::Json,
        );
        
        // Load categories
        let categories_path = format!("{}/categories.json", request.seed_data_path);
        let categories: Vec<CreateCategoryInput> = ImportService::import_json_file(
            &categories_path,
            context.clone(),
        ).await.map_err(|e| SeedContentError::Loading(e.to_string()))?;
        
        // Import categories
        let category_service = CategoryService::new(self.db.clone());
        let mut category_ids = Vec::new();
        for category in categories {
            match category_service.create(request.tenant_id, category).await {
                Ok(cat) => category_ids.push(cat.id),
                Err(e) => return Err(SeedContentError::Import(e.to_string())),
            }
        }
        
        // Load posts
        let posts_path = format!("{}/posts.json", request.seed_data_path);
        let posts: Vec<CreatePostInput> = ImportService::import_json_file(
            &posts_path,
            context,
        ).await.map_err(|e| SeedContentError::Loading(e.to_string()))?;
        
        // Import posts
        let post_service = PostService::new(self.db.clone());
        let mut imported = 0;
        let mut failed = 0;
        let mut errors = Vec::new();
        
        for mut post in posts {
            // Assign random category if available
            if !category_ids.is_empty() && post.category_id.is_none() {
                post.category_id = Some(category_ids[0]);
            }
            
            match post_service.create_post(request.tenant_id, post).await {
                Ok(_) => imported += 1,
                Err(e) => {
                    failed += 1;
                    errors.push(e.to_string());
                }
            }
        }
        
        Ok(SeedContentOutcome {
            module_slug: request.module_slug,
            imported_count: imported,
            failed_count: failed,
            errors,
        })
    }
    
    fn has_seed_data(&self, module_slug: &str) -> bool {
        module_slug == "blog"
    }
}
```

#### 2.2 Newsletter Seed Content Loader

**File:** `crates/modules/rustok-newsletter/src/seed.rs`

```rust
use async_trait::async_trait;
use rustok_content_portability::ImportService;
use rustok_content_portability_api::{ImportContext, Format};
use rustok_installer::{SeedContentPort, SeedContentRequest, SeedContentOutcome, SeedContentError};
use crate::dto::{SubscribeInput, CreateCampaignInput};
use crate::services::{SubscriberService, CampaignService};
use sea_orm::DatabaseConnection;

pub struct NewsletterSeedContentLoader {
    db: DatabaseConnection,
}

impl NewsletterSeedContentLoader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SeedContentPort for NewsletterSeedContentLoader {
    async fn load_seed_content(
        &self,
        request: SeedContentRequest,
    ) -> Result<SeedContentOutcome, SeedContentError> {
        let context = ImportContext::new(
            request.tenant_id,
            request.user_id,
            Format::Json,
        );
        
        // Load subscribers
        let subscribers_path = format!("{}/subscribers.json", request.seed_data_path);
        let subscribers: Vec<SubscribeInput> = ImportService::import_json_file(
            &subscribers_path,
            context.clone(),
        ).await.map_err(|e| SeedContentError::Loading(e.to_string()))?;
        
        // Import subscribers
        let subscriber_service = SubscriberService::new(self.db.clone());
        let mut imported = 0;
        let mut failed = 0;
        let mut errors = Vec::new();
        
        for subscriber in subscribers {
            match subscriber_service.subscribe(request.tenant_id, subscriber).await {
                Ok(_) => imported += 1,
                Err(e) => {
                    failed += 1;
                    errors.push(e.to_string());
                }
            }
        }
        
        // Load campaigns
        let campaigns_path = format!("{}/campaigns.json", request.seed_data_path);
        let campaigns: Vec<CreateCampaignInput> = ImportService::import_json_file(
            &campaigns_path,
            context,
        ).await.map_err(|e| SeedContentError::Loading(e.to_string()))?;
        
        // Import campaigns
        let campaign_service = CampaignService::new(self.db.clone());
        for campaign in campaigns {
            match campaign_service.create(request.tenant_id, campaign).await {
                Ok(_) => imported += 1,
                Err(e) => {
                    failed += 1;
                    errors.push(e.to_string());
                }
            }
        }
        
        Ok(SeedContentOutcome {
            module_slug: request.module_slug,
            imported_count: imported,
            failed_count: failed,
            errors,
        })
    }
    
    fn has_seed_data(&self, module_slug: &str) -> bool {
        module_slug == "newsletter"
    }
}
```

### Phase 3: Seed Data Files

#### 3.1 Blog Seed Data

**File:** `seeds/blog/categories.json`

```json
[
  {
    "name": "Technology",
    "slug": "technology",
    "description": "Latest tech news and tutorials"
  },
  {
    "name": "Business",
    "slug": "business",
    "description": "Business insights and strategies"
  },
  {
    "name": "Lifestyle",
    "slug": "lifestyle",
    "description": "Tips for better living"
  }
]
```

**File:** `seeds/blog/posts.json`

```json
[
  {
    "title": "Getting Started with Rust",
    "slug": "getting-started-with-rust",
    "content": "# Getting Started with Rust\n\nRust is a systems programming language...",
    "excerpt": "Learn the basics of Rust programming language",
    "status": "published",
    "featured": true,
    "tags": ["rust", "programming", "tutorial"]
  },
  {
    "title": "Building Web Applications",
    "slug": "building-web-applications",
    "content": "# Building Web Applications\n\nModern web development...",
    "excerpt": "A guide to building modern web applications",
    "status": "published",
    "featured": false,
    "tags": ["web", "development"]
  }
]
```

#### 3.2 Newsletter Seed Data

**File:** `seeds/newsletter/subscribers.json`

```json
[
  {
    "email": "alice@example.com",
    "name": "Alice Johnson",
    "locale": "en"
  },
  {
    "email": "bob@example.com",
    "name": "Bob Smith",
    "locale": "en"
  },
  {
    "email": "carol@example.com",
    "name": "Carol Williams",
    "locale": "ru"
  }
]
```

**File:** `seeds/newsletter/campaigns.json`

```json
[
  {
    "title": "Weekly Tech Digest",
    "subject": "This Week in Tech",
    "preheader": "Latest tech news and updates",
    "content_sources": ["blog"],
    "status": "draft"
  },
  {
    "title": "Product Updates",
    "subject": "New Features Released",
    "preheader": "Check out what's new",
    "content_sources": ["commerce"],
    "status": "draft"
  }
]
```

### Phase 4: Composite Seed Content Port

**File:** `crates/utils/rustok-installer/src/seed_content_composite.rs`

```rust
use std::collections::HashMap;
use std::sync::Arc;
use rustok_installer::{SeedContentPort, SeedContentRequest, SeedContentOutcome, SeedContentError};

pub struct CompositeSeedContentPort {
    loaders: HashMap<String, Arc<dyn SeedContentPort>>,
}

impl CompositeSeedContentPort {
    pub fn new() -> Self {
        Self {
            loaders: HashMap::new(),
        }
    }
    
    pub fn register_loader(
        &mut self,
        module_slug: String,
        loader: Arc<dyn SeedContentPort>,
    ) {
        self.loaders.insert(module_slug, loader);
    }
}

#[async_trait::async_trait]
impl SeedContentPort for CompositeSeedContentPort {
    async fn load_seed_content(
        &self,
        request: SeedContentRequest,
    ) -> Result<SeedContentOutcome, SeedContentError> {
        let loader = self.loaders.get(&request.module_slug)
            .ok_or_else(|| SeedContentError::NotFound(
                format!("no loader for module: {}", request.module_slug)
            ))?;
        
        loader.load_seed_content(request).await
    }
    
    fn has_seed_data(&self, module_slug: &str) -> bool {
        self.loaders.get(module_slug)
            .map(|l| l.has_seed_data(module_slug))
            .unwrap_or(false)
    }
}
```

## Usage Example

```rust
use rustok_installer::{
    execute_seed_profile, execute_seed_content,
    SeedExecutionRequest, SeedProfile, SeedTenantRequest,
    CompositeSeedContentPort,
};
use rustok_blog::seed::BlogSeedContentLoader;
use rustok_newsletter::seed::NewsletterSeedContentLoader;

// Create composite port
let mut content_port = CompositeSeedContentPort::new();

// Register loaders
content_port.register_loader(
    "blog".to_string(),
    Arc::new(BlogSeedContentLoader::new(db.clone())),
);
content_port.register_loader(
    "newsletter".to_string(),
    Arc::new(NewsletterSeedContentLoader::new(db.clone())),
);

// Execute seed profile
let request = SeedExecutionRequest {
    profile: SeedProfile::Dev,
    tenant: SeedTenantRequest {
        name: "Demo Tenant".to_string(),
        slug: "demo".to_string(),
        domain: None,
    },
    enabled_modules: vec!["blog".to_string(), "newsletter".to_string()],
    disabled_modules: vec![],
    admin: Some(SeedUserRequest {
        tenant_id: Uuid::nil(),
        email: "admin@example.com".to_string(),
        name: "Admin".to_string(),
        password: "password".to_string(),
    }),
    demo_customer_password: Some("password".to_string()),
    actor: "installer".to_string(),
};

let outcome = execute_seed_profile(
    request,
    &tenant_port,
    &principal_port,
    &module_port,
    Some(&content_port),  // Pass content port
).await?;

println!("Created tenant: {}", outcome.tenant.id);
println!("Enabled modules: {:?}", outcome.enabled_modules);
println!("Content outcomes: {:?}", outcome.content_outcomes);
```

## Benefits

1. **Clean separation**: Infrastructure seed separate from content seed
2. **Modular**: Each module owns its content loader
3. **Reusable**: Uses Content Portability platform
4. **Optional**: Content seeding only for Dev profile
5. **Idempotent**: Can run multiple times
6. **Tenant-safe**: All content is tenant-isolated
7. **Extensible**: Easy to add new module loaders

## Next Steps

1. ✅ Create this integration plan
2. ⏳ Implement `SeedContentPort` trait
3. ⏳ Create blog seed loader
4. ⏳ Create newsletter seed loader
5. ⏳ Create seed data files
6. ⏳ Implement composite port
7. ⏳ Update `execute_seed_profile`
8. ⏳ Add tests
9. ⏳ Update documentation
