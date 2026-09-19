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
]);
requireAll("crates/modules/rustok-blog/src/migrations/m20260919_000023_enforce_blog_post_category_tenant_integrity.rs", [
  "fk_blog_posts_tenant_category",
  "ON DELETE SET NULL",
  "blog_posts_category_tenant_insert",
  "blog_posts_category_tenant_update",
  "blog_categories_delete_null_post_category",
  "invalid relations exist",
]);
requireAll("crates/modules/rustok-blog/src/migrations/mod.rs", [
  "mod m20260919_000023_enforce_blog_post_category_tenant_integrity;",
  "Box::new(m20260919_000023_enforce_blog_post_category_tenant_integrity::Migration)",
  '"m20260919_000023_enforce_blog_post_category_tenant_integrity"',
]);

requireAll("crates/modules/rustok-blog/src/services/category_name_projection.rs", [
  "load_scoped_categories_strict",
  "BlogError::invariant(",
  ".map_err(BlogError::from)?",
  "Category without localized copy",
]);
requireAll("crates/modules/rustok-blog/src/services/category_command.rs", [
  ".map_err(storage_category_tree_error)?",
  'BlogError::invariant("Moved category placement was not persisted")',
  '"Blog category Taxonomy hierarchy coverage is incomplete"',
  "Persisted Blog category depth is missing",
]);
forbid("crates/modules/rustok-blog/src/services/category_command.rs", [
  "or_insert((None, 0))",
]);
requireAll("crates/modules/rustok-blog/src/services/category_delete.rs", [
  "TaxonomyError::internal(format!(",
  "BlogError::CategoryNotFound(category_id) => TaxonomyError::TermNotFound(category_id)",
  '"has no canonical Taxonomy hierarchy placement"',
  '"Blog category Taxonomy hierarchy placement disappeared before delete completed"',
  '"Blog category Taxonomy hierarchy coverage is incomplete during sibling canonicalization"',
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
forbid("crates/modules/rustok-blog/src/services/post/repository.rs", [
  "translation_seed_in_tx",
  "baseline.as_ref()",
]);

requireAll("crates/modules/rustok-blog/src/services/tag.rs", [
  "load_term_names_strict",
  "TaxonomyTermKind::Tag",
  "resolve_name_for_locale_chain",
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

if (failures.length > 0) {
  console.error("Canonical module reference-contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Canonical module reference-contract verification passed for rustok-blog.");
