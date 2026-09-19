import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const failures = [];

function read(path) {
  return readFileSync(join(repoRoot, path), "utf8");
}
function fail(message) {
  failures.push(message);
}
function requireAll(path, markers) {
  const source = read(path);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${path}: missing reference invariant marker ${marker}`);
  }
}
function forbid(path, markers) {
  const source = read(path);
  for (const marker of markers) {
    if (source.includes(marker)) fail(`${path}: forbidden reference-module pattern ${marker}`);
  }
}
function rustFiles(path) {
  const root = join(repoRoot, path);
  const out = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const full = join(root, entry.name);
    if (entry.isDirectory()) {
      out.push(...rustFiles(relative(repoRoot, full)));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      out.push(relative(repoRoot, full));
    }
  }
  return out;
}

requireAll("crates/modules/rustok-blog/src/dto/post.rs", [
  "pub excerpt: Patch<String>",
  "pub category_id: Patch<Uuid>",
  "pub featured_image_url: Patch<String>",
  "pub seo_title: Patch<String>",
  "pub seo_description: Patch<String>",
  "pub version: i32",
  "pub sort_by: Option<PostSortField>",
  "pub sort_order: Option<PostSortOrder>",
]);
forbid("crates/modules/rustok-blog/src/dto/post.rs", [
  "pub version: Option<i32>",
  "pub search: Option<String>",
]);

requireAll("crates/modules/rustok-blog/src/services/post/commands.rs", [
  "if post.version != version",
  "Column::Version.eq(version)",
  "ensure_transition(current, BlogPostStatus::Published)?",
  "ensure_transition(current, BlogPostStatus::Draft)?",
  "ensure_transition(current, BlogPostStatus::Archived)?",
  "pub async fn restore_post(",
  '"Locale is required when changing localized Blog post copy or tags"',
]);
forbid("crates/modules/rustok-blog/src/services/post/commands.rs", [
  "PLATFORM_FALLBACK_LOCALE",
  'expect("localized change requires a canonical locale")',
  'expect("tag mutation requires a canonical locale")',
  'expect("localized-only update requires a canonical locale")',
]);

requireAll("crates/modules/rustok-blog/src/services/post/mod.rs", [
  "other => Err(BlogError::invariant(format!(",
  '"Unknown persisted Blog post status: {other}"',
]);
forbid("crates/modules/rustok-blog/src/services/post/mod.rs", [
  '"Unknown blog post status: {other}"',
]);

requireAll("crates/modules/rustok-blog/src/services/rbac.rs", [
  "Resource::BlogPosts, Action::Read",
  "Permission::BLOG_POSTS_READ",
]);
forbid("crates/modules/rustok-blog/src/services/rbac.rs", [
  "Resource::Posts, Action::Read",
  "Permission::POSTS_READ",
]);

requireAll("crates/modules/rustok-blog/src/services/category_owner.rs", [
  "load_scoped_categories_strict",
  'BlogError::invariant(\n                "Blog Category delete requires host-composed Taxonomy capability cleanup"',
  '"Blog Category Taxonomy projection coverage is incomplete"',
  '"Blog Category Taxonomy projection contains duplicate identities"',
  '"Blog Category Taxonomy projection contains Category without localized copy"',
]);
requireAll("crates/modules/rustok-blog/src/services/category.rs", [
  '"has no canonical Taxonomy hierarchy placement"',
  "Column::ParentTermId.eq(category_id)",
  '"Category must be a leaf before deletion; move or delete its children first"',
]);
requireAll("crates/modules/rustok-blog/src/migrations/m20260919_000023_enforce_blog_post_category_tenant_integrity.rs", [
  "fk_blog_posts_tenant_category",
  "uq_blog_categories_tenant_id",
  "blog_posts_category_tenant_insert",
  "blog_posts_category_tenant_update",
  "blog_categories_delete_null_post_category",
  "invalid relations exist",
]);
requireAll("crates/modules/rustok-blog/src/migrations/mod.rs", [
  "mod m20260919_000023_enforce_blog_post_category_tenant_integrity;",
  "mod m20260919_000024_enforce_blog_post_channel_visibility_tenant_integrity;",
  "mod m20260919_000025_fix_blog_post_category_tenant_delete_action;",
  "Box::new(m20260919_000023_enforce_blog_post_category_tenant_integrity::Migration)",
  "Box::new(m20260919_000025_fix_blog_post_category_tenant_delete_action::Migration)",
  '"m20260919_000023_enforce_blog_post_category_tenant_integrity"',
  '"m20260919_000025_fix_blog_post_category_tenant_delete_action"',
]);

requireAll("crates/modules/rustok-blog/src/migrations/m20260919_000025_fix_blog_post_category_tenant_delete_action.rs", [
  "fk_blog_posts_tenant_category",
  "ON DELETE SET NULL (category_id)",
  "DROP CONSTRAINT IF EXISTS",
  "Intentionally irreversible",
]);
forbid("crates/modules/rustok-blog/src/migrations/m20260919_000023_enforce_blog_post_category_tenant_integrity.rs", [
  "ON DELETE SET NULL;\n",
]);

requireAll("crates/modules/rustok-blog/src/services/category_name_projection.rs", [
  "load_scoped_categories_strict",
  "BlogError::invariant(",
  ".map_err(BlogError::from)?",
  "Category without localized copy",
]);
requireAll("crates/modules/rustok-blog/src/services/category.rs", [
  "rustok_taxonomy::lock_category_hierarchy_writer_in_tx(&txn, tenant_id).await?",
  "// Serialize before reading hierarchy so a concurrent structural move cannot be",
  "self.publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id)",
]);

requireAll("crates/modules/rustok-blog/src/services/category_command.rs", [
  "rustok_taxonomy::lock_category_hierarchy_writer_in_tx(&txn, tenant_id).await?",
  ".map_err(storage_category_tree_error)?",
  'BlogError::invariant("Moved category placement was not persisted")',
  '"Blog category Taxonomy hierarchy coverage is incomplete"',
  "Persisted Blog category depth is missing",
]);
forbid("crates/modules/rustok-blog/src/services/category_command.rs", [
  "or_insert((None, 0))",
]);
requireAll("crates/modules/rustok-blog/src/services/category_delete.rs", [
  "rustok_taxonomy::lock_category_hierarchy_writer_in_tx(txn, tenant_id).await?",
  "TaxonomyError::internal(format!(",
  "BlogError::CategoryNotFound(category_id) => TaxonomyError::TermNotFound(category_id)",
  '"has no canonical Taxonomy hierarchy placement"',
  '"Blog category Taxonomy hierarchy placement disappeared before delete completed"',
  "hierarchy_rows.len() != blog_category_ids.len()",
  '"Blog category Taxonomy hierarchy coverage is incomplete during sibling canonicalization"',
  "detach_category_from_posts_in_tx",
  "blog_post::Column::CategoryId.eq(category_id)",
  "blog_post::Column::Version.eq(post.version)",
  "checked_add(1)",
  '"Blog post {} changed before category detachment could commit"',
]);

requireAll("crates/modules/rustok-blog/src/services/post/repository.rs", [
  "load_channel_slugs(
        &self,
        tenant_id: Uuid",
  "load_channel_slugs_map(
        &self,
        tenant_id: Uuid",
  "validate_persisted_version",
  "next_persisted_version",
  '"invalid persisted version"',
  '"Title is required for a new locale"',
  '"Content is required for a new locale"',
]);
requireAll("crates/modules/rustok-blog/src/services/post/queries.rs", [
  "pub async fn list_public_visible_with_locale_fallback(",
  "Self::validate_persisted_version(&post)?",
]);
forbid("crates/modules/rustok-blog/src/services/post/repository.rs", [
  "translation_seed_in_tx",
  "baseline.as_ref()",
]);

requireAll("crates/modules/rustok-blog/src/services/tag.rs", [
  "load_term_names_strict",
  "TaxonomyTermKind::Tag",
  "resolve_name_for_locale_chain",
  "lock_module_term_in_tx(",
  "detach_tag_from_posts_in_tx",
  "blog_post::Column::Version.eq(post.version)",
  '"Blog post {} changed before Tag detachment could commit"',
  "blog_post_tag::Entity::delete_many()",
  "blog_post_tag::Column::TenantId.eq(tenant_id)",
  "blog_post_tag::Column::PostId.eq(post_id)",
  "blog_post_tag::Column::TagId.eq(tag_id)",
  "tenant_id: Set(tenant_id)",
  "ensure_terms_for_module_in_tx(",
]);

requireAll("crates/modules/rustok-blog/src/services/category.rs", [
  "rustok_taxonomy::lock_category_hierarchy_writer_in_tx(&txn, tenant_id).await?",
  "ensure_hierarchy_coverage_in_tx(&txn, tenant_id).await?",
  '"Blog category Taxonomy hierarchy coverage is incomplete before create"',
]);
requireAll("crates/modules/rustok-blog/src/services/category.rs", [
  "rustok_taxonomy::lock_category_hierarchy_writer_in_tx(&txn, tenant_id).await?",
  "// Serialize before reading hierarchy so a concurrent structural move cannot be",
]);
requireAll("crates/modules/rustok-blog/src/module.rs", [
  '"dependencies"',
  '["content", "comments", "taxonomy", "outbox", "channel"]',
]);
requireAll("crates/modules/rustok-blog/rustok-module.toml", [
  'taxonomy = { version_req = ">=0.1.0" }',
  'channel = { version_req = ">=0.1.0" }',
]);
requireAll("crates/modules/rustok-blog/src/module.rs", [
  "Resource::Tags",
  "Action::Create",
  "Action::Read",
  "Action::Update",
  "Action::Delete",
  "Action::List",
  "Action::Manage",
]);
forbid("crates/modules/rustok-blog/src/services/category.rs", [
  "blog-category-tree:",
]);
requireAll("crates/modules/rustok-blog/src/services/category_taxonomy_sync.rs", [
  ".map_err(BlogError::from)",
  "BLOG_TAXONOMY_SCOPE",
]);
forbid("crates/modules/rustok-blog/src/services/category_taxonomy_sync.rs", [
  "map_taxonomy_error",
  "BlogError::Validation(format!(",
]);

requireAll("crates/modules/rustok-blog/src/migrations/m20260919_000024_enforce_blog_post_channel_visibility_tenant_integrity.rs", [
  "fk_blog_post_channel_visibility_tenant_post",
  "blog_post_channel_visibility_tenant_insert",
  "blog_post_channel_visibility_tenant_update",
  "invalid relations exist",
]);
requireAll("crates/modules/rustok-blog/src/migrations/mod.rs", [
  "mod m20260919_000024_enforce_blog_post_channel_visibility_tenant_integrity;",
  "Box::new(m20260919_000024_enforce_blog_post_channel_visibility_tenant_integrity::Migration)",
  '"m20260919_000024_enforce_blog_post_channel_visibility_tenant_integrity"',
]);

requireAll("crates/modules/rustok-blog/src/services/comment_projection.rs", [
  "DomainEvent::ReindexRequested",
  "Column::CommentCount.eq(post.comment_count)",
  "fn next_comment_count(",
  "let post_updated =",
  "return Ok(false);",
  "if post_updated",
]);
forbid("crates/modules/rustok-blog/src/services/comment_projection.rs", [
  "blog_post::Column::Version",
  "blog_post::Column::UpdatedAt",
  "DomainEvent::BlogPostUpdated {\n                    post_id: change.post_id",
]);

requireAll("crates/modules/rustok-blog/src/graphql/query.rs", [
  "Err(BlogError::Forbidden(_)) if is_public_request(ctx) => return Ok(None)",
  "query_tenant_id(ctx, tenant, tenant_id)?",
  '"Blog queries must use the current tenant"',
  '"Blog is not available for the current channel"',
]);
requireAll("crates/modules/rustok-blog/src/graphql/mutation.rs", [
  "mutation_tenant_id(tenant, &auth, tenant_id)?",
  '"Blog mutations must use the current tenant"',
]);

for (const path of rustFiles("crates/modules/rustok-blog/src/graphql")) {
  forbid(path, [
    "async_graphql::Error::new(err.to_string())",
    "async_graphql::Error::new(error.to_string())",
    "format!(\"Channel module check failed: {error}\")",
    "resolved via ",
    "request_channel_resolution_source(",
  ]);
}
for (const path of rustFiles("crates/modules/rustok-blog/src/integrations")) {
  forbid(path, ["crate::entities", "crate::{entities", "crate::entities::"]);
}

requireAll("crates/modules/rustok-blog/src/error/public.rs", [
  "pub struct BlogPublicError",
  "ErrorKind::Database | ErrorKind::Internal",
  '"The Blog operation could not be completed"',
]);

requireAll("crates/modules/rustok-blog/src/error/public.rs", [
  "pub fn internal() -> Self",
  "StatusCode::INTERNAL_SERVER_ERROR.as_u16()",
]);
requireAll("crates/modules/rustok-blog/src/error/mod.rs", [
  "TaxonomyTermNotFound(Uuid)",
  "TaxonomyError::Internal(message) => Self::Invariant",
  "TaxonomyError::Conflict(message) => Self::Conflict(message)",
  "TaxonomyError::TermNotFound(term_id)",
]);
requireAll("crates/modules/rustok-taxonomy/src/error.rs", [
  '#[error("Taxonomy internal operation failed")]',
  "Internal(String)",
  "pub fn internal(message: impl Into<String>) -> Self",
]);
requireAll("crates/modules/rustok-blog/src/dto/comment.rs", [
  "pub command_id: Uuid",
  "Stable identity of this logical create command. Reuse across retries.",
]);
requireAll("crates/modules/rustok-blog/src/services/comment.rs", [
  "input.command_id",
  ".with_idempotency_key(",
  "Self::ensure_blog_target(&existing)?;",
  "comments_read_port_context(",
]);

requireAll("crates/modules/rustok-blog/admin/src/model.rs", [
  "pub version: i32",
  "pub version: Option<i32>",
]);
requireAll("crates/modules/rustok-blog/admin/src/transport/graphql_adapter.rs", [
  "draft.version.ok_or_else",
]);
requireAll("crates/modules/rustok-blog/admin/src/transport/native_server_adapter.rs", [
  "draft.version.ok_or_else",
  "fn public_internal_error() -> ServerFnError",
  "Err(error) => Err(public_blog_error(error))",
  "use_context::<HostRuntimeContext>().ok_or_else(public_internal_error)?",
]);
requireAll("crates/modules/rustok-blog/storefront/src/transport/native_server_adapter.rs", [
  "fn public_internal_error() -> ServerFnError",
  "use_context::<HostRuntimeContext>().ok_or_else(public_internal_error)?",
  '"Blog is not available for the current channel"',
  "context.tenant_id != tenant_id",
]);
for (const path of [
  "crates/modules/rustok-blog/admin/src/transport/native_server_adapter.rs",
  "crates/modules/rustok-blog/storefront/src/transport/native_server_adapter.rs",
]) {
  forbid(path, [
    "expect_context::<HostRuntimeContext>()",
    ".map_err(ServerFnError::new)?",
    "TransactionalEventBus in host runtime context",
    "Err(error) => Err(ServerFnError::new(error))",
  ]);
}

forbid("crates/modules/rustok-blog/src/lib.rs", [
  "pub mod entities;",
  "pub use entities::",
]);

forbid("crates/modules/rustok-blog/src/services/category_command.rs", [
  "blog-category-tree:",
]);
forbid("crates/modules/rustok-blog/src/services/category_delete.rs", [
  "blog-category-tree:",
]);

requireAll("crates/modules/rustok-taxonomy/src/category_hierarchy.rs", [
  "pub async fn lock_category_hierarchy_writer_in_tx(",
  "pg_advisory_xact_lock(hashtextextended($1, 0))",
]);

requireAll("crates/modules/rustok-taxonomy/src/owner_category_read.rs", [
  "Taxonomy Category hierarchy contains a missing or foreign-scope parent",
  "Taxonomy Category hierarchy contains an invalid position or self-parent",
]);
requireAll("crates/modules/rustok-taxonomy/src/module_term_mutation.rs", [
  ".filter(taxonomy_term::Column::ScopeValue.eq(module_scope))",
  ".lock_exclusive()",
]);
requireAll("crates/modules/rustok-taxonomy/src/services.rs", [
  '"Module-owned Taxonomy terms must be updated by their owning module"',
  '"Module-owned Taxonomy terms must be deleted by their owning module"',
  ".lock_exclusive()",
]);
requireAll("crates/modules/rustok-taxonomy/src/services.rs", [
  "let Some(term) = taxonomy_term::Entity::find_by_id(route.term_id)",
  ".lock_exclusive()",
  "let Some(term) = taxonomy_term::Entity::find()",
]);
requireAll("crates/modules/rustok-channel/src/services/channel_service.rs", [
  "pub async fn is_module_enabled_for_tenant(",
]);
requireAll("crates/modules/rustok-channel/src/services/channel_service.rs", [
  "pub async fn is_module_enabled(",
  "if !channel.is_active",
  "pub async fn is_module_enabled_for_tenant(",
]);
requireAll("crates/modules/rustok-blog/src/graphql/query.rs", [
  "is_module_enabled_for_tenant(tenant_id, channel_id, MODULE_SLUG)",
]);
requireAll("crates/modules/rustok-blog/storefront/src/transport/native_server_adapter.rs", [
  "is_module_enabled_for_tenant(tenant_id, channel_id, MODULE_SLUG)",
  "request_context",
  ".is_some_and(|context| context.tenant_id != tenant_id)",
]);
requireAll("crates/modules/rustok-comments/src/services.rs", [
  "Comment position is exhausted for thread",
  "active.comment_count = Set(thread.comment_count)",
  ".lock_exclusive()",
]);
requireAll("crates/modules/rustok-comments/src/entities/comment_thread.rs", [
  "self.last_commented_at = Set(live_comments.first().map(|comment| comment.created_at))",
]);
requireAll("crates/modules/rustok-blog/src/dto/post.rs", [
  "saturating_add(u64::from(per_page).saturating_sub(1))",
  "u32::try_from(total_pages).unwrap_or(u32::MAX)",
]);
requireAll("crates/modules/rustok-blog/src/error/public.rs", [
  "let internal = matches!(rich.kind, ErrorKind::Database | ErrorKind::Internal)",
  "ErrorKind::Internal.error_code().to_string()",
  "The Blog operation could not be completed",
]);

requireAll("crates/modules/rustok-taxonomy/src/owner_category_route_sync.rs", [
  "crate::lock_category_hierarchy_writer_in_tx(txn, tenant_id).await?",
]);
forbid("crates/modules/rustok-taxonomy/src/owner_category_route_sync.rs", [
  "serialize_category_hierarchy_writer",
]);

requireAll("crates/modules/rustok-taxonomy/src/owner_category_sync.rs", [
  "lock_category_hierarchy_writer_in_tx(txn, tenant_id).await?",
]);
forbid("crates/modules/rustok-taxonomy/src/owner_category_sync.rs", [
  "serialize_category_hierarchy_writer",
]);

requireAll("crates/modules/rustok-taxonomy/src/category_delete.rs", [
  "crate::lock_category_hierarchy_writer_in_tx(&txn, tenant_id).await?",
  ".lock_exclusive()",
]);

requireAll("crates/modules/rustok-blog/src/graphql/mutation.rs", [
  "let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;",
  "ensure_public_blog_channel_enabled(",
  '"Permission denied: comments:create required"',
]);

requireAll("crates/modules/rustok-taxonomy/src/owner_read.rs", [
  "pub async fn load_term_names_strict_for_module(",
  "Taxonomy owner attachment references a term outside the allowed module/global scope",
  "TaxonomyScopeType::Global",
  "TaxonomyScopeType::Module",
]);

requireAll("crates/modules/rustok-blog/src/services/tag.rs", [
  "load_term_names_strict_for_module(",
  "BLOG_SCOPE_VALUE",
]);
requireAll("crates/modules/rustok-blog/src/services/tag.rs", [
  "enforce_scope(&security, Resource::Tags, Action::Create)?;",
  ".create_module_term_in_tx(",
  "publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id)",
]);
forbid("crates/modules/rustok-blog/src/services/tag.rs", [
  "CreateTaxonomyTermInput",
  ".create_term(",
]);

requireAll("crates/modules/rustok-blog/src/services/category.rs", [
  "TaxonomyOwnerCategoryReader::load_scoped_categories_in_strict(",
  "category_taxonomy_sync::BLOG_TAXONOMY_SCOPE",
  "Blog category {category_id} is missing canonical Taxonomy ownership or hierarchy",
  "Blog category {category_id} has no canonical localized copy",
]);
requireAll("crates/modules/rustok-blog/src/services/post/commands.rs", [
  "if publish {",
  "enforce_scope(&security, Resource::BlogPosts, Action::Publish)?;",
]);
requireAll("crates/modules/rustok-blog/src/services/post/mod.rs", [
  "const MAX_POST_METADATA_BYTES: usize = 64 * 1024;",
  "const MAX_POST_CHANNEL_SLUGS: usize = 32;",
  "const MAX_POST_CHANNEL_SLUG_BYTES: usize = 100;",
  "Post metadata cannot exceed",
  "A post cannot target more than",
]);

requireAll("crates/modules/rustok-taxonomy/src/route_key_registry.rs", [
  "localized taxonomy route key",
  "is_unique_constraint(&error)",
]);
requireAll("crates/modules/rustok-taxonomy/src/translation_evidence.rs", [
  "reconcile_route_keys_for_locale_in_tx(",
  'if evidence.operation != "delete"',
]);

requireAll("crates/modules/rustok-blog/src/services/post/mod.rs", [
  "const MAX_POST_METADATA_BYTES: usize = 64 * 1024;",
  "const MAX_POST_CHANNEL_SLUGS: usize = 32;",
  "const MAX_POST_CHANNEL_SLUG_BYTES: usize = 100;",
  "Post metadata cannot exceed",
  "A post cannot target more than",
  "Channel slugs cannot exceed",
]);

requireAll("crates/modules/rustok-taxonomy/src/module_term_mutation.rs", [
  "taxonomy_term_alias::Entity::delete_many()",
  "if existing.slug != slug",
  "let has_existing_alias = taxonomy_term_alias::Entity::find()",
]);

requireAll("crates/modules/rustok-taxonomy/src/owner_category_route_sync.rs", [
  "aliases.remove(&next_slug);",
]);

requireAll("crates/modules/rustok-blog/src/services/post/commands.rs", [
  "security.get_scope(Resource::Tags, Action::Create)",
  "sync_post_tags_in_tx(",
]);

requireAll("crates/modules/rustok-blog/src/services/tag.rs", [
  "ensure_module_terms_for_owner_in_tx(",
  "allow_create",
]);

requireAll("crates/modules/rustok-taxonomy/src/services.rs", [
  "pub async fn ensure_module_terms_for_owner_in_tx(",
  "allow_create",
]);

requireAll("crates/modules/rustok-blog/src/services/post/repository.rs", [
  "ensure_channel_slugs_exist_for_tenant_in_tx(",
]);

requireAll("crates/modules/rustok-taxonomy/src/services.rs", [
  "if input.scope_type == TaxonomyScopeType::Module",
  "Module-owned Taxonomy terms must be created by the owning module",
]);

requireAll("crates/modules/rustok-blog/src/services/tag.rs", [
  "enforce_scope(&security, Resource::Tags, Action::Update)?;",
  "enforce_scope(&security, Resource::Tags, Action::Delete)?;",
  "ensure_module_owned_term(&term)?;",
]);

requireAll("crates/modules/rustok-blog/src/services/comment.rs", [
  'comments_write_port_context(',
  '                    &security,',
  '                    "moderate",',
]);

if (failures.length > 0) {
  console.error("Canonical module reference-contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Canonical module reference-contract verification passed for rustok-blog.");
