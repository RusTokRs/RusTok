# Reactions Crate API Specification

## Public surface

Deliberate public modules and exports:

- `model`: Core domain types (`ReactionCatalog`, `ReactionSubjectRef`, `ApplyReactionCommand`, `ReactionSnapshot`, `ReactionActorState`, `ReactionAggregate`, etc.)
- `provider`: Reaction provider interfaces, registries, and ports (`ReactionSubjectProvider`, `ReactionSubjectRegistry`, `ReactionReadPort`, `ReactionWritePort`, etc.)
- `module`: `ReactionsModule` implementing `RusToKModule` and `MigrationSource`
- `reconciliation`: Aggregate reconciliation types (`ReactionReconciliationRequest`, `ReactionReconciliationReport`, `RepairReactionSubjectCommand`, etc.)
- `service`: `ReactionsService`
- `graphql`: Optional GraphQL schema attachment and resolvers (`ReactionsQuery`, `ReactionsMutation`)

## Platform Capability Bindings
- **RusToKModule Implementation**: `ReactionsModule` in `src/module.rs`
- **Module Slug**: `reactions`
- **Module Name**: `Reactions`
- **Module Kind**: `ModuleKind::Optional`
- **Dependencies**: `["outbox"]`
- **Runtime Extensions**:
  - `ensure_reaction_subject_registry` registers shared reaction subject provider registry
  - `ensure_reaction_subject_factory_registry` registers reaction subject factory registry
- **Event Listeners**:
  - `ReactionTargetDeletionHandler`: Listens for domain subject deletion events to cascade clean reaction records.
- **Database Migrations**: Sea-ORM migrations defined in `src/migrations/mod.rs` with dependency on `outbox`.

## Domain Collaborations
- **Outbox**: Publishes reaction events through `rustok_outbox::TransactionalEventBus`.
- **Subject Providers**: Modules owning revisioned content (e.g. `blog`, `forum`, `comments`) implement `ReactionSubjectProvider` to validate reaction rules and permissions.
- **GraphQL**: Exposes `ReactionsQuery` and `ReactionsMutation` with schema attachment.

## Minimum Contract Set
- `ReactionsService`: Subject reactions query, toggle/react/unreact mutation, actor state resolution.
- `ReactionSubjectProvider` & `ReactionSubjectRegistry`: Dynamic registry of reaction targets and authorization handlers.
- `ReactionReconciliationReport`: Discrepancy detection between reaction counters and actor states.

### Input DTOs/Commands
- `ReactToSubjectInput`: Subject ID, subject type, reaction key, actor identification.
- `UnreactFromSubjectInput`: Subject ID, subject type, reaction key, actor identification.
- `ReactionReconciliationRequest`: Parameters for verifying reaction counter integrity against individual actor rows.
- `RepairReactionSubjectCommand`: Action to recalculate and reconcile corrupted aggregate reaction counters.

### Domain Invariants
- **Bounded Reaction Keys**: Allowed reaction keys are validated against subject configuration (`bounded_multiple_selection_rejects_excess_keys`).
- **Single vs Multiple Selection Modes**: Subjects enforce either exclusive single reaction per actor (e.g., upvote/downvote) or multiple independent emoji reactions.
- **Tenant Scope Enforcement**: Reactions strictly enforce tenant boundaries (`validate_subject_tenant_enforces_exact_tenant`).
- **Idempotent Mutations**: Repeating the same reaction or removing an un-selected reaction is an idempotent no-op.
- **Cascading Target Deletion**: Deleting the owning entity automatically marks or purges associated reactions.

### Events / Outbox Side Effects
- `ReactionRecorded`: Emitted when an actor reacts to a subject.
- `ReactionRevoked`: Emitted when an actor removes a reaction from a subject.

### Errors / Failure Codes
- `ReactionError::SubjectNotFound`: Target entity does not exist or provider is missing.
- `ReactionError::DisallowedKey`: Selected emoji/reaction key is not permitted for the subject.
- `ReactionError::TenantMismatch`: Cross-tenant reaction attempt rejected.
- `ReactionError::Unauthorized`: Actor does not satisfy subject provider permission checks.
- `ReactionError::ReconciliationFailed`: Aggregate counter repair could not be verified.
- `ReactionError::Database`: Storage layer error.
