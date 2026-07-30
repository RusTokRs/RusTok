# Blog article richtext cutover inventory

Status: `in_progress_source_verified_no_compile`.

The Blog article cutover is atomic. A surface is not considered migrated merely because it can expose `RichTextDocument` or `RichTextView`; storage, every writer, every reader, Search, SEO, and AI must agree on the same owner contract before compatibility fields are removed.

## Owner contract

- writes: `rustok_api::RichTextDocument`
- reads: `rustok_api::RichTextView`
- text consumers: server-derived plain text
- fixed owner profile: `article`

## Implemented boundaries

- `crates/rustok-blog/src/richtext.rs` owns article validation, canonical JSON, HTML projection, and plain-text projection.
- GraphQL exposes canonical writes and reads while temporary compatibility declarations remain contained in `graphql/types.rs`.
- Next admin uses a shared `RichTextDocument` editor and consumes `RichTextView`.
- Blog Leptos storefront GraphQL and native transports now carry the same `RichTextView` plus server-derived plain text; the UI renders only owner-generated HTML.
- Blog SEO description, OpenGraph, structured-data, and template-field fallback now summarize `PostResponse.content_plain_text`; legacy `PostResponse.body` is not read by the SEO projection.

## Blocking surfaces

1. **Storage schema** — `blog_post_translations` still stores `body` and `body_format`.
2. **Search projection** — Blog indexing still builds the document body from `bt.body`.
3. **AI Blog draft writer** — direct Blog draft create/update commands still write Markdown and leave canonical `content` empty.

These blockers must be migrated together with the storage transition. Removing GraphQL compatibility fields before them would split the owner contract and make save/reload/index/render behavior inconsistent.

## Executable evidence

`crates/rustok-blog/contracts/evidence/blog-richtext-cutover-inventory.json` records the exact source markers for completed and blocked surfaces. `scripts/verify/verify-blog-fba.mjs` validates the inventory as part of the existing Blog FBA chain.

The verifier is a source guard only. Compilation, runtime tests, migration execution, Search reindex verification, and browser parity remain user-owned.
