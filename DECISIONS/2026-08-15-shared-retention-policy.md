# Shared Retention Policy

## Status

Accepted

## Context

Translation Memory and Alloy evidence both need the same closed lifecycle
policy vocabulary: owner lifecycle, explicit retention deadline, and legal
hold. Keeping separate domain enums would let persistence values and automatic
collection semantics drift between evidence owners.

## Decision

`rustok-core::RetentionPolicy` is the canonical policy vocabulary. It owns the
persisted names, validates the `retain_until` invariant against an injected
time, and declares that legal hold is never an automatic collection candidate.

Each domain still owns its records, authorization, retention receipts,
redaction, and collection worker. A shared policy does not authorize a legal
hold, select a retention duration, or delete domain data.

## Consequences

- Domain schemas store only the canonical policy names.
- Transport-specific enums map to the shared contract at their boundary.
- Any evidence reaper must reject legal-hold records before collection.
- Future retention owners reuse this contract instead of defining another
  policy enum.
