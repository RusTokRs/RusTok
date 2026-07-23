# rustok-blog / CRATE_API

## Public Modules
`controllers`, `dto`, `entities`, `error`, `graphql`, `locale`, `services`, `state_machine`.

## Primary Public Types and Signatures

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
    pub content: RichTextDocument,
    pub parent_comment_id: Option<Uuid>,
}
```

#### UpdateCommentInput
```rust
pub struct UpdateCommentInput {
    pub locale: String,
    pub content: Option<RichTextDocument>,
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
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub post_id: Uuid,
    pub author_id: Option<Uuid>,
    pub content: RichTextView,
    pub content_text: String,
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

#### PostSummary (for listings)
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

## Events
- Publishes: `BlogPostCreated`, `BlogPostPublished`, `BlogPostUnpublished`, `BlogPostUpdated`, `BlogPostArchived`, `BlogPostDeleted`
- Consumes: none

## Dependencies on Other RusToK Crates
- `rustok-content`
- `rustok-comments`
- `rustok-core`
- `rustok-outbox`
- `rustok-taxonomy`

## Common AI Mistakes
- Tries to add separate migrations for blog (the module uses content tables).
- Confuses blog state-machine and content state-machine.
- Skips permission checks (`Resource::Posts`, `Resource::Comments`).
- Returns the first translation without locale fallback instead of using `locale.rs`.
- Passes `UpdateNodeInput` directly instead of `UpdatePostInput` from rustok-blog.
- Does not pass `author_id` from `SecurityContext` when creating a post.
- Uses `Uuid::nil()` as `tenant_id` in event_bus.publish() — should take it from the node.

## Minimum Contract Set

### Input DTOs/Commands
- Input contract is defined by the public DTOs/commands from the crate (see sections with `Create*Input`/`Update*Input`/query/filter above and corresponding `pub` exports in `src/lib.rs`).
- All changes to public DTO fields are considered breaking changes and require synchronized updates to transport adapters in `apps/server`.

### Domain Invariants
- Module invariants are enforced in services/state machines and DTO validation; invalid transitions/parameters must result in a domain error.
- Multi-tenant boundary invariants (tenant/resource isolation, auth context) are considered a mandatory part of the contract.

### Events / Outbox Side Effects
- If the module publishes domain events, publication must go through the transactional outbox/transport contract without local workarounds.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.

### Errors / Failure Codes
- Public `*Error`/`*Result` types of the module define the failure contract and must not lose semantics when mapped to HTTP/GraphQL/CLI.
- For validation/auth/conflict/not-found scenarios, a stable error-class must be maintained, used by tests and adapters.
