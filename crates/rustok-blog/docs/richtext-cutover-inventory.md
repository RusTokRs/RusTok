# Blog article richtext cutover inventory

Status: `implemented_source_verified_no_compile`.

The Blog article source cutover is target-only. Writes accept
`rustok_api::RichTextDocument`; reads return `rustok_api::RichTextView` and
server-derived plain text under the fixed `article` profile.

## Implemented source boundary

- `blog_post_translations.body` stores canonical article document JSON.
- `m20260730_000006_cutover_blog_article_richtext` validates every retained row
  before irreversibly dropping `body_format`.
- Owner, REST, GraphQL, Next admin, Leptos storefront, SEO, Search, and AI share
  the typed owner contract.
- Search parses every Blog row; no raw-body fallback remains.
- AI drafts construct canonical documents directly.
- Storefront format summarizers are physically removed.
- Forum-to-Blog orchestration fails closed unless source content is canonical
  and Article-compatible.

## Offline conversion utility

`crates/rustok-blog/src/bin/blog_article_richtext_backfill.rs` prepares retained
legacy owner rows before the irreversible migration. Dry-run is the default and
never writes rows or checkpoint state. The tool scans the actual
`blog_post_translations` / `blog_posts` owner tables, validates canonical roots,
extracts supported `rt_json_v1` envelopes, and rejects unknown formats.

Writes require `--apply` after a complete successful preflight. Historical
Markdown remains an offline input only and requires the separate
`--allow-markdown-plain-text` acknowledgement; it is preserved as literal
paragraph text rather than interpreted as a platform Markdown contract. Apply
uses optimistic body/format predicates per batch and performs a final
post-apply scan. The NDJSON report contains identifiers and outcomes, not
content bodies.

Evidence: `crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json`.
Guardrail: `scripts/verify/verify-blog-richtext-offline-backfill.mjs`.

## Execution boundary

Compilation, database execution, retained PostgreSQL evidence, transport
checks, and browser parity remain maintainer-owned. Run preflight before the
schema migration:

```bash
DATABASE_URL=postgresql://... cargo run -p rustok-blog \
  --bin blog_article_richtext_backfill -- \
  --tenant-id=<uuid> --report=artifacts/blog-richtext-preflight.ndjson
```

After reviewing the report and backup, rerun with `--apply`. Add
`--allow-markdown-plain-text` only when literal-text conversion is accepted.
The utility does not execute the schema migration or Search reindex.

## Guardrail

The machine inventory, dedicated offline-backfill verifier, and Blog
FBA/GraphQL/storefront verifiers reject reintroduction of legacy transport
fields, raw JSON aliases, Search fallback, AI Markdown writes, removed
summarizers, unsafe default writes, or checkpoint mutation during dry-run.
