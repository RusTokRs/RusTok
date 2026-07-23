---
id: doc://docs/operations/config-config-apps-server-config-production-redis-example-yaml-settings-rustok-events-allow-relay-target-fallback.md
kind: operations_documentation
language: en
source_language: en
entities:
  - config://apps/server/config/production.redis.example.yaml#settings.rustok.events.allow_relay_target_fallback
last_verified_snapshot: snap_jsonl_00000021
status: deprecated
---

# Removed: `settings.rustok.events.allow_relay_target_fallback`

## Purpose

This key was removed. The runtime does not fall back when `outbox_iggy` cannot
use Iggy.

## Contract

- Canonical entity: `config://apps/server/config/production.redis.example.yaml#settings.rustok.events.allow_relay_target_fallback`
- Entity kind: `feature`
- Source: `apps/server/config/production.redis.example.yaml`:77

## Procedure

1. Review the source definition and confirm prerequisites.
2. Execute or operate this item according to the project runbook.
3. Record outcomes, rollback notes, and follow-up actions.

## Evidence

- `apps/server/config/production.redis.example.yaml:77`

## Notes

Generated from diagnostic `diag_env_67520c61ec79eeac`. Review this page before relying on it as operational documentation.
