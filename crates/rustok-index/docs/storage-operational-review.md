# Index storage operational review

## Purpose

This document evaluates the non-numeric operational properties of the three M2
PostgreSQL storage candidates. It complements the generated replacement `100k`
and `1m` comparison; it does not replace measured latency, size, WAL, buffer,
planner, or maintenance evidence and it does not select the production model by
itself.

The review is architecture-driven and remains valid across benchmark reruns
unless a candidate's physical design or the accepted Index ownership boundary
changes.

## Audit result

The M2 audit found that the first scale packet could not be used as canonical
selection evidence after the benchmark identity contract changed:

- entity reads and mutation workloads now constrain the complete generic schema
  identity: module, entity, and schema version;
- typed EAV field rows now store the same complete schema identity, include it in
  their primary and lookup keys, and reference the matching entity envelope;
- typed EAV joins, mutations, deletes, and maintenance churn constrain that full
  identity;
- JSONB and typed EAV maintenance entity mutations now constrain module, entity,
  and schema version instead of relying on `entity_name` alone;
- static verification locks these predicates and keys before replacement scale
  evidence is accepted.

The previously archived smoke and 100k packets remain useful historical harness
and diagnostic evidence, but the storage ADR must cite replacement same-commit
`100k` and `1m` packets generated after these corrections.

## Required production properties

The selected canonical storage must:

- represent arbitrary owner-published modules, schemas, versions, fields, and
  links without source-domain code in Index core;
- preserve the complete entity identity: tenant, module, entity, schema version,
  entity ID, and locale;
- apply entity and link mutations atomically with monotonic source-version
  guards;
- support deterministic typed filtering, ordering, projection, exact count, and
  keyset pagination;
- permit online secondary-index evolution and observable rebuild/backfill work;
- expose diagnostics that operators can relate to schemas, fields, indexes,
  partitions, ingestion lag, and maintenance pressure;
- avoid requiring exclusive table rewrites for ordinary schema publication or
  routine health maintenance.

Links remain independently relational for every candidate, so link persistence
and traversal operations do not decide the entity-payload model.

## Evaluation dimensions

| Dimension | Question |
| --- | --- |
| Genericity | Can a new module/schema be registered without Index-core code or a source-specific table? |
| Schema evolution | Does adding or retiring a field require table DDL, data rewrites, or source-specific migrations? |
| Index management | Can typed hot-path indexes be created, audited, rebuilt, and retired predictably? |
| Mutation atomicity | How many physical rows and indexes must one logical entity mutation coordinate? |
| Query compilation | How much candidate-specific join, cast, deduplication, and alias logic is required? |
| Constraints | Which invariants are enforced by PostgreSQL versus registry validation and ingestion code? |
| Diagnostics | Can operators identify the schema/field responsible for size, latency, WAL, or dead tuples? |
| Rebuild and partitioning | Can data be streamed, shadow-built, partitioned, and cut over without source-table reads? |

## JSONB entity rows

### Operational strengths

- One canonical entity row contains the source version and complete validated
  payload, so a logical entity update does not fan out across one row per field.
- New fields and new owner-published schemas do not require adding PostgreSQL
  columns or source-specific tables.
- The complete generic identity is represented directly in the entity primary
  key and can be used consistently by reads, mutations, rebuilds, and drift
  repair.
- General JSON containment and schema-specific typed expression indexes can
  coexist: the registry/query planner can use typed indexes for declared hot
  fields without changing the base row shape.
- Entity-level diagnostics, cardinality checks, source-version guards, and
  shadow rebuilds map naturally to one row per entity/locale.

### Operational costs and controls

- PostgreSQL cannot enforce every registry-declared field type inside an
  arbitrary payload. Record validation must remain mandatory before persistence,
  and production migrations must constrain the stable envelope separately from
  payload semantics.
- Typed comparison and ordering require controlled casts or generated/indexed
  expressions. The schema/index manager must own deterministic index names,
  predicates, expression definitions, concurrent creation, readiness, and
  retirement.
- A general GIN index is not a substitute for workload-specific typed indexes;
  its size and write amplification must be justified by measured query coverage.
- Query compilation and maintenance must always include the full schema identity.
  Partial module/entity/version predicates can invalidate correctness and
  benchmark conclusions.
- Large payload rewrites may create tuple and index churn even when only one
  field changes, so WAL and maintenance evidence remains a decision input.

### Eligibility

Eligible as canonical generic storage, subject to acceptable corrected scale
metrics and explicit production rules for typed indexes, partitions, validation,
and source-version/atomic-link transactions.

## Typed EAV rows

### Operational strengths

- New fields can be represented without adding table columns, and values retain
  typed PostgreSQL columns rather than relying on JSON casts at query time.
- The corrected prototype stores module, entity, schema version, entity ID, and
  locale on every field row and enforces an entity-envelope foreign key.
- A shared typed-index family can serve many schemas when the complete schema
  identity, field name, and value type are included in deterministic keys.
- Individual field rows can make field-level storage and selectivity visible to
  operators.

### Operational costs and controls

- One logical entity expands into an identity row plus multiple field rows,
  increasing row count, index entries, dead tuples, vacuum work, and transaction
  surface area.
- Every projection or predicate spanning fields requires joins, aliases, and
  careful ordinal handling; multi-value fields additionally require
  deduplication and cardinality-aware planning.
- The foreign key prevents orphaned rows, but production persistence must also
  prevent mixed typed values, duplicate ordinals, and incomplete logical entity
  replacement under concurrent ingestion.
- Atomic mutation, delete, rebuild, and drift repair must coordinate the entity
  envelope and every field row. Partial success is not acceptable.
- Operator diagnostics must aggregate many physical rows back to logical
  schema/entity/field identities, making capacity and maintenance analysis more
  complex than one-row entity storage.

### Eligibility

Architecturally eligible after the full-identity correction, but its materially
higher mutation, query-planning, and maintenance complexity requires a decisive
replacement-evidence advantage to justify selection over JSONB.

## Hot typed projection

### Operational strengths

- Native typed columns and entity-specific indexes provide the simplest SQL and
  the best-case physical baseline for a known fixed schema.
- PostgreSQL constraints, statistics, and operator tooling map directly to
  dedicated columns and tables.

### Architectural incompatibility

- Every new entity type or indexed field requires source-specific table/index
  DDL, migrations, query code, and operational rollout.
- The shape hard-codes Product, Variant, and SalesChannel semantics into storage,
  contrary to the generic Index Engine ownership boundary.
- Concurrent schema versions and extension-defined schemas would multiply
  bespoke tables and migration paths.
- A new module could not become queryable through ordinary schema/source
  registration alone.

### Eligibility

Not eligible as the canonical generic Index storage model. It remains a
best-case comparison baseline. A future optional derived projection or cache
would require a separate measured design and may not replace the canonical
source-versioned generic representation.

## Cross-candidate operational result

| Candidate | Generic canonical storage | Field publication without base-table DDL | Logical mutation surface | Query/compiler complexity | Provisional operational position |
| --- | --- | --- | --- | --- | --- |
| JSONB entity rows | Yes | Yes | One entity row plus independent links | Moderate; typed expressions and casts | Simplest eligible candidate |
| Typed EAV | Yes after full-identity correction | Yes | Entity plus many field rows and links | High; joins, ordinals, deduplication | Requires decisive evidence advantage |
| Hot typed projection | No | No | Dedicated typed row plus links | Low for fixed schemas | Baseline only; reject as canonical |

This result narrows the ADR decision boundary but does not choose a winner:

1. the hot typed projection must be rejected as canonical storage unless the
   accepted Index architecture is changed by a separate decision;
2. JSONB and typed EAV remain the measured generic candidates;
3. typed EAV must overcome its additional mutation, query-planning, and
   maintenance burden with a clear corrected scale advantage;
4. the accepted ADR must cite replacement same-commit `100k` and `1m` packets
   generated after the full-identity corrections.

## ADR completion checklist

Before accepting the storage ADR:

- inspect provenance and exact cardinality/digest parity for both replacement
  packets;
- verify full-schema-identity predicates and EAV field keys in recorded SQL and
  plans;
- compare first-run and warm-run latency, buffers, planner shapes, load/size
  ratios, mutation WAL, churn, dead tuples, and VACUUM behavior;
- specify the production entity/link envelope, source-version and transaction
  rules, partitioning, and secondary-index lifecycle for the selected model;
- record a concrete rejection reason for every alternative;
- keep production migrations absent until that accepted revision is merged.
