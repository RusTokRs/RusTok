# flex

`flex` contains shared Flex contracts for attached and standalone modes.

## Purpose

- Provide transport-agnostic registry contracts for Flex field definitions.
- Keep module-to-module dependencies clean while host adapters supply persistence and runtime wiring.

## Responsibilities

- `FieldDefinitionService` trait.
- `FieldDefRegistry` runtime registry.
- Command/view DTOs for field-definition CRUD orchestration.
- Owner-owned attached field-definition and standalone GraphQL query/mutation roots, runtime handle, and input/output DTOs under `flex::graphql`.
- Owner-owned standalone REST request/response DTOs and view-to-response mappings under `flex::rest`; the server controller remains only the Loco/Axum adapter.
- `FlexModule` capability-only runtime metadata for the manifest-driven module registry.

## Multilingual status

The current Flex multilingual contract is already partially live and must be treated as canonical by contributors and agents:

- `FieldDefinition` now carries explicit `is_localized` semantics in `rustok-core`, registry DTOs, GraphQL inputs, and attached-mode persistence.
- Attached-mode registered consumers are `user`, `product`, `order`, and `topic`. `node` is not part of the live attached contract yet.
- Standalone schema UI copy (`name`, `description`) no longer belongs in `flex_schemas`; it is stored in `flex_schema_translations`.
- Standalone entry payloads no longer treat inline locale-aware JSON as the canonical path: shared values stay in `flex_entries.data`, while locale-aware values now live in `flex_entry_localized_values`.
- Generic attached localized value storage now lives in the shared `flex` crate and persists into `flex_attached_localized_values`; live donor read/write paths now exist for `user`, `product`, `order`, and `topic`.
- `topic` is no longer schema-only: forum topics now use `forum_topics.metadata` as the donor payload, and locale-aware Flex keys are resolved through the same attached multilingual contract as the other live donors.
- Cleanup migrations remove residual inline locale-aware Flex payloads from donor metadata and standalone entry base rows; runtime resolves only shared payload plus parallel localized records.
- Attached field-definition and standalone schemas/entries GraphQL surfaces are live through manifest-driven host composition; GraphQL roots, runtime handle, permission checks, error mapping, event publication, and DTOs are owner-owned in `flex::graphql`. Standalone REST contract DTOs and view mappings are owner-owned in `flex::rest`, while server only supplies the Loco/Axum handler adapter, concrete standalone persistence adapter, and attached registry/cache/DB wiring through `FlexGraphqlRuntime`. Rollout/governance is enforced through the `capability_only` ghost-module manifest, `mod-flex` host wiring, explicit `flex_schemas:*` / `flex_entries:*` RBAC, and repo-side validation (`cargo xtask validate-manifest`, `cargo xtask module validate flex`, `node scripts/verify/verify-flex-multilingual-contract.mjs`, `node scripts/verify/verify-flex-standalone-contract.mjs`).
- Full end-to-end integration coverage remains an explicit verification debt; do not treat it as a contract gap or as permission to reintroduce inline localized storage.

Do not implement new Flex multilingual behavior from older plans that assume inline localized copy in base rows or treat JSON blobs as the canonical multilingual storage path.

## Interactions

- Depends on `rustok-core` (`FlexError`, `FieldType`, `ValidationRule`).
- Depends on `rustok-events` (`EventEnvelope`).
- Registered in `modules.toml` as a capability-only ghost module with `flex_schemas:*` and `flex_entries:*` permissions.
- Consumed by manifest-driven host schema composition, REST, and bootstrap wiring; GraphQL ownership and REST DTO ownership are in this crate, while the host supplies persistence/registry/cache adapters through `FlexGraphqlRuntime`.

## Entry points

- `flex::FlexModule`
- `flex::FieldDefRegistry`
- `flex::FieldDefinitionService`
- `flex::{CreateFieldDefinitionCommand, UpdateFieldDefinitionCommand, FieldDefinitionView}`
- `flex::graphql::{FlexQuery, FlexMutation, FlexGraphqlRuntime}`
- `flex::graphql::{FieldDefinitionObject, CreateFieldDefinitionInput, UpdateFieldDefinitionInput, DeleteFieldDefinitionPayload}`
- `flex::graphql::{FlexSchemaObject, FlexEntryObject, CreateFlexSchemaInput, UpdateFlexSchemaInput, CreateFlexEntryInput, UpdateFlexEntryInput, DeleteFlexPayload}`
- `flex::rest::{CreateFlexSchemaRequest, UpdateFlexSchemaRequest, CreateFlexEntryRequest, UpdateFlexEntryRequest, FlexSchemaResponse, FlexEntryResponse, DeleteFlexResponse}`

## Docs

- Module documentation: [`docs/README.md`](./docs/README.md)
- Implementation plan: [`docs/implementation-plan.md`](./docs/implementation-plan.md)
- Platform docs index: [`../../docs/index.md`](../../docs/index.md)
