# rustok-blog / CRATE_API

## Role

`rustok-blog` is the canonical native-module reference owner for Blog posts,
Blog Category membership/settings, Blog-owned post/tag/channel relations and the
Blog-side Comments integration.

Persistence entities are **not** public API. Consumers use owner DTOs, services,
transport roots, or explicit ports.

## Public surface

Deliberate public modules/facades:

- `controllers` and `openapi` — owner HTTP/OpenAPI surface;
- `dto` — owner command/query/response contracts;
- `error` — `BlogError`, `BlogPublicError`, `BlogResult`;
- `graphql` — `BlogQuery`, `BlogMutation`, schema data attachment;
- `migrations` — module migration source;
- `services` — owner service types;
- compatibility facades `richtext`, `state_machine`, and
  `public_comments_snapshot`.

`entities` is crate-private. Direct cross-crate SeaORM access is not supported.

Primary entry points:

- `BlogModule`
- `PostService`
- `CommentService`
- `CategoryService`
- `TagService`
- `CommentsThreadPort`
- `BlogQuery` / `BlogMutation`
- `controllers::axum_router`

## Post owner contract

### Create

`CreatePostInput` requires an explicit content locale and canonical
`RichTextDocument`. The supplied locale is normalized before storage.

The Blog slug is canonical/global for the post, not per locale.

Arbitrary `metadata` is extensibility data only. The owner rejects metadata keys
that shadow canonical typed state, including tags, category, featured image, SEO,
and channel visibility.

### Update

`UpdatePostInput` has these semantics:

```rust
pub struct UpdatePostInput {
    pub locale: Option<String>,
    pub title: Option<String>,
    pub content: Option<RichTextDocument>,
    pub excerpt: Patch<String>,
    pub slug: Option<String>,
    pub tags: Option<Vec<String>>,
    pub category_id: Patch<Uuid>,
    pub featured_image_url: Patch<String>,
    pub seo_title: Patch<String>,
    pub seo_description: Patch<String>,
    pub channel_slugs: Option<Vec<String>>,
    pub metadata: Option<Value>,
    pub version: i32,
}
```

`Patch<T>` means:

- `Keep`: field omitted, preserve current value;
- `Set(value)`: store the supplied value;
- `Clear`: explicitly store NULL/remove the optional value.

The predecessor `version` is mandatory. Update is compare-and-swap bound to
that version and conflicts fail closed.

A localized mutation requires an explicit locale. Runtime fallback locales are
read policy and are never used as write provenance. Creating a previously absent
locale requires that locale's own required `title` and `content`; Blog never
copies text from another locale and relabels it.

### Lifecycle

The allowed transition graph is:

```text
Draft     -> Published
Published -> Draft
Published -> Archived
Archived  -> Draft     (restore)
```

Production commands consult the same domain transition table:

- `publish_post`
- `unpublish_post`
- `archive_post`
- `restore_post`

Invalid transitions fail; transports/UI do not reinterpret them.

### Reads and lists

`PostResponse.version` is the edit predecessor revision.

Locale resolution on reads follows the shared runtime fallback contract, but an
existing post with no localized records is a storage invariant violation rather
than a fabricated empty post. Persisted Article richtext JSON is also an owner
storage invariant: malformed JSON or a document that no longer satisfies the
fixed Article profile fails closed as an internal Blog invariant, never as client
input validation.

`PostListQuery` is an owner list/filter API, not full-text search. Sort fields
and order are typed through `PostSortField` / `PostSortOrder`, and owner
queries use a stable post-id tie-breaker.

Full-text search remains the Search capability's responsibility.

## Derived state and events

Blog lifecycle/content mutations publish typed Blog lifecycle/update or neutral
reindex events through the transactional outbox.

Comments lifecycle events project both the derived `comment_count` and the
processed public-comment lifecycle cursor into Blog. The projection:

- is idempotent through the Blog delivery ledger;
- is tenant-scoped;
- uses the observed `comment_count` as its retry predecessor;
- does **not** mutate Blog business `version` or business `updated_at`;
- publishes neutral `ReindexRequested { target_type: "blog" }` only when
  `comment_count` changes;
- records update/status-change lifecycle events without mutating
  `comment_count`;
- exposes the latest processed lifecycle event id as the public snapshot
  invalidation cursor. Snapshot keys include that cursor, so each processed
  lifecycle change makes earlier cached snapshots unreachable without cache-key
  enumeration.

## Tenant authority

`TenantContext` is authoritative in GraphQL/HTTP/native host adapters. Optional
GraphQL tenant arguments may only match the current tenant; they cannot switch
tenants. Authenticated actor tenant mismatch fails closed on both queries and
mutations.

Privileged Blog post reads use the same owner permission resource as Blog writes:
`Resource::BlogPosts`. The generic platform `Resource::Posts` permission is not a
fallback authority for Blog drafts or archived posts.

## Public error contract

Internal services return `BlogError`. Public HTTP/GraphQL/native boundaries map
through the owner-controlled `BlogPublicError` descriptor.

Database/connector/internal details are redacted. Typed not-found, conflict,
forbidden, validation and business errors retain their public status/code
semantics.

Persisted enum decoding, missing/duplicate canonical Taxonomy Category projections,
unsupported storage backends and broken host composition are internal failures, not
client validation. Taxonomy dependency errors preserve not-found/conflict/internal
classification when crossing back into Blog. Native server functions never render
raw owner/dependency errors and resolve host runtime context fallibly.

## Integration boundary

`src/integrations/` adapts Blog owner state to external capabilities (SEO,
Reactions, public-comment snapshots). Integration adapters must use owner
services/helpers/ports and must not become a second repository or business-policy
implementation.

SEO distinguishes true owner not-found from infrastructure failure. Reaction
authorization consumes the Blog owner subject snapshot rather than querying
Blog entities directly.

## Comments / FBA

Blog consumes Comments through `CommentsThreadPort` and typed `PortContext` /
`PortError`.

`CreateCommentInput.command_id` is the stable logical command identity and must
be reused by callers across retries so provider idempotency remains stable.

Comment reads and mutations that start from a Comments record revalidate the canonical Blog post in the same Blog service boundary. A stale Comments thread left briefly by asynchronous target-deletion processing is therefore not treated as a valid Blog surface.
Comment creation also revalidates the canonical post after the external Comments write; if terminal deletion won the race, Blog compensates the created comment with a fresh idempotent delete command and returns post-not-found. The terminal `TargetDeleted` event remains the durable cleanup backstop.

Current Blog FBA status is `boundary_ready`, not `transport_verified`.
Remote transport and runtime fallback/live evidence remain separate promotion
requirements.

## Module-owned UI contract

`admin/` and `storefront/` are owner-local adapter crates. They do not own
Blog domain policy.

The admin edit model carries `PostResponse.version`; edit transports require it
before sending an update. Lifecycle UI exposes explicit restore for archived
posts.

The manifest's bundled UI locales (currently `en` and `ru`) describe shipped
interface translations only. They do not restrict tenant Blog content locales.

The storefront package also exposes an authenticated Blog-bound public comment
composer. Its native and GraphQL write adapters preserve the current tenant,
require `comments:create`, require an enabled Blog channel, and delegate the
write through `CommentService::create_public_comment` and the Comments owner
port. The `hide_comment_form` degraded mode remains planned; it is not runtime-
verified evidence.

## Verification

Canonical reference gates:

```bash
npm run verify:module-source-layout
npm run verify:module-reference-contract
npm run verify:blog:fba
cargo xtask module validate blog
cargo check -p rustok-blog
cargo test -p rustok-blog --lib
```

The FBA registry is `crates/modules/rustok-blog/contracts/blog-fba-registry.json`.
The Comments projection contract is
`crates/modules/rustok-blog/contracts/evidence/blog-comments-event-projection.json`.

## Minimum Contract Set

### Input DTOs/Commands
- Input contract is defined by the public DTOs/commands from the crate.
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
