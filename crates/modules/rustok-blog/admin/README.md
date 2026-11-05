# rustok-blog-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-blog` module.

## Responsibilities

- Exposes the blog admin root view used by `apps/admin`.
- Stays module-owned: blog-specific admin UI does not live in `apps/admin`.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Exposes Blog-owned Category selection, per-post channel visibility, featured image, localized SEO defaults, and RFC 3339 schedule authoring for drafts through both selected transports.
- Owns blog CRUD through a module transport facade: SSR/hydrate selects native `#[server]` functions, while standalone CSR selects the parallel GraphQL contract.
- Owns a separate comment-moderation slice through `admin/src/moderation.rs`; selecting a post loads its non-deleted owner queue and supports approve/spam/trash actions through the selected native or GraphQL transport.
- Paginates the moderation queue with bounded `page`/`per_page` inputs, resets page state when the selected post changes, and prevents navigation outside the server-reported total.
- Keeps moderation separate from the post detail query so editors without `blog_posts:manage` retain normal CRUD behavior and reduced GraphQL builds can degrade only the moderation panel.
- Embeds owner-side post SEO editing through `rustok-seo-panel` instead of relying on a central SEO entity editor.
- Keeps Leptos render/bind code under `admin/src/ui/`: the editor/moderation page composition resides in `ui/mod.rs`, while the editor controller remains in `ui/leptos.rs`. The crate `lib.rs` is now a pure facade. Native server functions, GraphQL adapters and moderation GraphQL queries stay under `admin/src/transport/`.
- Mounts the shared sandboxed `@rustok/richtext` frame through the thin WASM lifecycle bridge only during browser hydration. SSR emits the iframe markup and never executes browser/WASM code.

## Entry Points

- `BlogAdmin` — composed root admin page containing the existing post editor and the selected-post moderation panel.
- `rustok-module.toml [provides.admin_ui]` advertises `leptos_crate`, `route_segment`, and `nav_label` for host composition.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/blog` through the generic module page route.
- Uses `transport/native_server_adapter.rs` for the Leptos SSR/hydrate internal path and keeps `transport/graphql_adapter.rs` plus `transport/moderation_adapter.rs` as the parallel public/headless path. A failed mutation is never retried through another protocol.
- Treats a missing `posts` GraphQL contract in reduced server builds as an unavailable list surface and renders the normal empty state instead of surfacing a dashboard-level error.
- Treats a missing `moderationComments`, `moderateComment`, or `BlogCommentModerationStatus` contract as a moderation-only unavailable state.
- The backend requires `blog_posts:manage`, current-tenant binding, and the Blog field-aware rate-limit policy before trusted owner-side comment reads or status changes.
- Uses the shared `rustok-seo` GraphQL contract through `rustok-seo-panel` for explicit post SEO authoring.
- Native and GraphQL adapters call the same Blog owner services and must keep their DTO, authorization, tenant, locale, and error semantics aligned.
- The host mounts Blog Fly preview and property ports through this package's selected transport facade. Property requests compare the complete schema reference with the registered contribution; configuration and owner-data preview execute in `BlogWidgetService`.

## Known Limitations / Pending Implementation

The Fly contribution exposes owner data as the editor's bounded JSON preview.
Mounted property editing, preview and draft save/reload still require browser
evidence on the deployed host; manifest and pure adapter tests do not establish
that result. The Blog reference readiness plan tracks the remaining client and
runtime gates. The shared Page Builder currently displays raw schema field
labels and bounded JSON data, so localized property labels and visual widget
canvas rendering require separate Page Builder-owned implementation/evidence.

The shared Page Builder FBA registry/validators still model independent
interoperability versions. Blog removes unused independent version metadata and
is not enrolled in that registry. The policy mismatch with
Fly's module-only evolution and the missing registry admission need an owner-wide
contract/registry/validator reconciliation before full reference acceptance.
Blog's owner widget payloads and schema references are already unversioned;
that bounded correction does not resolve the shared policy mismatch.

## Documentation

- See [CRATE_API.md](CRATE_API.md) for full contract specification.
- See [platform docs](../../../../docs/index.md).

## Editorial write contract

Post creation uses a complete `BlogPostDraft`; post editing uses a distinct
`BlogPostUpdate` with mandatory predecessor revision and canonical nullable
`Patch` semantics. Pure command builders compare target-language editor values
against the loaded owner snapshot. Unchanged fields are omitted from both
native and GraphQL commands. Owner fields can change without touching copy.

A fallback projection is shown as a labelled read-only source. It never hydrates
editable title, body, excerpt, SEO or Category identity in a missing target
language. A new Post translation requires its own title and body; a new Category
translation requires its own name. Changing another field cannot create a
translation from fallback text. Existing optional copy remains distinguishable
as keep, set and clear.

Create/update and lifecycle adapters acknowledge the owner command without a separate detail
read. The controller invalidates the old edit revision immediately after that
acknowledgement, then reloads independently. One typed `PostEditorSession` owns
the editor identity, revision and snapshot; a failed reload retains the saved
identity, exposes its localized notice, and cannot become a duplicate create.
A failed reload displays a saved
notice and blocks another save until a fresh snapshot is loaded. UUID request
identity binds asynchronous loads/writes to their selected entity and locale;
principal changes or later selections discard predecessor completions. Lifecycle
bindings live in `ui/leptos/lifecycle.rs`. Existing Post and exact-target Category routes cannot be
cleared on update; empty create routes still delegate generation to Blog. It never
reports a committed command as a failed save or grants implicit Read permission.

Editor controls use `rustok-forms-leptos` and the shared form lifecycle/dirty
state. Category rows use the canonical `DataGrid` with the complete filtered
collection; the grid owns client pagination. Category-list failures remain
visible and do not clear an existing Post assignment.

## Known Limitations / Pending Implementation

The latest editor continuation adds source regressions for actual native HTTP
write-only acknowledgements (update and all six lifecycle commands), using the
shared Fly request fixture. These latest Rust changes have not yet been compiled
or executed in this branch; combined integration verification owns that gate.
Thirteen Next pure/API tests passed locally; mounted workflow coverage remains open.

Mounted browser, accessibility and authenticated persistence/reload scenarios
remain required in both clients. Default unit tests and SSR compilation do not
certify those workflows. The owner reference plan tracks that acceptance work.
