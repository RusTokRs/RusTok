# Pages / Page Builder Authenticated Inline Edit Adapter Packet

Date: 2026-08-06  
Status: source-ready / execution-pending  
Historical scope: reusable adapter only

## Rechecked boundary

This packet records the reusable adapter introduced by PR #3039. It remains the source authority for the Fly/Page Builder real-DOM interaction boundary. It does not retroactively claim that the adapter slice implemented Pages authentication or persistence.

The earlier consumer-open boundary is superseded by the later Pages authenticated inline consumer packet:

`docs/modules/pages-page-builder-authenticated-inline-consumer-packet-2026-08-06.md`

## Ownership

- `fly` remains the sole project/document/command authority.
- `fly-leptos` owns browser listener lifecycle, temporary DOM attributes and plain-text event capture.
- `rustok-page-builder-storefront` owns the feature-gated inline surface and conversion into a canonical Fly patch session.
- Pages authentication and document persistence belong to the later consumer slice.

The DOM is never accepted as a document tree or hidden JavaScript authority.

## Grant and real-DOM buffer

The reusable grant binds session, stable selected page, consumer document revision, exact current Fly project hash, opaque proof and expiry. The proof is redacted from `Debug` and is not emitted into rendered DOM attributes or HTML.

The hydrate-only adapter:

1. finds the explicitly mounted root;
2. marks only allow-listed instrumented nodes with `contenteditable="plaintext-only"`;
3. leaves all other nodes read-only;
4. uses bubbling `focusout` as the commit boundary;
5. reads plain text, normalizes line endings, rejects NUL and values above 64 KiB;
6. restores every prior attribute and removes its listener on cleanup or setup failure.

## Canonical mutation

`AuthenticatedInlineEditSession::apply_authorized` validates grant identity, expiry, sequence, selected page, exact project hash and component eligibility. Runtime-owned subtrees, provider/composite/templated nodes and interactive controls remain read-only. Unchanged focusout values return `NoContentChange` and do not consume the grant.

The only changed-document mutation is:

```text
EditorCommand::Patch
  → ComponentPatch::set_field("content", plain_text)
  → Fly validation/history/revision hash
```

The result returns the complete current project, previous/new hash and command sequence. A changed canonical hash requires a new grant.

## Historical consumer boundary

The adapter evidence correctly retains these booleans as false for this slice:

- Pages consumer grant issuance added;
- Pages consumer save transport added;
- anonymous storefront inline mount added.

They were outside PR #3039. The first two are superseded by the later Pages authenticated inline consumer packet. Anonymous mounting remains absent.

## Deliberate limits

This adapter slice does not:

- authenticate a Pages request;
- issue or cryptographically verify a Pages-owned grant;
- persist a Pages body;
- mount inline editing in the anonymous storefront;
- edit rich text or runtime-owned content;
- change publish, rollback, artifacts, caches, events or database schemas;
- claim FFA or FBA.

## Maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-page-builder/scripts/verify/verify-page-builder-authenticated-inline-edit-adapter.mjs
cargo test -p fly-leptos --all-targets -- --nocapture
cargo test -p rustok-page-builder-storefront \
  --features inline-edit,ssr --all-targets -- --nocapture
cargo check -p rustok-page-builder-storefront \
  --features inline-edit,hydrate --target wasm32-unknown-unknown
```

Execution evidence remains pending.
