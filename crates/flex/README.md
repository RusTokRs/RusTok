# flex

`flex` contains shared Flex contracts for attached and standalone modes.

## Purpose

- Provide transport-agnostic registry contracts for Flex field definitions.
- Let any domain module opt in to runtime-defined custom fields with a minimal adapter instead of rebuilding definitions, validation, localization and transport.
- Keep module-to-module dependencies clean while host adapters supply persistence and runtime wiring.

Flex is explicit opt-in. A domain entity does not become a Flex donor merely because it has a
`metadata`/JSON column.

## Responsibilities

- `FieldDefinitionService` trait.
- `FieldDefRegistry` runtime registry.
- Command/view DTOs plus owner-owned row-to-core, view-source, command-to-adapter-input mapping, persisted JSON shape, lifecycle guardrail, type-name, event helpers, and cache invalidation event taxonomy for field-definition CRUD orchestration.
- Owner-owned attached field-definition and standalone GraphQL query/mutation roots, runtime handle, and input/output DTOs under `flex::graphql`.
- Owner-owned standalone REST request/response DTOs, request-to-command mappings, and view-to-response mappings under `flex::rest`; the server controller remains only the Axum adapter.
- Owner-owned standalone fields_config parsing/schema building/serialization, localized field-key derivation, row-to-view mapping, entry normalization/schema validation, shared/localized split, read resolution, and PATCH merge helpers; server persistence adapters only expose source traits and adapt SeaORM rows into storage calls.
- `FlexModule` capability-only runtime metadata for the manifest-driven module registry.

## Donor policy

The runtime donor registry is a product contract, not an inventory of tables that happen to support JSON.

- `user`, `product`, and `order` are the currently registered attached donors.
- `forum.topic` is intentionally **not** a Flex donor. Forum topic metadata remains Forum-owned domain/internal state. Historical topic field-definition/localized-value storage is retained temporarily for a data audit and explicit cleanup migration; new topic custom-field CRUD is not exposed through the registry.
- `taxonomy.category` is the next planned donor after Taxonomy becomes the canonical Category owner. Taxonomy owns built-in category identity/hierarchy/localized copy/presentation; Flex supplies only administrator-defined extension fields.
- Future donors such as groups/profiles must opt in only with a demonstrated product surface and must reuse Flex rather than introduce a module-local custom-fields engine.

Critical normalized business invariants remain in their owner modules even for Flex-enabled entities.

## Multilingual status

The current Flex multilingual contract is already partially live and must be treated as canonical by contributors and agents:

- `FieldDefinition` carries explicit `is_localized` semantics in `rustok-core`, registry DTOs, GraphQL inputs, and attached-mode persistence.
- Standalone schema UI copy (`name`, `description`) no longer belongs in `flex_schemas`; it is stored in `flex_schema_translations`.
- Standalone entry payloads no longer treat inline locale-aware JSON as the canonical path: shared values stay in `flex_entries.data`, while locale-aware values live in `flex_entry_localized_values`.
- Generic attached localized value storage lives in the shared `flex` crate and persists into `flex_attached_localized_values`.
- Cleanup migrations remove residual inline locale-aware Flex payloads from donor metadata and standalone entry base rows; runtime resolves only shared payload plus parallel localized records.
- Authoring accepts only a valid normalized locale and prepares locale-aware updates from that exact row. Read-time fallback is a presentation concern and must never seed another locale or become input to a write.
- Attached field-definition and standalone schemas/entries GraphQL surfaces are live through manifest-driven host composition; GraphQL roots, runtime handle, permission checks, error mapping, event publication, and DTOs are owner-owned in `flex::graphql`. Standalone REST contract DTOs and view mappings are owner-owned in `flex::rest`, while server only supplies the Axum handler adapter, concrete standalone persistence adapter, and attached registry/cache/DB wiring through `FlexGraphqlRuntime`. Rollout/governance is enforced through the `capability_only` ghost-module manifest, `mod-flex` host wiring, explicit `flex_schemas:*` / `flex_entries:*` RBAC, and repo-side validation (`cargo xtask validate-manifest`, `cargo xtask module validate flex`, `node scripts/verify/verify-flex-multilingual-contract.mjs`, `node scripts/verify/verify-flex-standalone-contract.mjs`).
- Full end-to-end integration coverage remains an explicit verification debt; do not treat it as a contract gap or as permission to reintroduce inline localized storage.

Do not implement new Flex multilingual behavior from older plans that assume inline localized copy in base rows or treat JSON blobs as the canonical multilingual storage path.

## Interactions

- Depends on `rustok-core` (`FlexError`, `FieldType`, `ValidationRule`).
- Depends on `rustok-events` (`EventEnvelope`).
- Registered in `modules.toml` as a capability-only ghost module with `flex_schemas:*` and `flex_entries:*` permissions.
- Consumed by manifest-driven host schema composition, REST, and bootstrap wiring; GraphQL ownership, REST DTO/command-mapping ownership, field-definition row/view/command/persisted-JSON/lifecycle policy ownership, and standalone fields_config/schema/key-derivation/row-view/entry validation/split/merge ownership are in this crate, while the host supplies persistence/registry/cache adapters through `FlexGraphqlRuntime`.

## Entry points

- `flex::FlexModule`
- `flex::FieldDefRegistry`
- `flex::FieldDefinitionService`
- `flex::{CreateFieldDefinitionCommand, UpdateFieldDefinitionCommand, FieldDefinitionView, FieldDefinitionViewSource}`
- `flex::impl_field_definition_command_conversions!`
- `flex::graphql::{FlexQuery, FlexMutation, FlexGraphqlRuntime}`
- `flex::graphql::{FieldDefinitionObject, CreateFieldDefinitionInput, UpdateFieldDefinitionInput, DeleteFieldDefinitionPayload}`
- `flex::graphql::{FlexSchemaObject, FlexEntryObject, CreateFlexSchemaInput, UpdateFlexSchemaInput, CreateFlexEntryInput, UpdateFlexEntryInput, DeleteFlexPayload}`
- `flex::rest::{CreateFlexSchemaRequest, UpdateFlexSchemaRequest, CreateFlexEntryRequest, UpdateFlexEntryRequest, FlexSchemaResponse, DeleteFlexResponse}`
- `flex::{parse_standalone_fields_config, build_standalone_custom_fields_schema, serialize_standalone_fields_config, standalone_localized_field_keys}`
- `flex::{StandaloneSchemaViewSource, StandaloneSchemaTranslationSource, StandaloneEntryViewSource, standalone_schema_view_from_source, standalone_entry_view_from_source}`
- `flex::normalize_and_validate_standalone_entry`

## Docs

- Module documentation: [`docs/README.md`](./docs/README.md)
- Implementation plan: [`docs/implementation-plan.md`](./docs/implementation-plan.md)
- Platform Taxonomy/Flex Category plan: [`../../docs/architecture/taxonomy-flex-category-platform-plan.md`](../../docs/architecture/taxonomy-flex-category-platform-plan.md)
- Platform docs index: [`../../docs/index.md`](../../docs/index.md)