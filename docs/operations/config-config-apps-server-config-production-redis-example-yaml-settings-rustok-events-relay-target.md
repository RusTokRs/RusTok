---
id: doc://docs/operations/config-config-apps-server-config-production-redis-example-yaml-settings-rustok-events-relay-target.md
kind: operations_documentation
language: en
source_language: en
entities:
  - config://apps/server/config/production.redis.example.yaml#settings.rustok.events.relay_target
last_verified_snapshot: snap_jsonl_00000021
status: deprecated
---

# Deprecated: `settings.rustok.events.relay_target`

## Purpose

This key was removed. Use `settings.rustok.events.delivery_profile`; the
profile owns its delivery topology and has no independent relay target.

## Contract

- Canonical entity: `config://apps/server/config/production.redis.example.yaml#settings.rustok.events.relay_target`
- Entity kind: `feature`
- Source: `apps/server/config/production.redis.example.yaml`:76

## Procedure

1. Review the source definition and confirm prerequisites.
2. Execute or operate this item according to the project runbook.
3. Record outcomes, rollback notes, and follow-up actions.

## Evidence

- `apps/server/config/production.redis.example.yaml:76`

## Notes

Generated from diagnostic `diag_env_2b504349bc52d653`. Review this page before relying on it as operational documentation.
