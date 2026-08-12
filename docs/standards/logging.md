---
id: doc://docs/standards/logging.md
kind: standard
language: en
status: verified
---

# Logging Standard

RusToK uses structured `tracing` events. Logs describe an operation and its
bounded identifiers; they never contain richtext documents, comments, page
projects, credentials, tokens, cookies, or other user-authored payloads.

## Required fields

Use stable low-cardinality field names where they exist:

- `tenant_id`, `actor_id`, `module_slug`;
- owner resource identity such as `post_id`, `topic_id`, `comment_id`, or
  `page_id`;
- `operation_id`, `idempotency_key_hash`, `expected_revision`;
- a bounded `error_code` or lifecycle state.

Do not log a generic `node_id` for owner-domain records. Blog, Forum, Comments,
and Pages use their owner terminology.

## Service instrumentation

Owner services create the business span. Adapters may create transport spans,
but must not duplicate document validation or log payloads.

```rust
#[tracing::instrument(
    skip(self, security, input),
    fields(tenant_id = %tenant_id, actor_id = ?security.user_id, post_id)
)]
pub async fn create_post(
    &self,
    tenant_id: Uuid,
    security: SecurityContext,
    input: CreatePostInput,
) -> BlogResult<PostResponse> {
    let created = self.create_post_transaction(tenant_id, security, input).await?;
    tracing::Span::current().record("post_id", tracing::field::display(created.id));
    tracing::info!(status = ?created.status, "Blog post created");
    Ok(created)
}
```

The snippet demonstrates the logging shape; use the exact DTO and method names
owned by the module being changed.

## Levels

- `error`: an operation failed and requires investigation or retry handling;
- `warn`: a guarded degraded path, rejected external input, or recoverable
  invariant violation;
- `info`: lifecycle transitions and successful operator-visible operations;
- `debug`: bounded diagnostic state useful during development;
- `trace`: hot-path detail that is normally disabled.

Expected user validation failures should normally be `debug` or `warn`, not
`error`. Internal failures are `error` and include a typed error code where
available.

## Privacy and cardinality

Never log:

- `RichTextDocument`, rendered HTML, extracted plain text, Page Builder JSON;
- comment/review text, titles, descriptions, email bodies, search queries;
- authorization headers, JWTs, refresh tokens, cookies, secrets;
- arbitrary URLs with query strings;
- unbounded exception text as a metric label.

Identifiers belong in span fields. Content size may be recorded as a number,
and validation failure kind may be recorded through a bounded enum/code.

## Transactions and events

Record one outcome after the owner transaction commits. Do not log success
before the transaction or outbox write completes. Outbox delivery logs use
`operation_id`, event type, attempt count, and bounded failure code; they do not
repeat event payloads.

## Verification

- exercise success, validation failure, permission denial, and storage failure;
- verify secret/content fields are absent from captured logs;
- ensure retry loops do not emit an error storm for one logical operation;
- keep dashboards and alert queries synchronized when field names change.

See also [Distributed tracing](./distributed-tracing.md) and
[Instrumentation examples](./instrumentation-examples.md).
