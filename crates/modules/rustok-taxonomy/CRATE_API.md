# Taxonomy Crate API Specification

## Public Modules
- `dto`: Input and output data transfer objects (`CreateTaxonomyTermInput`, `TaxonomyTermResponse`).
- `entities`: SeaORM entity definitions for taxonomy terms, categories, and translations.
- `error`: Strongly typed domain error variants (`TaxonomyError`, `TaxonomyResult`).
- `migrations`: SeaORM migration chain for taxonomy schemas and route keys.
- `module`: Lifecycle descriptor `TaxonomyModule` implementing `RusToKModule`.
- `module_term_mutation`: Transactional mutations for module-scoped taxonomy terms.
- `seo_targets`: SEO target provider implementations for taxonomy categories.
- `services`: Domain service layer (`TaxonomyService`) for taxonomy management.
- `translation_target`: Registration and provider for multi-locale term translation.

## Platform Capability Bindings
- **RusToKModule Implementation**: `TaxonomyModule` in `src/module.rs`
- **Module Slug**: `taxonomy`
- **Module Name**: `Taxonomy`
- **Module Kind**: `ModuleKind::Optional`
- **Dependencies**: `["content", "outbox"]`
- **Permissions**:
  - `TAXONOMY_CREATE`
  - `TAXONOMY_READ`
  - `TAXONOMY_UPDATE`
  - `TAXONOMY_DELETE`
  - `TAXONOMY_LIST`
  - `TAXONOMY_MANAGE`
- **Runtime Extensions**:
  - `TaxonomyCategorySeoTargetProvider` registered via `rustok_seo_targets::register_seo_target_provider`.
- **Database Migrations**: Sea-ORM migrations defined in `src/migrations/mod.rs` (including route key registry and owner hierarchy migrations).

## Domain Collaborations
- **Content**: Subscribes to content classification events and scopes taxonomy terms to content types.
- **Outbox**: Emits taxonomy term lifecycle and translation sync events through `rustok_outbox::TransactionalEventBus`.
- **SEO & Translation Targets**: Implements targets for hierarchical category SEO and multi-locale term translations.
- **Downstream Consumers**: Consumed by `product`, `forum`, `blog`, `profiles`, and `comments` for categories, tags, and terms.

## Minimum Contract Set
- `TaxonomyService`: Canonical dictionary service for managing shared and module-local terms.
- `TaxonomyOwnerReader` & `TaxonomyOwnerCategoryReader`: Read ports for owner categories and term hierarchies.
- `TaxonomyCategoryDeleteCleanupPort`: Cleans category tree references and compacts siblings upon category deletion.
- `TaxonomyTranslationTargetProvider`: Handles exact-locale translation synchronization and change tracking.

### Input DTOs/Commands
- `CreateTaxonomyTermInput`: Name, slug, scope type (Global vs Module), scope value, parent term ID, aliases.
- `UpdateTaxonomyTermInput`: Partial metadata updates, placement reordering, and translation updates.
- `SyncModuleCategoryInput`: Synchronizes external module-owned categories into the taxonomy tree.
- `SetTaxonomyCategoryPresentationInput`: Icon key, color, and media asset attachments.

### Domain Invariants
- **Scope Isolation & Precedence**: Module-scoped terms take precedence over global terms for owner modules.
- **Hierarchy Depth Bounds**: Maximum category hierarchy depth enforced by `MAX_TAXONOMY_CATEGORY_DEPTH`.
- **Cycle Prevention**: Moving or parenting categories enforces strict cycle detection (`candidate_hierarchy_rejects_cycle`).
- **Route Key & Slug Storage Bounds**: Route keys bounded to 120 characters, scope values to 64 characters.
- **No Shadowing Conflicts**: Aliases and translation slugs cannot shadow another term's canonical key or alias within the same scope.

### Events / Outbox Side Effects
- `DomainEvent::TaxonomyTermCreated { term_id }`
- `DomainEvent::TaxonomyTermUpdated { term_id }`
- `DomainEvent::TaxonomyTermDeleted { term_id }`

### Errors / Failure Codes
- `TaxonomyError::NotFound`: Target taxonomy term or category does not exist.
- `TaxonomyError::DuplicateSlug`: Slug collision within the same tenant and scope.
- `TaxonomyError::CycleDetected`: Attempted circular category parent assignment.
- `TaxonomyError::HierarchyDepthExceeded`: Category depth exceeds platform limit.
- `TaxonomyError::Validation`: Input validation or route key format failure.
- `TaxonomyError::Database`: Storage failure.
