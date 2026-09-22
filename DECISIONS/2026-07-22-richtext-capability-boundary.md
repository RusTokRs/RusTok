# Richtext capability boundary and single-document contract

- Date: 2026-07-22
- Status: Accepted

## Context

RusToK currently has a Tiptap prototype inside the Next Blog package, a
versioned `rt_json_v1` envelope implemented in `rustok-core`, owner-local body
tables in Blog, Forum, and Comments, and no complete shared read path. The same
source is transported as both a string and `content_json`; locale is duplicated
inside the document; Forum UI is partly owned by the Blog package; direct
Comments writes can bypass document validation.

The repository already assigns shared rich-text and locale behavior to
`rustok-content`, while neutral cross-boundary types belong to `rustok-api`.
Creating another backend module would split that ownership. Writing a second
editor in Rust would duplicate Tiptap behavior and still require a browser
editing engine.

Next and Leptos also cannot mount an ordinary ProseMirror editor under the same
parent-document assumptions. The current parent CSP forbids inline style
attributes, while ProseMirror writes them during normal editing.

## Decision

### Contract and naming

The canonical concept name is `richtext`. The persisted source of truth is one
current `RichTextDocument` in the ProseMirror/Tiptap root JSON shape. It has no
outer version or locale envelope.

RusToK's server-side profile registry is authoritative. Reusing the
ProseMirror shape avoids a permanent handwritten tree codec; it does not make
the installed Tiptap extension defaults authoritative.

Built-in node and mark discriminator values retain the pinned external
ProseMirror/Tiptap spelling. RusToK-defined fields and custom attributes use the
platform `snake_case` convention.

Locale remains in the owner request and owner translation/body row. A migrated
write field is typed as `RichTextDocument`; clients do not submit a second
string body, `content_json`, or a selectable content format. Where a truly
generic discriminator remains necessary, this capability's variant is named
`richtext` and has no alias. Other capability-owned document formats remain
outside the richtext contract.

Only one current schema is supported. A schema change requires an atomic update
of repository-owned writers/readers and an owner-data migration, not a second
versioned runtime path.

### Backend ownership

- `rustok-api::richtext` owns transport-neutral document, profile-identifier,
  and read-projection types plus optional transport adapters behind existing
  feature boundaries.
- `rustok-content::richtext` owns profiles, strict structural validation,
  normalization, safe semantic HTML rendering, plain-text extraction, limits,
  and shared behavioral fixtures.
- Blog, Forum, Comments, and future consumers own their fields, tables, locale
  rows/effective selection, RBAC, lifecycle, revisions, events, profile
  selection, and transport while reusing the canonical shared locale contract.
- `rustok-core::rt_json` and the generic legacy content-format helper are
  removed after repository-owned callers move.
- No new tenant module, module slug, shared richtext table, or standalone
  richtext backend crate is introduced.

The production read renderer is implemented once in Rust. It returns a derived
safe HTML projection to both Next and Leptos and a shared plain-text projection
to Search/Index and other text consumers. Browser Tiptap rendering is editor
preview only and is not a second production renderer.

### Browser ownership and CSP

`@rustok/richtext` owns one vanilla Tiptap runtime, one extension/profile
registry, one toolbar behavior, static editor assets, and thin framework
adapters. It owns no persistence, locale selection, transport, upload, mention,
or domain policy.

Both Next and Leptos mount that runtime in the same sandboxed editor frame. The
parent CSP is not weakened. Each host exposes the same package-produced frame
at a same-origin route. The frame has a dedicated restrictive CSP, permits
style attributes only inside the editor document, has no direct data network,
storage, forms, popups, or navigation, and communicates through a bounded
private `MessageChannel` after a source/nonce handshake. Its response sets
same-origin framing, nosniff, no-referrer, and restrictive permissions headers;
the static endpoint ignores and never sets auth/session cookies or embeds
tenant/user data. The opaque-origin frame and asset-loading model must pass the
supported-browser spike before implementation proceeds. Failure requires a
separate CSP/origin decision rather than an implicit external-origin fallback.

If a Rust-side Leptos lifecycle adapter warrants extraction, it is a support
crate rather than a backend module and follows the repository support-crate
documentation/registry rules.

### Scope

The first owner profiles are Blog article, Forum discussion, and Comments
comment. Images and embeds are excluded initially. A later image node stores a
Media identity and resolves it through an owner/integration adapter; it does not
create a direct `rustok-content -> rustok-media` dependency or store arbitrary
external HTML/URLs.

Pages body remains Page Builder/Fly. Product descriptions, Product richtext
attributes, and future Reviews require an explicit owner storage/API/index
decision before adopting the editor. Short descriptions, excerpts, SEO fields,
labels, and slugs remain plain text unless their owner explicitly opts in.

Markdown is not a storage or authoring contract. A future UI may use
Tiptap-provided import/export without changing the backend. Material existing
Markdown records, if any, are handled only by upstream tooling in the offline
cutover.

### Cutover

The internal cutover is atomic: migrate owner rows/revisions, update all
repository-owned transports and UIs, rebuild projections, then delete legacy
constants, aliases, DTO fields, format selectors, validators, editor mappings,
and migration code. There is no internal dual-read/dual-write or fallback path.
A staged external bridge requires a separate approved decision, removal owner,
and calendar deadline.

## Consequences

- Multilingual storage remains domain-owned and the editor stays locale-free.
- Next and Leptos share one browser implementation without shipping Tiptap to
  anonymous storefront readers.
- Production rendering, search text, and security policy cannot drift by host.
- Adding an editor extension becomes a full contract change: server profile,
  renderer, plain-text behavior, CSP/accessibility tests, fixtures, and owner
  rollout must land together.
- Existing `rt_json_v1`, Markdown modes, raw JSON previews, `content_json`,
  format aliases, and raw body-copy orchestration become cutover work rather
  than supported compatibility behavior.
- The frame adds an explicit browser boundary and messaging protocol, but keeps
  ProseMirror's inline style behavior outside the stricter parent document.

## Rejected alternatives

- **Standalone `rustok-richtext` backend crate:** rejected because accepted
  ownership already places shared behavior in `rustok-content` and neutral
  types in `rustok-api`.
- **All layers inside `rustok-content`:** rejected because browser framework
  adapters and neutral public types have separate dependency boundaries.
- **Full Rust editor:** rejected because it would duplicate a mature browser
  editing engine and still need DOM/selection/clipboard behavior.
- **Direct Tiptap mount in the parent document:** rejected under the current
  CSP because core ProseMirror behavior writes inline styles.
- **Separate JavaScript and Rust production renderers:** rejected as duplicated
  security/business semantics.
- **Vendor-neutral snake-case AST plus a Tiptap codec:** rejected because the
  current prototype demonstrates the drift and data-loss risk of a permanent
  hand-maintained mapping.
- **Persistent Markdown plus richtext modes:** rejected because migrated fields
  have one typed source of truth.

## Implementation reference

The execution sequence, audited legacy inventory, profiles, migration rules,
and verification matrix live in
[the Richtext implementation plan](../docs/modules/rich-text-implementation-plan.md).
