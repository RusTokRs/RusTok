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
- Blog Leptos storefront GraphQL and native transports now carry the same `RichTextView` plus server-derived plain text; the UI renders only owner-generated HTML.
- **SEO projection** now summarizes `PostResponse.content_plain_text` for descriptions, OpenGraph, structured data, and template fields; legacy `PostResponse.body` is not read.
- **Search projection** now parses canonical storage rows as `RichTextDocument` and derives their body through `rustok-content::plain_text` with the fixed `Article` profile in the same projector transaction. Legacy storage rows retain a contained raw-body fallback until the storage migration removes `body_format`.

## Blocking surfaces

1. **Storage schema** — `blog_post_translations` still stores `body` and `body_format`.
2. **AI Blog draft writer** — an owner text-import adapter is available, but direct Blog draft create/update commands still write Markdown compatibility fields and leave canonical `content` empty.

These blockers must be migrated together with the storage transition. Removing GraphQL compatibility fields before them would split the owner contract and make save/reload/index/render behavior inconsistent.

## Executable evidence

`crates/rustok-blog/contracts/evidence/blog-richtext-cutover-inventory.json` records the exact source markers for completed and blocked surfaces. `scripts/verify/verify-blog-fba.mjs` validates the inventory as part of the existing Blog FBA chain.

The verifier is a source guard only. Compilation, runtime tests, migration execution, Search reindex verification, and browser parity remain user-owned.
