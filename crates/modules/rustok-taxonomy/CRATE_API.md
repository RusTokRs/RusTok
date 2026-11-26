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
- **Module dependencies**: `["outbox"]` for storage migrations and event delivery.
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
- **Locale**: Resolves localized terms with the platform helpers in `rustok-api::locale`.
- **Outbox**: Writes taxonomy lifecycle and translation synchronization events through `rustok_outbox_storage::TransactionalEventBus`.
- **SEO & Translation Targets**: Implements targets for hierarchical category SEO and multi-locale term translations.
- **Downstream Consumers**: `product`, `groups`, `forum`, `blog`, `profiles`, and `comments` use Taxonomy categories, tags, or terms. Distribution composes the Product and Groups native adapters from these Taxonomy owner methods.

## Minimum Contract Set
- `rustok-taxonomy-ports::TaxonomyTermNamesReadPort`: Bounded tenant-scoped name
  resolution with embedded and authenticated gRPC providers. The standalone
  `rustok-taxonomy-service` binary serves this read profile.
- `TaxonomyService`: Canonical dictionary service for managing shared and module-local terms.
- `TaxonomyOwnerReader` & `TaxonomyOwnerCategoryReader`: Read ports for owner categories and term hierarchies.
- `TaxonomyOwnerReader::load_scoped_terms_in` and `TaxonomyService::resolve_term_id_for_module_in` read scoped vocabulary in the consuming owner's transaction.
- `sync_module_category_copy_in_tx` synchronizes localized copy and hierarchy while retaining presentation state and extending owner-managed route history.
- `TaxonomyOwnerReader::load_terms_for_module_in` and `load_term_ids_for_module_in` read global and module terms on the caller's connection.
- `TaxonomyService::delete_module_category_with_cleanup` preserves the callback's typed error while rolling back the canonical delete on refusal.
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

### Transactional module route and strict hierarchy readers

- `TaxonomyService::resolve_term_route_for_module_in<C: ConnectionTrait>` resolves
  the route registry and alias identity on the same supplied connection/transaction.
  The existing pool-based method delegates to it; locale/module/global precedence
  and alias provenance are unchanged. Consumers still authorize their own binding.
- `TaxonomyOwnerCategoryReader::load_module_category_parents_in_strict` verifies
  every selected ID is a tenant-local Category in the exact module scope and has
  an explicit hierarchy placement. Duplicate IDs return Validation; missing or
  wrong-scope membership/placement returns Conflict, never synthetic roots. Empty
  selection remains empty. It does not require a translation or alter the generic
  legacy root-default placement reader. Full-graph cycle/depth validation belongs
  to the consumer taking the whole-tenant snapshot or the hierarchy write owner.
- The server's Forum adapter consumes these APIs without importing Taxonomy
  persistence entities or creating a Taxonomy -> Forum dependency. Its real-owner
  transaction/rollback tests and the strict-reader missing-placement regression
  are authored but unexecuted until a Rust-enabled environment runs them.
