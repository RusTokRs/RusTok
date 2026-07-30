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
- The same owner module exposes `article_document_from_plain_text` for format-free text import. Blank lines delimit canonical paragraphs; the adapter accepts no Markdown alias, raw JSON, HTML, or caller-selected profile.
- GraphQL exposes canonical writes and reads while temporary compatibility declarations remain contained in `graphql/types.rs`.
- Next admin uses a shared `RichTextDocument` editor and consumes `RichTextView`.
- Blog Leptos storefront GraphQL and native transports carry the same `RichTextView` plus server-derived plain text; the UI renders only owner-generated HTML.
- **SEO projection** summarizes `PostResponse.content_plain_text` for descriptions, OpenGraph, structured data, and template fields; legacy `PostResponse.body` is not read.
- **Search projection** parses canonical storage rows as `RichTextDocument` and derives their body through `rustok-content::plain_text` with the fixed `Article` profile in the same projector transaction. Legacy storage rows retain a contained raw-body fallback until the storage migration removes `body_format`.
- **AI Blog draft writer** now passes Blog operations through a local owner adapter in `rustok-ai`. Generated create/update text is converted with `article_document_from_plain_text` before the Blog owner service is called, legacy write fields are cleared, and existing-post source input prefers server-derived `content_plain_text`.
- The Markdown-shaped structs still assembled inside `crates/rustok-ai/src/direct.rs` are recorded as contained compatibility only; the adapter prevents those fields from reaching the owner unchanged.

## Blocking surface

1. **Storage schema** — `blog_post_translations` still stores canonical documents through the compatibility `body` and `body_format` columns instead of a target canonical document plus server-derived text representation.

The storage transition must land atomically with removal of GraphQL and AI direct compatibility fields, Search's legacy raw-body fallback, and the quarantined storefront summarizers. Until then, implemented writers and readers use the owner contract over the compatibility storage representation.

## Executable evidence

`crates/rustok-blog/contracts/evidence/blog-richtext-cutover-inventory.json` records exact source markers for completed, contained, and blocked surfaces. `scripts/verify/verify-blog-fba.mjs` validates the inventory as part of the existing Blog FBA chain.

The verifier is a source guard only. Compilation, runtime tests, migration execution, Search reindex verification, and browser parity remain user-owned.
