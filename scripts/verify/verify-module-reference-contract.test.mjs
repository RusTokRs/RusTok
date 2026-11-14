import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const verifier = join(repoRoot, "scripts/verify/verify-module-reference-contract.mjs");
const blog = "crates/modules/rustok-blog";

// Snapshot real owner sources instead of manufacturing a file of passing markers.
// These tests establish the guard's rejection behavior, never runtime readiness.
const sources = [
  `${blog}/src`,
  `${blog}/admin/src`,
  `${blog}/storefront/src`,
  `${blog}/docs/implementation-plan.md`,
  `${blog}/build.rs`,
  `${blog}/rustok-module.toml`,
  "crates/modules/rustok-taxonomy/src",
  "crates/modules/rustok-channel/src/services/channel_service.rs",
  "crates/modules/rustok-comments/src/services.rs",
  "crates/modules/rustok-comments/src/entities/comment_thread.rs",
  "crates/libs/rustok-core/src/error/mod.rs",
  "crates/libs/rustok-core/src/registry.rs",
  "scripts/verify/verify-blog-comments-event-projection.mjs",
  "scripts/verify/verify-blog-comments-event-projection.test.mjs",
];

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "rustok-reference-contract-"));
  t.after(() => {
    assert.equal(dirname(root), resolve(tmpdir()));
    assert.match(basename(root), /^rustok-reference-contract-/);
    rmSync(root, { recursive: true, force: true });
  });
  for (const path of sources) {
    const target = join(root, path);
    mkdirSync(dirname(target), { recursive: true });
    cpSync(join(repoRoot, path), target, { recursive: true });
  }
  return root;
}

function verify(root) {
  const result = spawnSync(process.execPath, [verifier], {
    cwd: repoRoot,
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
  assert.ifError(result.error);
  return { status: result.status, output: `${result.stdout}${result.stderr}` };
}

function replace(root, path, before, after, all = false) {
  const target = join(root, path);
  const source = readFileSync(target, "utf8").replaceAll("\r\n", "\n");
  assert.ok(source.includes(before), `fixture mutation drifted: ${path}: ${before}`);
  writeFileSync(target, all ? source.replaceAll(before, after) : source.replace(before, after));
}

test("reference source guard accepts the unchanged owner snapshot", (t) => {
  const result = verify(fixture(t));
  assert.equal(result.status, 0, result.output);
});

const regressions = [
  {
    name: "creation command identity becomes optional",
    path: `${blog}/src/dto/post.rs`,
    before: "pub command_id: Uuid",
    after: "pub command_id: Option<Uuid>",
    diagnostic: /missing reference invariant marker pub command_id: Uuid/,
  },
  {
    name: "creation command bypasses receipt admission",
    path: `${blog}/src/services/post/commands/create.rs`,
    before: "idempotency::admit(",
    after: "untracked_admission(",
    diagnostic: /missing reference invariant marker idempotency::admit/,
  },
  {
    name: "creation commits without receipt completion",
    path: `${blog}/src/services/post/commands/create.rs`,
    before: "idempotency::complete(",
    after: "untracked_completion(",
    diagnostic: /missing reference invariant marker idempotency::complete/,
  },
  {
    name: "native category commands lose nullable field semantics",
    path: `${blog}/admin/src/transport/native_server_adapter/categories.rs`,
    before: "input = leptos::server_fn::codec::Json",
    after: "input = leptos::server_fn::codec::PostUrl",
    diagnostic: /native admin inputs must preserve JSON null, number and string types/,
  },
  {
    name: "native browser calls drop canonical authentication metadata",
    path: `${blog}/admin/src/transport/native_server_adapter.rs`,
    before: "client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient",
    after: "client = leptos::server_fn::client::browser::BrowserClient",
    diagnostic: /native admin browser calls must use canonical auth metadata/,
  },
  {
    name: "native edit encoding loses JSON field semantics",
    path: `${blog}/admin/src/transport/native_server_adapter.rs`,
    before: "input = leptos::server_fn::codec::Json",
    after: "input = leptos::server_fn::codec::PostUrl",
    diagnostic: /native admin inputs must preserve JSON null, number and string types/,
  },
  {
    name: "native widget encoding loses numeric property types",
    path: `${blog}/admin/src/transport/native_server_adapter/widget.rs`,
    before: "input = leptos::server_fn::codec::Json",
    after: "input = leptos::server_fn::codec::PostUrl",
    diagnostic: /native admin inputs must preserve JSON null, number and string types/,
  },
  {
    name: "the mutation surface no longer matches the source contract",
    path: `${blog}/src/graphql/mutation.rs`,
    before: "    async fn ",
    after: "    fn ",
    all: true,
    diagnostic: /expected GraphQL mutation methods/,
  },
  {
    name: "nullable update loses Keep/Set/Clear",
    path: `${blog}/src/dto/post.rs`,
    before: "pub excerpt: Patch<String>",
    after: "pub excerpt: Option<String>",
    diagnostic: /missing reference invariant marker pub excerpt: Patch<String>/,
  },
  {
    name: "backend edit revision becomes optional",
    path: `${blog}/src/dto/post.rs`,
    before: "pub version: i32",
    after: "pub version: Option<i32>",
    diagnostic: /forbidden reference-module pattern pub version: Option<i32>/,
  },
  {
    name: "admin edit revision becomes optional while response revision remains required",
    path: `${blog}/admin/src/model.rs`,
    before: "    pub scheduled_at: Patch<String>,\n    pub version: i32,",
    after: "    pub scheduled_at: Patch<String>,\n    pub version: Option<i32>,",
    diagnostic: /forbidden reference-module pattern pub version: Option<i32>/,
  },
  {
    name: "admin edit loses nullable Patch semantics",
    path: `${blog}/admin/src/model.rs`,
    before: "pub excerpt: Patch<String>",
    after: "pub excerpt: Option<String>",
    diagnostic: /missing reference invariant marker pub excerpt: Patch<String>/,
  },
  {
    name: "editor command loses loaded revision binding",
    path: `${blog}/admin/src/core/commands.rs`,
    before: "Some(post.version) == draft.version",
    after: "post.version > 0",
    diagnostic: /missing reference invariant marker Some\(post.version\) == draft.version/,
  },
  {
    name: "editor command loses requested locale binding",
    path: `${blog}/admin/src/core/commands.rs`,
    before: "post.requested_locale == draft.locale",
    after: "!draft.locale.is_empty()",
    diagnostic: /missing reference invariant marker post.requested_locale == draft.locale/,
  },
  {
    name: "post command drops predecessor CAS",
    path: `${blog}/src/services/post/commands.rs`,
    before: ".filter(blog_post::Column::Version.eq(version))",
    after: ".filter(blog_post::Column::Id.eq(post_id))",
    diagnostic: /missing reference invariant marker Column::Version.eq\(version\)/,
  },
  {
    name: "localized writes invent a fallback locale",
    path: `${blog}/src/services/post/commands.rs`,
    before: "use super::*;",
    after: "use super::*;\nuse rustok_api::PLATFORM_FALLBACK_LOCALE;",
    diagnostic: /forbidden reference-module pattern PLATFORM_FALLBACK_LOCALE/,
  },
  {
    name: "mutation bypasses current tenant authority",
    path: `${blog}/src/graphql/mutation.rs`,
    before: "mutation_tenant_id(tenant, &auth, tenant_id)?",
    after: "tenant_id.unwrap_or(tenant.id)",
    diagnostic: /mutation create_post must resolve current tenant authority/,
  },
  {
    name: "projection changes the business revision",
    path: `${blog}/src/services/comment_projection.rs`,
    before: "blog_post::Column::CommentCount,",
    after: "blog_post::Column::Version,",
    diagnostic: /forbidden reference-module pattern blog_post::Column::Version/,
  },
  {
    name: "GraphQL leaks an internal error string",
    path: `${blog}/src/graphql/query.rs`,
    before: "use super::types::*;",
    after: "use super::types::*;\nfn unsafe_error() { async_graphql::Error::new(error.to_string()); }",
    diagnostic: /forbidden reference-module pattern async_graphql::Error::new/,
  },
  {
    name: "facade exposes private persistence",
    path: `${blog}/src/lib.rs`,
    before: "mod entities;",
    after: "pub mod entities;",
    diagnostic: /forbidden reference-module pattern pub mod entities;/,
  },
  {
    name: "orchestration port exposes private persistence",
    path: `${blog}/src/ports/mod.rs`,
    before: "pub use crate::services::orchestration::*;",
    after: "pub use crate::services::orchestration::*;\npub use crate::entities::blog_post;",
    diagnostic: /forbidden reference-module pattern pub use crate::entities/,
  },
  {
    name: "module no longer composes the actual projection handler",
    path: `${blog}/src/module.rs`,
    before: "BlogCommentProjectionHandler::new",
    after: "MissingProjectionHandler::new",
    diagnostic: /register_event_listeners must inject the transactional outbox bus/,
  },
  {
    name: "service facade exports an undefined symbol",
    path: `${blog}/src/services/mod.rs`,
    before: "pub use post::{",
    after: "pub use post::MissingReferenceSymbol;\npub use post::{",
    diagnostic: /re-export 'MissingReferenceSymbol'.*is not exposed/,
  },
];

for (const regression of regressions) {
  test(`reference source guard rejects when ${regression.name}`, (t) => {
    const root = fixture(t);
    replace(root, regression.path, regression.before, regression.after, regression.all);
    const result = verify(root);
    assert.equal(result.status, 1, result.output);
    assert.match(result.output, regression.diagnostic);
  });
}
