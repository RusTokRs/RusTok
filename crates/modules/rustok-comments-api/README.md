# rustok-comments-api

Neutral cross-owner contract for the Comments module.

## Purpose

rustok-comments-api contains only the stable types and provider port required by consumers such as Blog. It does not contain Comments persistence, services, GraphQL implementation details, or transport clients.

## Contract

CommentsThreadPort is the only consumer-facing owner boundary for comment reads and writes. CommentRecord, CommentListItem, command inputs, filters, and moderation status requests are defined here so consumers do not link the rustok-comments implementation crate.

The Comments implementation crate remains the sole owner of storage and implements the port. Hosts compose the implementation through runtime capability wiring.

## Invariants

- Consumers never query Comments-owned tables.
- Missing provider capability is represented as an unavailable optional integration.
- Public comment reads use the explicitly safe public projection method.
- Every `CommentsThreadPort` operation is mandatory; runtime provider absence is handled by capability composition, not by trait-level operation stubs.
- PortContext remains authoritative for tenant, actor, authorization, correlation, deadline, channel, and idempotency semantics.
