# AGENTS

This file is the canonical contributor and AI-agent governance contract for the RusToK repository.
It defines how changes are discovered, designed, implemented, verified, documented, and integrated.

The keywords MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are normative.
A repository-local rule may be stricter than this file but may not weaken it unless an accepted architecture decision explicitly changes the platform contract.

Do not create numbered, suffixed, legacy, or parallel variants of this governance file.
Update this canonical AGENTS.md in place.

## 1. Authority and source-of-truth hierarchy

RusToK distinguishes current implementation truth from accepted target architecture.

1. Current executable truth is the code, schema, manifests, and runtime wiring that actually exist.
2. Accepted target architecture is defined by active ADRs in DECISIONS/ and by canonical architecture documents linked from docs/index.md.
3. Component-local README.md and docs/ describe the live owner contract and must be synchronized with code.
4. An implementation plan describes sequencing, evidence, and remaining work. It MUST NOT silently redefine architecture.
5. Tests, fixtures, generated artifacts, comments, examples, and historical evidence are never authoritative over the canonical owner contract.

When sources disagree:
- do not choose the most convenient source;
- identify whether the discrepancy is current-runtime drift or unfinished accepted architecture;
- update the stale source in the same change when it is in scope;
- do not use stale documentation or stale code as justification for preserving an obsolete design.

Every important concept MUST have exactly one canonical owner and one canonical source of truth.
Other representations must be explicitly modeled as projections, adapters, overlays, caches, indexes, transport DTOs, or derived state.

An agent MUST NOT invent an exception to a platform rule.
If two mandatory rules genuinely conflict, surface the conflict and resolve it through the owning contract or an ADR rather than bypassing either rule.

## 2. Required start protocol

Before changing repository content, contributors and agents MUST:

1. Read docs/index.md, the canonical documentation map.
2. Read the relevant component README.md and local docs/README.md or implementation plan.
3. Before creating or renaming modules, crates, packages, folders, files, public types, query keys, config keys, or documentation, follow the Naming Contract in docs/standards/coding.md.
4. For new modules or major module refactors, read docs/modules/module-authoring.md before changing code.
5. Before frontend changes, read that frontend's AI_AGENT_RULES.md and the relevant module UI documentation.
6. Resolve ownership through docs/modules/registry.md and local component documentation.
7. Inspect active ADRs that govern the affected boundary before proposing a competing model.
8. Capture the current main commit SHA before starting work.
9. Identify the affected change surface before implementation.

The minimum change-surface review for a non-trivial change is:
- canonical owner and source of truth;
- domain model and invariants;
- persistence and migrations;
- all writers and mutation paths;
- all readers and projections;
- events, outbox, workers, caches, indexes, and search;
- public and internal transports;
- tenant, channel, locale, auth, policy, and trace context;
- UI and operator surfaces;
- fixtures, seeds, tests, scripts, generated/reference artifacts;
- current documentation and ADR impact.

Do not patch the first visible caller until the owning contract and affected surfaces are understood.

## 3. Multi-agent and Git concurrency protocol

RusToK is edited concurrently by multiple agents and contributors.
Stale-branch assumptions are therefore a correctness risk.

- Start a task from the current main unless the user explicitly assigns another base.
- Record the base SHA used for the task.
- Work only on the assigned task branch. Do not mutate another agent's branch.
- Before opening or updating a PR after substantial work, refresh main and compare base..main.
- Before merge, refresh main again and inspect changes that landed after the recorded base.
- If concurrent changes touch the same owner, schema, migration chain, public contract, event contract, generated surface, or files in the task boundary, re-read and reconcile them before proceeding.
- If concurrent changes are independent and do not change the affected contract, do not create meaningless churn merely to make every file identical to the latest main.
- Never resurrect an older branch model after main has established a newer canonical contract.
- Never force-update shared branches or rewrite another contributor's published history.
- Merge conflict resolution must preserve the current canonical architecture, not mechanically preserve both sides.

A PR description SHOULD record the relevant base SHA when concurrent platform work is active.

## 4. Ideal platform implementation standard

Treat every change as work on an ideal, long-lived, production-grade platform.
Deliver the coherent target design, not the smallest patch that makes one symptom or test disappear.

MUST:
- investigate and address root causes;
- preserve clear ownership and dependency direction;
- keep code, schema, callers, transports, tests, configuration, generated artifacts, and documentation consistent;
- implement required runtime wiring, not only source markers;
- verify observable behavior at the appropriate boundary.

MUST NOT:
- introduce hidden coupling, special-case branches, fragile workarounds, or knowingly duplicated sources of truth;
- leave a known inconsistency for a hypothetical later cleanup when it is part of the current cutover;
- create speculative infrastructure, registries, providers, feature flags, abstractions, or config keys without a current canonical consumer or requirement.

Prefer designs in which invalid states are unrepresentable through Rust types, database constraints, or both.

## 5. Ownership, boundaries, and dependency direction

Ownership is semantic, not merely directory-based.

- Platform foundation: crates/libs/*, apps/server composition, and shared infrastructure.
- Domain modules: crates/modules/*.
- Frontends: apps/admin, apps/storefront, apps/next-admin, apps/next-frontend.
- Operational tooling: scripts/, deployment/configuration files, observability tooling.
- Detailed ownership is defined in docs/modules/registry.md and component-local docs.

Rules:
- apps/server is a host and composition root, not a domain-logic dumping ground.
- A consumer MUST NOT read another module's private tables, repositories, entities, or implementation-only types merely because direct access is convenient.
- Cross-module behavior MUST use an owner-defined contract: port, event, provider seam, public transport contract, or another documented boundary.
- Shared libraries MUST have a clear owner, stable responsibility, and dependency direction.
- Do not put code into rustok-core, rustok-api, rustok-ui-core, or another shared crate merely because more than one consumer exists.
- Host applications compose owner-owned entrypoints; they do not absorb module-owned business rules.

### Reuse and abstraction

Reuse is driven by shared semantics and ownership, not textual similarity.

- Two similar implementations trigger an abstraction review, not automatic extraction.
- Extract a shared abstraction when consumers genuinely share the same stable contract, lifecycle, and dependency direction.
- Keep owner-local implementations separate when similar code represents different bounded-context semantics.
- Before adding reusable code, inspect existing canonical libraries and their documented responsibilities.
- Do not create a shared helper that forces one owner to depend on another owner's implementation details.

## 6. Canonical vocabulary

One concept SHOULD use one canonical term across schema, domain types, APIs, events, UI, tests, and current documentation.

- Do not introduce casual synonyms for the same domain concept when no boundary-specific distinction exists.
- A boundary-specific name is allowed only when it communicates a real difference in semantics.
- Names MUST follow docs/standards/coding.md.
- Do not encode implementation status, waves, temporary state, or replacement history into canonical internal names.

## 7. Data and write-side correctness

Write-side correctness takes priority over convenience.

### Database invariants

When PostgreSQL can enforce an invariant reliably, the invariant SHOULD be enforced in the database in addition to typed domain validation where useful.

Use the appropriate mechanism:
- primary, unique, and exclusion constraints;
- foreign keys, including tenant-scoped composite foreign keys where required;
- CHECK constraints;
- generated or derived columns when the database can own them safely;
- transactional or deferred constraint triggers for commit-time cross-row invariants;
- advisory or row locks where they are part of the canonical concurrency design.

Do not rely only on service-layer discipline for tenant ownership, uniqueness, referential integrity, impossible state combinations, or identity constraints that the database can enforce.

### Atomicity

A domain mutation that changes mutually dependent state MUST commit atomically.

This includes, when applicable:
- aggregate state;
- configuration that changes aggregate validity;
- revision or change cursors;
- durable outbox/event records;
- idempotency receipts;
- projection-invalidating facts;
- dependent rows required for the new invariant.

Do not implement required atomicity as a sequence of best-effort writes with later cleanup.

### Derived state

Derived state is not a second source of truth.

Caches, denormalized projections, search documents, computed identity, summaries, counters, and materialized read models MUST:
- declare their canonical source;
- be deterministically rebuildable or reconcilable;
- have explicit invalidation/revision semantics;
- never silently become the authoritative write model.

If derived state participates in correctness or uniqueness, its synchronization MUST be guaranteed by the database or transaction boundary rather than by caller convention alone.

### Tenant and effective-context integrity

Tenant, channel, locale, principal, policy, and other result-affecting dimensions MUST be resolved through canonical typed context and propagated explicitly.

- Do not reconstruct effective context independently inside downstream packages.
- Do not introduce package-local header/query/cookie fallback chains after canonical resolution.
- Cache keys, idempotency keys, projections, and policy decisions MUST include every effective dimension that changes their meaning.
- Cross-tenant references MUST be impossible by schema or canonical authorization rules, preferably both.

### Destructive and invalidating mutations

A configuration or schema change MUST NOT silently destroy business-owned data merely to restore consistency.

When a requested change conflicts with existing canonical data:
- reject with actionable diagnostics; or
- execute an explicit destructive/reconciliation operation whose consequences are part of the command contract.

Cascade deletion is appropriate only where ownership semantics truly mean the child has no independent business lifetime.

### Concurrency

For mutable state with revision, ordering, reservation, or optimistic-concurrency semantics:
- define lost-update behavior explicitly;
- do not bypass revision checks with last-write-wins unless that behavior is the accepted contract;
- preserve monotonic revisions/cursors where the platform relies on them.

### Retry safety and idempotency

Any operation that infrastructure may repeat SHOULD be naturally idempotent or protected by a stable idempotency identity/receipt.

This especially applies to:
- workers;
- webhook consumers;
- outbox/event consumers;
- imports;
- external integrations;
- long-running reconciliation commands.

### Time, money, quantities, and units

- Persist machine timestamps in UTC unless a domain contract explicitly requires another representation.
- User-facing calendar semantics MUST carry an explicit timezone where timezone changes meaning.
- Time-sensitive domain logic SHOULD use an injectable clock for deterministic tests.
- Money MUST NOT use binary floating-point.
- Currency and rounding semantics belong to the owning contract.
- Physical quantities SHOULD use typed units when unit ambiguity can affect correctness.

## 8. Events, projections, and read-side correctness

- Domain state and its events have one canonical owner.
- Do not create module-local parallel event buses or outbox mechanisms when the platform event/outbox path owns the responsibility.
- Where write + event durability must be atomic, use the canonical transactional event path.
- Event payloads SHOULD carry stable contract facts, not become full duplicate domain models.
- Read models may be denormalized for performance but MUST preserve domain semantics.
- Search and indexing MUST NOT flatten correlated domain state into combinations that do not exist.
- Projection updates MUST have deterministic rebuild/reconciliation and revision/cursor semantics appropriate to the owner contract.

## 9. Security, privacy, and failure behavior

Security-sensitive behavior MUST fail closed unless an explicitly documented degraded mode says otherwise.

- Missing authorization, policy, capability, or tenant context must not silently become permissive.
- Do not authorize from display labels, untrusted strings, or ad-hoc header conventions.
- Secrets, credentials, private keys, tokens, and sensitive personal values MUST NOT be emitted to logs, metrics, events, traces, audit payloads, fixtures, or error messages.
- Use canonical secret owners/handles rather than duplicating secret material.
- Errors at public or operator boundaries SHOULD be actionable without exposing sensitive internals.
- Security-relevant denials and degraded states SHOULD be observable through structured logs/metrics/traces where appropriate.

## 10. Performance and operational behavior

Correctness and ownership come first, but obvious scalability hazards are not acceptable.

- Avoid N+1 IO on hot or unbounded paths.
- Potentially unbounded collections MUST have explicit pagination, streaming, or bounded batch semantics.
- Database indexes SHOULD follow real query contracts and measured access patterns.
- Do not create a cache or duplicated read model solely to hide a broken ownership/query design.
- Optimization MUST NOT create a competing source of truth.

Observability is part of the runtime contract for important failure modes.
New asynchronous, external, or operational boundaries SHOULD define the structured logs, metrics, traces, and correlation identifiers needed to diagnose failure without leaking secrets.

## 11. Architecture decisions and implementation plans

Non-trivial architecture changes MUST be captured in DECISIONS/ using an ADR.

An ADR SHOULD state:
- problem and scope;
- canonical owner and source of truth;
- accepted vocabulary;
- allowed and forbidden states;
- persistence and database invariants;
- atomicity/concurrency boundary;
- tenant/channel/locale/auth dimensions;
- event and projection effects;
- failure and destructive-change behavior;
- migration/cutover implications;
- alternatives and rationale;
- verification requirements.

ADR history is preserved by supersession, not by silently rewriting architectural history.
A later decision may supersede an earlier one; current architecture docs should point to the active decision.

Implementation plans answer how and in what sequence accepted architecture is delivered.
They MUST NOT introduce a competing architecture merely because it is easier to implement.

Do not create a new document when a suitable canonical document already exists.
Extend the existing document instead.

## 12. Documentation policy

All repository artifacts are English-only, including code, comments, documentation, commit messages, examples, fixtures intended as documentation, and generated text artifacts.
README.ru.md is the sole repository-document exception.

Platform-wide documentation lives in docs/.
Component documentation lives with the component.

When changing architecture, APIs, events, modules, tenancy, routing, UI contracts, persistence contracts, or observability:
- update the affected local component docs;
- update related central docs;
- update docs/index.md when the documentation map changes;
- update docs/modules/registry.md when module ownership/composition changes;
- update verification documentation when the verification contract changes;
- delete superseded current-state guidance once replacement is canonical.

Documentation MUST describe the actual current state and, where relevant, clearly distinguish unfinished accepted target work from already implemented behavior.

## 13. Initial implementation and zero-legacy policy

RusToK is a pre-release initial implementation.
Internal repository code has no legacy consumers that justify compatibility layers.

Implement the canonical target architecture directly.

- A replacement is atomic: update every repository-owned caller, transport, schema, fixture, test, seed, script, generated artifact, and current document, then delete the superseded implementation in the same change.
- Do not add or retain compatibility wrappers, deprecated aliases, dual read/write paths, fallback-to-legacy behavior, old/new adapters, or parallel implementations of the same internal contract.
- Do not create internal numbered/version-family routes, types, modules, GraphQL fields, storage envelopes, or package exports to avoid completing a cutover.
- The surviving internal implementation uses the canonical unversioned name.
- Existing legacy encountered inside the task boundary must be removed, not extended or hidden behind another facade.
- For unreleased schema work, amend or consolidate pending migrations instead of stacking corrective migrations solely to preserve repository history.
- Before declaring completion, search for removed names and verify that remaining occurrences are explicitly preserved historical evidence rather than executable code or current guidance.

Zero-legacy does not authorize silent data loss.
Deleting an obsolete API/model and transforming or discarding persisted data are separate decisions.
A change MUST determine whether persisted state is disposable development/fixture state, canonical data requiring deterministic transformation, or externally preserved history.

A temporary compatibility bridge is allowed only when the user explicitly requires staged compatibility for an external consumer.
It must have a named removal owner, deadline, and deletion task in the owning module plan.

Version identifiers are allowed only at genuine independently deployed external boundaries such as published package releases, immutable migration ordering, or public wire/event contracts consumed outside this repository.
Boundary versioning MUST map immediately to the single canonical internal model and MUST NOT fork internal domain logic.

## 14. No stubs, fake completion, or suppression

A warning, unused symbol, failing test, missing caller, missing runtime registration, or missing transport is evidence to investigate, not something to silence.

Do not use:
- todo or unimplemented placeholders as delivered functionality;
- no-op handlers;
- unconditional success;
- fake adapters or dummy persistence;
- in-memory fallbacks as substitutes for required durable behavior;
- broad dead-code or unused lint suppression to make incomplete code appear clean.

If required target behavior exists but is unwired, wire it through the real owner, composition, transport, and UI path and verify observable behavior.
If the code is not part of the target architecture, delete it and its stale tests, fixtures, configuration, and documentation.

Generated code and genuinely externally-invoked symbols that static analysis cannot observe may use the narrowest justified expectation/suppression with an explicit reason and evidence of the external entry path.

Source-marker and compile-only checks MAY supplement behavior tests but MUST NOT substitute for missing runtime functionality.

## 15. Verification strategy

Verification is change-driven.

Contributors and agents MUST identify the affected boundaries and run or request the canonical checks for those boundaries rather than relying only on a full workspace run or only on a narrow unit test.

Use docs/verification/README.md and component verification guides as the canonical command source.

Typical hierarchy:
1. type/unit checks for local pure logic;
2. database/integration tests for persistence invariants and transactional behavior;
3. transport contract tests for API mapping, auth, and error semantics;
4. composition/runtime tests for registration and real entrypaths;
5. projection/search/outbox/worker evidence when those boundaries changed;
6. targeted UI evidence when UI changed;
7. broader platform checks when composition or shared contracts changed.

Never bypass or disable pre-commit/pre-push hooks.
Fix the root cause of failures.

Do not edit CI/CD workflow files unless the user explicitly requests CI/CD changes.

When diagnosing GitHub Actions failures, follow the current runbook in docs/verification/README.md and use the repository-provided failed-log tooling before reasoning from stale logs.

If the user explicitly chooses to run tests themselves, an agent may leave execution to the user, but MUST:
- state exactly which checks were not run;
- never describe unexecuted checks as green;
- still perform all feasible static/repository consistency checks relevant to the change.

## 16. Definition of Done

A task is complete only when every applicable item below is true:

- one canonical implementation remains;
- the canonical owner and source of truth are clear;
- repository-owned callers are migrated;
- old implementations and stale aliases are removed;
- required database invariants are enforced at the correct layer;
- tenant/context integrity is preserved;
- transaction, concurrency, retry, and destructive-change semantics are correct;
- events/outbox/projections/indexes are consistent with the mutation;
- public/internal transports are synchronized with the owner contract;
- UI/operator surfaces are synchronized where applicable;
- fixtures, seeds, tests, scripts, generated/reference artifacts, and configuration match the new contract;
- current documentation matches implementation;
- architecture changes have the required ADR;
- removed names and obsolete concepts have been searched repository-wide;
- no task-local placeholder, hidden fallback, unexplained suppression, or known inconsistent path remains;
- applicable verification has passed, or unrun checks and blockers are reported explicitly.

A partially implemented target MUST be described as incomplete.
Do not report completion merely because the edited files compile.

## 17. Specialized mandatory rules

The following documents contain mandatory specialized contracts and are part of this governance model:

- docs/index.md — canonical documentation map;
- docs/standards/coding.md — naming and code-quality contract;
- docs/modules/module-authoring.md — module creation/refactor contract;
- docs/modules/registry.md — module ownership and readiness registry;
- docs/architecture/principles.md — platform architecture principles;
- docs/architecture/api.md — API and transport contracts;
- docs/architecture/database.md — database contract;
- docs/architecture/i18n.md — localization contract;
- docs/verification/README.md — canonical verification entrypoint;
- frontend-local AI_AGENT_RULES.md files — mandatory rules for their owning frontend.

Specialized documents SHOULD carry detailed, fast-changing operational instructions.
This root file SHOULD remain focused on stable repository-wide invariants and point to specialized runbooks rather than duplicating them.

## 18. AI-agent operating rules

All automated agents MUST follow the complete repository governance above and additionally:

1. Read docs/index.md before repository work.
2. Read local AI_AGENT_RULES.md before frontend modifications.
3. Use the current main as the normal starting point and follow the multi-agent concurrency protocol.
4. Do not create a new documentation file when an existing canonical document is suitable.
5. Do not modify another agent's branch.
6. Do not bypass hooks, policy checks, or verification by editing generated outputs only.
7. Do not invent package-local auth, tenant, channel, locale, i18n, event, storage, or observability contracts where a platform contract already exists.
8. Keep module-owned UI and transport boundaries aligned with docs/modules/module-authoring.md and frontend-local rules.
9. Do not treat required parallel platform surfaces as legacy merely because more than one transport or host exists; distinguish intentional target surfaces from obsolete duplicate implementations.
10. Communicate concrete blockers and unverified checks explicitly rather than hiding them with stubs, fallbacks, compatibility layers, or optimistic completion claims.
