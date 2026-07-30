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

## Execution boundary

Compilation, migration execution, retained PostgreSQL evidence, transport
checks, and browser parity remain maintainer-owned. Legacy rows must be
converted offline before retrying the fail-closed migration.

## Guardrail

The machine inventory and Blog FBA/GraphQL/storefront verifiers reject
reintroduction of legacy fields, raw JSON aliases, Search fallback, AI Markdown
writes, or removed summarizers.
