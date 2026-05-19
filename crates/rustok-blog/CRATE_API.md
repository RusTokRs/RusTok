# rustok-blog / CRATE_API

## Публичные модули
`controllers`, `dto`, `entities`, `error`, `graphql`, `locale`, `services`, `state_machine`.

## Основные публичные типы и сигнатуры

### BlogModule
```rust
pub struct BlogModule;
impl RusToKModule for BlogModule { ... }
impl MigrationSource for BlogModule { fn migrations() -> Vec<Box<dyn MigrationTrait>> }
```

### Transport entry points
```rust
pub mod graphql {
    pub struct BlogQuery;
    pub struct BlogMutation;
}

pub mod controllers {
    pub fn routes() -> Routes;
}
```

### PostService
```rust
pub struct PostService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl PostService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self;
    pub async fn create_post(tenant_id, security, input: CreatePostInput) -> BlogResult<Uuid>;
    pub async fn update_post(post_id, security, input: UpdatePostInput) -> BlogResult<()>;
    pub async fn publish_post(post_id, security) -> BlogResult<()>;
    pub async fn unpublish_post(post_id, security) -> BlogResult<()>;
    pub async fn archive_post(post_id, security, reason: Option<String>) -> BlogResult<()>;
    pub async fn delete_post(post_id, security) -> BlogResult<()>;
    pub async fn get_post(tenant_id, security, post_id, locale: &str) -> BlogResult<PostResponse>;
    pub async fn get_post_with_locale_fallback(tenant_id, security, post_id, locale: &str, fallback_locale: Option<&str>) -> BlogResult<PostResponse>;
    pub async fn list_posts(tenant_id, security, query: PostListQuery) -> BlogResult<PostListResponse>;
    pub async fn list_public_visible_with_locale_fallback(tenant_id, query: PostListQuery, fallback_locale: Option<&str>, channel_slug: Option<&str>) -> BlogResult<PostListResponse>;
    pub async fn get_post_by_slug(tenant_id, security, locale: &str, slug: &str) -> BlogResult<Option<PostResponse>>;
    pub async fn get_posts_by_tag(tenant_id, security, tag, page, per_page) -> BlogResult<PostListResponse>;
    pub async fn get_posts_by_category(tenant_id, security, category_id, page, per_page) -> BlogResult<PostListResponse>;
    pub async fn get_posts_by_author(tenant_id, security, author_id, page, per_page) -> BlogResult<PostListResponse>;
}
```


### CommentService
```rust
pub struct CommentService {
    comments: CommentsService,
    event_bus: TransactionalEventBus,
}

impl CommentService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self;
    pub async fn create_comment(tenant_id, security, post_id, input: CreateCommentInput) -> BlogResult<CommentResponse>;
    pub async fn get_comment(tenant_id, comment_id, locale: &str) -> BlogResult<CommentResponse>;
    pub async fn update_comment(tenant_id, comment_id, security, input: UpdateCommentInput) -> BlogResult<CommentResponse>;
    pub async fn moderate_comment(tenant_id, comment_id, security, input: ModerateCommentInput, fallback_locale: Option<&str>) -> BlogResult<CommentResponse>;
    pub async fn delete_comment(tenant_id, comment_id, security) -> BlogResult<()>;
    pub async fn list_for_post(tenant_id, security, post_id, filter: ListCommentsFilter) -> BlogResult<(Vec<CommentListItem>, u64)>;
}
```

### DTO


#### CreateCommentInput
```rust
pub struct CreateCommentInput {
    pub locale: String,
    pub content: String,
    pub parent_comment_id: Option<Uuid>,
}
```

#### UpdateCommentInput
```rust
pub struct UpdateCommentInput {
    pub locale: String,
    pub content: Option<String>,
}
```

#### ModerateCommentInput
```rust
pub enum ModerateCommentStatus {
    Approved,
    Spam,
    Trash,
}

pub struct ModerateCommentInput {
    pub status: ModerateCommentStatus,
    pub locale: Option<String>,
}
```

#### CommentResponse
```rust
pub struct CommentResponse {
    pub id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub post_id: Uuid,
    pub author_id: Option<Uuid>,
    pub content: String,
    pub status: String,
    pub parent_comment_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
}
```

#### CreatePostInput
```rust
pub struct CreatePostInput {
    pub locale: String,
    pub title: String,            // max 512
    pub body: String,
    pub excerpt: Option<String>,  // max 1000
    pub slug: Option<String>,     // max 255
    pub publish: bool,
    pub tags: Vec<String>,        // max 20
    pub category_id: Option<Uuid>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Option<Vec<String>>,
    pub metadata: Option<Value>,
}
```

#### UpdatePostInput
```rust
pub struct UpdatePostInput {
    pub locale: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
    pub excerpt: Option<String>,
    pub slug: Option<String>,
    pub tags: Option<Vec<String>>,
    pub category_id: Option<Uuid>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Option<Vec<String>>,
    pub metadata: Option<Value>,
    pub version: Option<i32>,
}
```

#### PostResponse
```rust
pub struct PostResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub author_id: Uuid,
    pub title: String,
    pub slug: String,
    pub locale: String,             // requested locale
    pub effective_locale: String,   // actual locale used (after fallback)
    pub available_locales: Vec<String>,
    pub body: String,
    pub body_format: String,
    pub excerpt: Option<String>,
    pub status: BlogPostStatus,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub tags: Vec<String>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Vec<String>,
    pub metadata: Value,
    pub comment_count: i64,
    pub view_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub version: i32,
}
```

#### PostSummary (для листингов)
```rust
pub struct PostSummary {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub locale: String,
    pub effective_locale: String,
    pub excerpt: Option<String>,
    pub status: BlogPostStatus,
    pub author_id: Uuid,
    pub author_name: Option<String>,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub tags: Vec<String>,
    pub featured_image_url: Option<String>,
    pub channel_slugs: Vec<String>,
    pub comment_count: i64,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
```

### State Machine
```rust
pub struct BlogPost<S>;       // generic over Draft/Published/Archived
pub enum BlogPostStatus { Draft, Published, Archived }
pub enum CommentStatus { Pending, Approved, Spam, Trash }
pub struct Draft { created_at, updated_at }
pub struct Published { published_at, updated_at }
pub struct Archived { archived_at, reason }
pub trait ToBlogPostStatus { fn to_status(&self) -> BlogPostStatus; }
```

### Locale module
```rust
pub fn resolve_translation<'a>(translations: &'a [NodeTranslationResponse], requested: &str) -> ResolvedTranslation<'a>;
pub fn resolve_body<'a>(bodies: &'a [BodyResponse], requested: &str) -> ResolvedBody<'a>;
pub fn available_locales(translations: &[NodeTranslationResponse]) -> Vec<String>;
```

### Channel visibility
- Wire-level `channel_slugs` / `channelSlugs` contract is preserved for create,
  update, detail, and list surfaces.
- Canonical persistence is typed relation `blog_post_channel_visibility`, not
  metadata.
- Public GraphQL read-path filters published posts at DB level through that
  relation; empty allowlists remain globally visible.

### Tag vocabulary
- Wire-level `tags: Vec<String>` contract is preserved for post create, update,
  detail, and list surfaces.
- Canonical tag identity now lives in shared `rustok-taxonomy`
  (`taxonomy_terms`, `taxonomy_term_translations`, `taxonomy_term_aliases`).
- `rustok-blog` keeps `blog_post_tags` as the module-owned relation table and
  resolves/creates module-scoped tags transactionally while reusing matching
  global taxonomy terms.

## События
- Публикует: `BlogPostCreated`, `BlogPostPublished`, `BlogPostUnpublished`, `BlogPostUpdated`, `BlogPostArchived`, `BlogPostDeleted`
- Потребляет: нет

## Зависимости от других rustok-крейтов
- `rustok-content`
- `rustok-comments`
- `rustok-core`
- `rustok-outbox`
- `rustok-taxonomy`

## Частые ошибки ИИ
- Пытается добавлять отдельные миграции для blog (модуль использует таблицы content).
- Путает blog state-machine и content state-machine.
- Пропускает проверку permissions (`Resource::Posts`, `Resource::Comments`).
- Возвращает первый перевод без locale fallback вместо использования `locale.rs`.
- Передаёт `UpdateNodeInput` напрямую вместо `UpdatePostInput` из rustok-blog.
- Не передаёт `author_id` из `SecurityContext` при создании поста.
- Использует `Uuid::nil()` как `tenant_id` в event_bus.publish() — нужно брать из узла.

## Минимальный набор контрактов

### Входные DTO/команды
- Входной контракт формируется публичными DTO/командами из crate (см. разделы с `Create*Input`/`Update*Input`/query/filter выше и соответствующие `pub`-экспорты в `src/lib.rs`).
- Все изменения публичных полей DTO считаются breaking-change и требуют синхронного обновления transport-адаптеров `apps/server`.

### Доменные инварианты
- Инварианты модуля фиксируются в сервисах/стейт-машинах и валидации DTO; недопустимые переходы/параметры должны завершаться доменной ошибкой.
- Инварианты multi-tenant boundary (tenant/resource isolation, auth context) считаются обязательной частью контракта.

### События / outbox-побочные эффекты
- Если модуль публикует доменные события, публикация должна идти через транзакционный outbox/transport-контракт без локальных обходов.
- Формат event payload и event-type должен оставаться обратно-совместимым для межмодульных потребителей.

### Ошибки / коды отказов
- Публичные `*Error`/`*Result` типы модуля определяют контракт отказов и не должны терять семантику при маппинге в HTTP/GraphQL/CLI.
- Для validation/auth/conflict/not-found сценариев должен сохраняться устойчивый error-class, используемый тестами и адаптерами.
