---
id: doc://docs/operations/config-config-apps-server-config-development-yaml-settings-rustok-events-relay-target.md
kind: operations_documentation
language: en
source_language: en
entities:
  - config://apps/server/config/development.yaml#settings.rustok.events.relay_target
last_verified_snapshot: snap_jsonl_00000021
status: deprecated
---

# Deprecated: `settings.rustok.events.relay_target`

## Purpose

This key was removed. Use the single `settings.rustok.events.delivery_profile`
selector. It has no independently switchable relay target and no fallback.

## Contract

- Replacement: `settings.rustok.events.delivery_profile`
- Canonical guide: `apps/server/docs/event-transport.md`

## Procedure

1. Review the source definition and confirm prerequisites.
2. Execute or operate this item according to the project runbook.
3. Record outcomes, rollback notes, and follow-up actions.

## Evidence

- `apps/server/config/development.yaml:62`

## Notes

Generated from diagnostic `diag_env_ec73a0270fa67586`. Review this page before relying on it as operational documentation.
