# rustok-starter

## Purpose

`rustok-starter` owns the canonical declarative Starter Blueprint models and data import engine for RusToK.

## Responsibilities

- Define the typed `StarterBlueprint` schema and JSON/YAML deserialization contracts.
- Orchestrate topological, multi-module starter content provisioning across:
  - `rustok-taxonomy` (vocabularies, terms, categories)
  - `rustok-pages` (Fly / GrapesJS landing pages, atomic publication)
  - `rustok-blog` (categories, posts, tags, rich-text bodies)
  - `rustok-forum` (categories, topics, replies, accepted solution)
  - `rustok-navigation` (header/footer menus and navigation trees)
- Guarantee re-run idempotency without raw SQL bypasses or duplicate domain records.
- Ship the embedded default starter blueprint (`default-starter`).
- Expose the importer engine to CLI, server bootstrap, and admin operations.

## Entry points

- `StarterEngine::new(db, event_bus)`: Main entrypoint for importing blueprints.
- `StarterBlueprint`: Typed blueprint definition.
- `StarterExecutionReport`: Structured execution telemetry and counts.
- `embedded::default_starter()`: Built-in production starter blueprint.

## Interactions

- Driven by `rustok-installer-cli` / `rustok-cli` during seed and installation workflows.
- Invoked by `apps/server` via GraphQL mutation `importStarter` under `system:manage` authorization.
- Interacts strictly with module-owned public application services (`PageService`, `PostService`, `CategoryService`, `TopicService`, `TaxonomyService`, `NavigationService`).

## Documentation

See the canonical platform guide: [Starter Blueprints & Demo Data Guide](../../../docs/guides/starter-blueprints.md).

