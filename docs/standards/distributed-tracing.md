---
id: doc://docs/standards/distributed-tracing.md
kind: standard
language: en
status: verified
---

# Distributed Tracing

RusToK traces one logical operation across its host adapter, owner service,
database transaction, outbox publication, and asynchronous consumer. The owner
module defines the business span and resource terminology.

## Span hierarchy

```text
HTTP / GraphQL / native server function
  -> owner service operation
     -> validation and policy
     -> database transaction
     -> transactional outbox write
  -> asynchronous outbox delivery
     -> owner or projection consumer
```

Do not introduce a generic content-node layer solely to create spans. Blog,
Forum, Comments, and Pages retain their own services and identifiers.

## Context propagation

- inbound HTTP extracts the configured W3C trace context;
- internal futures remain inside the current `tracing` span;
- spawned work is explicitly instrumented with the parent span;
- event/outbox transport propagates trace context through the approved envelope
  metadata, not through an owner document;
- consumers create a linked/child processing span and preserve the logical
  `operation_id`.

Never put trace headers or span IDs into owner persistence as business data.

## Service example

```rust
#[tracing::instrument(
    name = "forum.reply.create",
    skip(self, security, input),
    fields(tenant_id = %tenant_id, topic_id = %topic_id, reply_id)
)]
async fn create_reply(
    &self,
    tenant_id: Uuid,
    security: SecurityContext,
    topic_id: Uuid,
    input: CreateReplyInput,
) -> ForumResult<ReplyView> {
    let reply = self.create_reply_transaction(tenant_id, security, topic_id, input).await?;
    tracing::Span::current().record("reply_id", tracing::field::display(reply.id));
    Ok(reply)
}
```

Payload-bearing inputs are skipped because richtext and other authored content
must not enter telemetry.

## Error recording

Record typed, bounded failure information:

- module and operation;
- error code/category;
- retryable state where the contract defines it;
- affected owner resource identity when safe;
- elapsed time and attempt number.

Do not record full documents, rendered HTML, Page Builder projects, arbitrary
error bodies, credentials, or user-authored strings.

## Asynchronous boundaries

The owner transaction and later event delivery are distinct executions. The
delivery span must expose queue delay and attempt count so operators can
distinguish write latency from projection lag. An outbox retry reuses the event
operation identity but creates a new delivery attempt span.

## Sampling

Use the platform sampling configuration. Security failures and internal errors
may be retained at a higher configured rate, but sampling policy must remain
content-free and low-cardinality. Modules must not create ad hoc exporters.

## Verification

- verify trace continuity through HTTP/GraphQL/native service and outbox paths;
- verify spawned tasks keep the intended parent;
- verify transaction rollback is visible and never followed by a success span;
- verify mutation adapters do not replay work through another protocol;
- inspect exported attributes for authored content, secrets, and unbounded
  labels;
- keep trace field names synchronized with dashboards and runbooks.

See [Logging](./logging.md) and
[Instrumentation examples](./instrumentation-examples.md).
