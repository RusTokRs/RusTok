# rustok-reactions

## Purpose

`rustok-reactions` is the optional shared owner for reusable reactions across
revisioned subjects owned by other RusToK modules.

## Current source slice

The module currently initializes the neutral subject-provider and deferred
factory registries and exposes their bounded metadata through
`ReactionsService`. It contains no persistence, migration, event, worker,
transport or UI implementation.

## Ownership

The future owner will persist reaction catalogs, actor state, idempotent command
receipts and aggregate projections. It will not own subject existence,
visibility, lifecycle, content, profile identity, reputation, achievements or
notifications.

Each producer module publishes a provider through `rustok-reactions-api` and
remains authoritative for the current subject revision and reaction policy. The
Reactions owner never reads producer-private tables.

## Degraded mode

The module is optional and is not default-enabled. With Reactions absent,
producer modules keep their ordinary owner commands and may hide reaction UI.
Existing Forum votes remain unchanged.

## Entry points

- `ReactionsModule`
- `ReactionsService`
- re-exported `rustok_reactions_api` contracts

See [module contract](docs/README.md) and
[implementation plan](docs/implementation-plan.md).
