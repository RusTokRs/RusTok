---
id: doc://docs/operations/env-rustok-event-transport.md
kind: operations_documentation
language: en
source_language: en
entities:
  - env://RUSTOK_EVENT_TRANSPORT
last_verified_snapshot: snap_jsonl_00000021
status: deprecated
---

# Deprecated: `RUSTOK_EVENT_TRANSPORT`

## Purpose

This variable was removed. Use `RUSTOK_EVENT_DELIVERY_PROFILE` with exactly
`memory`, `outbox_local`, or `outbox_iggy`. The canonical contract is in
[`apps/server` event delivery documentation](../../apps/server/docs/event-transport.md).

## Contract

- Removed variable: `RUSTOK_EVENT_TRANSPORT`
- Replacement: `RUSTOK_EVENT_DELIVERY_PROFILE`

## Evidence

- `apps/server/src/common/settings.rs`

## Notes

Generated from diagnostic `diag_env_9015d24d1db7192b`. Review this page before relying on it as operational documentation.
