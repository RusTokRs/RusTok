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
]);

requireAll("crates/modules/rustok-blog/src/services/post/repository.rs", [
  '"Title is required for a new locale"',
  '"Content is required for a new locale"',
]);
forbid("crates/modules/rustok-blog/src/services/post/repository.rs", [
  "translation_seed_in_tx",
  "baseline.as_ref()",
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
  "query_tenant_id(ctx, tenant, tenant_id)?",
  '"Blog queries must use the current tenant"',
]);
requireAll("crates/modules/rustok-blog/src/graphql/mutation.rs", [
  "mutation_tenant_id(tenant, &auth, tenant_id)?",
  '"Blog mutations must use the current tenant"',
]);

for (const path of rustFiles("crates/modules/rustok-blog/src/graphql")) {
  forbid(path, ["async_graphql::Error::new(err.to_string())", "async_graphql::Error::new(error.to_string())"]);
}
for (const path of rustFiles("crates/modules/rustok-blog/src/integrations")) {
  forbid(path, ["crate::entities", "crate::{entities", "crate::entities::"]);
}

requireAll("crates/modules/rustok-blog/src/error/public.rs", [
  "pub struct BlogPublicError",
  "ErrorKind::Database | ErrorKind::Internal",
  '"The Blog operation could not be completed"',
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
]);

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
