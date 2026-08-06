# Pages / Page Builder Authenticated Inline Edit Adapter Packet

Date: 2026-08-06  
Status: source-ready / execution-pending

## Rechecked boundary

The repository already had one canonical Fly document authority, typed commands/history, an isolated admin iframe, component instrumentation, browser cleanup utilities and a read-only Page Builder storefront renderer. It did not have a reusable real-DOM adapter or an authenticated storefront mutation contract.

This slice adds that reusable adapter boundary without mounting editing in the anonymous Pages storefront and without adding consumer persistence or transport.

## Ownership

- `fly` remains the sole project/document/command authority.
- `fly-leptos` owns browser listener lifecycle, temporary DOM attributes and plain-text event capture.
- `rustok-page-builder-storefront` owns the feature-gated authenticated inline surface and conversion into a canonical Fly patch session.
- the future Pages consumer owner must authenticate the user, authorize every request, persist the returned current project through its document-only revision transport and issue a replacement grant.

The DOM is never accepted as a document tree or hidden JavaScript authority.

## Grant and request identity

`AuthenticatedInlineEditGrant` binds:

- session id;
- stable selected page id;
- consumer document revision id;
- exact current Fly project hash;
- opaque authorization proof;
- absolute expiry.

The proof is serialized only in the commit request, is redacted from `Debug`, and is not emitted into rendered DOM attributes or HTML.

Each focusout commit request additionally binds a positive monotonic sequence, stable component id, the single supported `content` field and bounded normalized plain text.

## Real-DOM buffer

The hydrate-only adapter:

1. finds the explicitly mounted inline root;
2. marks only allow-listed instrumented nodes with `contenteditable="plaintext-only"`;
3. leaves all other rendered nodes read-only;
4. uses bubbling `focusout` as the commit boundary rather than treating every keystroke as a document mutation;
5. reads `innerText`, normalizes line endings, rejects NUL and values above 64 KiB;
6. restores every prior DOM attribute and removes its listener on cleanup or setup failure.

A successful request does not silently update the Fly project. The consumer callback must pass it through the canonical session and persistence owner.

## Canonical mutation

`AuthenticatedInlineEditSession::apply_authorized` fails closed unless:

- the grant and request identity match and are not expired;
- the sequence is newer than the last accepted request;
- the request hash equals the exact current Fly project hash;
- the component belongs to the selected page;
- the component is a stable static leaf plain-text component;
- the component is not provider-owned, a composite node with children, or template-backed;
- the component is outside every runtime-owned subtree rooted at a binding, condition or repeater target;
- the consumer authorization port accepts the request immediately before mutation.

A static leaf inside an ordinary unowned layout remains eligible. By contrast, a repeated container blocks its entire subtree, preventing duplicate rendered instances from sharing one editable component identity.

The only mutation is:

```text
EditorCommand::Patch
  → ComponentPatch::set_field("content", plain_text)
  → Fly validation/history/revision hash
```

The result returns the complete current encoded project, previous/new hash and command sequence. Because the hash changes, the current grant is intentionally one-commit: the consumer must persist the result and issue a fresh grant before another canonical edit.

## Feature and anonymous boundary

`rustok-page-builder-storefront` exposes the adapter only behind `inline-edit`. The existing default/read-only renderer still forces `instrument_components = false`. Existing Pages storefront features do not enable `inline-edit`, so this slice does not add editor code to the anonymous public graph.

## Source evidence

- `crates/fly-leptos/src/real_dom_inline.rs`;
- `crates/fly-leptos/src/root.rs`;
- `crates/fly-leptos/Cargo.toml`;
- `crates/rustok-page-builder-storefront/src/inline_edit.rs`;
- `crates/rustok-page-builder-storefront/src/lib.rs`;
- `crates/rustok-page-builder-storefront/Cargo.toml`;
- `crates/rustok-page-builder/contracts/evidence/page-builder-authenticated-inline-edit-adapter-source.json`;
- `crates/rustok-page-builder/scripts/verify/verify-page-builder-authenticated-inline-edit-adapter.mjs`.

## Deliberate limits

This slice does not:

- authenticate a Pages storefront request;
- issue or cryptographically verify a Pages-owned grant;
- add a Pages document-save server function, GraphQL mutation or HTTP route;
- mount inline editing in the anonymous storefront;
- persist on DOM input or focusout by itself;
- edit rich text, nested markup, dynamic bindings, runtime-owned subtrees or provider-owned components;
- add overlays, resize, DnD, properties or full-authoring controls to storefront;
- change publish, rollback, artifacts, caches, events or database schemas;
- claim tests, Cargo, WASM, browser, graph, workflow, CI or rollout execution;
- promote FFA or FBA.

## Next consumer slice

Pages must add an authenticated, authorized grant issuer and a document-only save endpoint that:

1. resolves the current user, tenant, page, locale and channel;
2. checks Page Builder inline-edit capability plus Pages update permission;
3. binds the exact current body revision and Fly project hash into a short-lived grant;
4. reauthorizes every commit request;
5. applies the canonical session result through the existing Pages document revision owner;
6. returns the new revision/hash and replacement grant;
7. remains absent from anonymous dependency and bundle profiles.

## Maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-page-builder/scripts/verify/verify-page-builder-authenticated-inline-edit-adapter.mjs
cargo test -p fly-leptos --all-targets -- --nocapture
cargo test -p rustok-page-builder-storefront \
  --features inline-edit,ssr --all-targets -- --nocapture
cargo check -p rustok-page-builder-storefront \
  --features inline-edit,hydrate --target wasm32-unknown-unknown
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs
```

Execution evidence remains pending.
