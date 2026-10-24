# rustok-moderation / CRATE_API

## Public surface

- Modules: `api`, `application`, `application_dispatch`, `application_recovery`, `domain`, `entities`, `error`, `migrations`, `model`, `module`, `ports`, `provider`, `service`.
- Types: `ModerationModule`, `ModerationService`, `ModerationSubjectCommandPort`, `ModerationSubjectAdapterRegistry`, `ModerationSubjectAdapterKey`, `ModerationCommandPort`, `ModerationRecoveryCommandPort`, `ModerationReadPort`, `ModerationError`, `ModerationResult<T>`.
- Core DTOs: `SubmitModerationReportCommand`, `OpenModerationCaseCommand`, `AssignModerationCaseCommand`, `DecideModerationCaseCommand`, `ApplyModerationDecisionCommand`, `ModerationDecisionApplication`.

## Public modules
`api`, `application`, `application_dispatch`, `application_recovery`, `domain`, `entities`, `error`, `migrations`, `model`, `module`, `ports`, `provider`, `service`.

## Platform Capability Bindings
- **Core Module Registration**: Exposes `ModerationModule` implementing `RusToKModule` (slug `"moderation"`, kind `ModuleKind::Core`) and `MigrationSource`.
- **Moderation Service & Case Management**:
  - `ModerationService` managing reports, cases, investigator assignments, decisions, and application operations.
  - Runtime extension `ModerationApplicationWorkRegistration` integrating with `rustok-runtime::ModuleWorkScheduler`.
- **Neutral Subject Command Ports**:
  - `ModerationSubjectCommandPort` enabling domain modules (Forum, Groups, Comments) to apply moderation decisions to their own storage without cross-owner persistence leaks.
  - `ModerationSubjectAdapterRegistry`, `ModerationSubjectAdapterFactory`, and `ModerationSubjectAdapterKey`.
  - `ModerationSubjectRef`, `ModerationScopeRef`, `ApplyModerationDecisionCommand`, and `ModerationDecisionApplication`.
- **Application Recovery & Audit**:
  - `ModerationRecoveryPort`, `ModerationRecoveryDecision`, and `ModerationRecoveryDecisionAction` for operator review and recovery of terminal operations.

## Domain Collaborations
- **Core Platform**: Integrates with `rustok-core` module system and `rustok-api` permissions (`MODERATION_CASES_READ`, `MODERATION_CASES_LIST`, `MODERATION_CASES_OVERRIDE`, `MODERATION_CASES_MANAGE`).
- **Runtime Work Fleet**: Registers background task adapter with `rustok-runtime` for periodic leasing and dispatch of due decision applications.
- **Domain Modules**: Interacts with domain crates exclusively via `ModerationSubjectCommandPort` and the host-assembled registry.

## Primary Public Types and Signatures
- `pub struct ModerationModule`
- `pub struct ModerationService`
- `pub trait ModerationSubjectCommandPort`
- `pub struct ModerationSubjectAdapterRegistry`
- `pub struct ModerationSubjectAdapterKey`
- `pub struct ModerationSubjectRef`
- `pub struct ModerationScopeRef`
- `pub struct SubmitModerationReportCommand`
- `pub struct OpenModerationCaseCommand`
- `pub struct AssignModerationCaseCommand`
- `pub struct DecideModerationCaseCommand`
- `pub struct ApplyModerationDecisionCommand`
- `pub struct ModerationDecisionApplication`
- `pub enum ModerationError`

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-api`
- `rustok-runtime`

## Common AI Mistakes
- Importing domain modules (e.g. `rustok-forum` or `rustok-groups`) directly into `rustok-moderation` — dispatch must use `ModerationSubjectCommandPort`.
- Bypassing the CAS operation lease state machine (`Pending` -> `Applying` -> `Applied` / `Retryable`) when dispatching decision effects.
- Overwriting case assignments or decision records without validating source revisions.
- Swallowing adapter dispatch errors instead of classifying them into `Retryable`, `Rejected`, or `OperatorReview`.

## Minimum Contract Set

### Input DTOs/Commands
- `SubmitModerationReportCommand` (`reporter_kind`, `reporter_id`, `scope`, `subject`, `reason_code`, `notes`)
- `OpenModerationCaseCommand` (`scope`, `subject`, `priority`, `initial_reason_code`, `notes`)
- `AssignModerationCaseCommand` (`case_id`, `investigator_id`, `expected_revision`)
- `DecideModerationCaseCommand` (`case_id`, `decision_kind`, `reasons`, `effects`, `notes`, `expected_revision`)
- `ApplyModerationDecisionCommand` (`decision_id`, `case_id`, `tenant_id`, `subject`, `scope`, `effect`)
- `ModerationDecisionApplication` (`operation_id`, `decision_id`, `status`, `receipt`)

### Domain Invariants
- Case revision fencing: Any mutation against a case requires `expected_revision`; mismatches fail closed with `ModerationError::Conflict`.
- Tenant isolation: Nil `tenant_id` is strictly rejected across all service methods, queries, and ports.
- Bounded lease execution: Operation leases expire after `DEFAULT_APPLICATION_LEASE_SECONDS` and are bounded by `MAX_APPLICATION_LEASE_SECONDS`.
- Retry exponential backoff: Retries are bounded between `APPLICATION_RETRY_BASE_SECONDS` and `APPLICATION_RETRY_MAX_SECONDS`.

### Events / Outbox Side Effects
- Decision enforcement creates durable rows in `moderation_application_operation` with lease expiration fencing.
- Dispatch completions transition operations to `Applied` or schedule retries in accordance with error classification.

### Errors / Failure Codes
- `ModerationError` variants: `Validation`, `NotFound`, `Conflict`, `Forbidden`, `Timeout`, `Unavailable`, `InvariantViolation`, `Database`.
- `ModerationCommandValidationError` and `ModerationSubjectAdapterBuildError`.
