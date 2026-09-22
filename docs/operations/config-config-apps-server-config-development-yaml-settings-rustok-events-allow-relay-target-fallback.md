---
id: doc://docs/operations/config-config-apps-server-config-development-yaml-settings-rustok-events-allow-relay-target-fallback.md
kind: operations_documentation
language: en
source_language: en
entities:
  - config://apps/server/config/development.yaml#settings.rustok.events.allow_relay_target_fallback
last_verified_snapshot: snap_jsonl_00000021
status: deprecated
---

# Removed: `settings.rustok.events.allow_relay_target_fallback`

## Purpose

This key was removed. `outbox_iggy` fails explicitly when Iggy is unavailable;
it never falls back to local delivery. See `apps/server/docs/event-transport.md`.

## Contract

- Canonical entity: `config://apps/server/config/development.yaml#settings.rustok.events.allow_relay_target_fallback`
- Entity kind: `feature`
- Source: `apps/server/config/development.yaml`:63

## Procedure

1. Review the source definition and confirm prerequisites.
2. Execute or operate this item according to the project runbook.
3. Record outcomes, rollback notes, and follow-up actions.

## Evidence

- `apps/server/config/development.yaml:63`

## Notes

Generated from diagnostic `diag_env_f2e50de4acb87c2b`. Review this page before relying on it as operational documentation.
