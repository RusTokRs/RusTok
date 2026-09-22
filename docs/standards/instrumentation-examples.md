---
id: doc://docs/standards/instrumentation-examples.md
kind: standard
language: en
status: verified
---

# Instrumentation Examples

These examples show the current ownership pattern. Substitute the exact
owner-local DTOs and resource names; do not create a generic shared-node service
to copy the snippets.

## Owner service span

```rust
#[tracing::instrument(
    name = "comments.create",
    skip(self, security, input),
    fields(tenant_id = %tenant_id, actor_id = ?security.user_id, comment_id)
)]
pub async fn create(
    &self,
    tenant_id: Uuid,
    security: SecurityContext,
    input: CreateCommentInput,
) -> CommentsResult<CommentResponse> {
    let result = self.create_transaction(tenant_id, security, input).await?;
    tracing::Span::current().record("comment_id", tracing::field::display(result.id));
    Ok(result)
}
```

Richtext validation runs inside the owner service with its fixed profile. The
span skips the document-bearing input.

## Database operation

```rust
#[tracing::instrument(
    name = "pages.body.persist",
    skip(transaction, document),
    fields(tenant_id = %tenant_id, page_id = %page_id, locale = %locale)
)]
async fn persist_page_document(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    document: &serde_json::Value,
) -> PagesResult<()> {
    // Owner persistence only. Never record `document`.
    Ok(())
}
```

## GraphQL or native adapter

Transport spans record transport identity and bounded operation names, then
delegate to the same owner service.

```rust
#[tracing::instrument(
    name = "graphql.blog.update_post",
    skip(context, input),
    fields(post_id = %id)
)]
async fn update_post(
    context: &async_graphql::Context<'_>,
    id: Uuid,
    input: UpdatePostInput,
) -> async_graphql::Result<PostView> {
    let runtime = context.data::<BlogRuntime>()?;
    runtime.update_post(id, input).await.map_err(Into::into)
}
```

Do not retry a mutation through a second protocol after an ambiguous transport
failure.

## Outbox consumer

```rust
#[tracing::instrument(
    name = "outbox.consume",
    skip(self, envelope),
    fields(event_type = envelope.event_type(), operation_id = %envelope.id())
)]
async fn consume(&self, envelope: EventEnvelope) -> Result<(), ConsumerError> {
    self.dispatch(envelope).await
}
```

The event payload is skipped. Retry attempt and bounded error code may be
recorded; content fields may not.

## Metrics

Metrics use bounded labels such as module, operation, outcome, error code, and
transport. IDs, locale values, URLs, titles, document text, and exception
messages are not metric labels.

Useful measurements include:

- request/operation latency;
- validation failures by profile and bounded error code;
- document byte/node counts as histograms;
- outbox queue depth, delivery delay, and retry count;
- editor frame load/handshake failures without document content.

## Verification checklist

- span fields use owner terminology;
- payload-bearing inputs are skipped;
- one logical operation has one owner span across adapters;
- transaction success is recorded only after commit;
- errors preserve trace context and typed error codes;
- logs and metrics contain no authored content or secrets.
