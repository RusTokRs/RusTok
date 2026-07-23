---
id: doc://docs/operations/config-config-apps-server-config-test-yaml-settings-rustok-events-allow-relay-target-fallback.md
kind: operations_documentation
language: en
source_language: en
entities:
  - config://apps/server/config/test.yaml#settings.rustok.events.allow_relay_target_fallback
last_verified_snapshot: snap_jsonl_00000021
status: deprecated
---

# Removed: `settings.rustok.events.allow_relay_target_fallback`

## Purpose

This key was removed. `outbox_iggy` fails closed rather than switching to a
different delivery target.

## Contract

- Canonical entity: `config://apps/server/config/test.yaml#settings.rustok.events.allow_relay_target_fallback`
- Entity kind: `feature`
- Source: `apps/server/config/test.yaml`:62

## Procedure

1. Review the source definition and confirm prerequisites.
2. Execute or operate this item according to the project runbook.
3. Record outcomes, rollback notes, and follow-up actions.

## Evidence

- `apps/server/config/test.yaml:62`

## Notes

Generated from diagnostic `diag_env_563fee57bb14bc66`. Review this page before relying on it as operational documentation.
