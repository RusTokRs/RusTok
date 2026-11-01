# rustok-starter — Crate API

Canonical declarative Starter Blueprint models and data import engine.

## Public surface

- Blueprint models: `StarterBlueprint`, `StarterContent`, `TaxonomyStarter`, `TaxonomyCategoryStarter`, `PageStarter`, `PageBodyStarter`, `BlogStarter`, `BlogPostStarter`, `BlogPostTranslationStarter`, `ForumStarter`, `ForumCategoryStarter`, `ForumTopicStarter`, `ForumReplyStarter`, `NavigationStarter`, `NavigationMenuStarter`, `NavigationMenuItemStarter`.
- Execution: `StarterEngine`, `StarterExecutionReport`, `StarterError`, `StarterResult`, `default_starter`.
- Modules: `drivers`, `embedded`, `importer`, `model`.

## Invariants

- The engine translates a blueprint into module starter commands; it owns no module write semantics.
- `default_starter` is the only blessed embedded blueprint; custom blueprints go through the same validation path.
- Execution reports are complete: every blueprint section is accounted for as applied, skipped, or failed.
