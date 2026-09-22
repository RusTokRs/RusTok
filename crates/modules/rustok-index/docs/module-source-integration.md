# Integrating a source module with `rustok-index`

This document is the normative integration guide for an in-repository/native
RusToK module that wants its domain data materialized and queried through
`rustok-index`.

The Index engine is domain-neutral. It must not depend on the source module,
read the source module's tables directly, or accept direct writes from the
source module into Index-owned tables. The source module or its distribution
bridge owns conversion from authoritative domain state into the generic Index
contracts.

## Important boundary: native modules versus standalone components

This guide applies to native Rust modules selected into the server distribution.
A standalone `wasm32-wasip2` component produced by `rustok-module-template`
cannot link `rustok-index`, register runtime extensions, access PostgreSQL, or
claim Index compatibility merely by publishing `platform.events` messages.

A standalone component requires a separately admitted host-owned broker
capability or event-to-Index bridge. No `platform.index` capability exists in
the current component ABI. Until such a boundary is implemented and documented,
standalone modules must treat Index integration as unavailable rather than
inventing a private payload contract.

## Compatibility levels

A module should state the highest level it implements.

| Level | Required contribution | Available behavior |
| --- | --- | --- |
| Schema | Versioned `IndexSchema` registration | Query planning and validation after data exists |
| Replay | Schema plus bounded `IndexSource::scan` and `load` | Initial build, rebuild, targeted reload |
| Incremental | Replay plus stable `IndexMutation` delivery | Ongoing updates without full rebuild |
| Drift-aware | Incremental plus authoritative absence/version evidence | Diagnosis and reconciliation |
| Repair-ready | Drift-aware plus admitted owner semantics and retained evidence | Targeted repair where supported |

Registering only a schema does not populate data. Implementing replay without
incremental delivery means the Index can be rebuilt but may become stale between
rebuilds.

## Recommended source-module layout

Keep Index-specific conversion at the module boundary instead of mixing it into
the domain model.

```text
src/
  domain/
  application/
  infrastructure/
  index/
    mod.rs
    schema.rs
    record.rs
    source.rs
    events.rs
    absence.rs        # only when authoritative absence is supported

tests/
  index_contract.rs
  index_postgres.rs   # environment-gated when PostgreSQL evidence is required
```

A distribution-owned bridge is also valid when the domain crate must not depend
on `rustok-index`. The existing Product integration follows that model: the
bridge registers schemas and PostgreSQL source factories only when Product is
selected.

## 1. Publish a stable schema

Every indexed entity requires a versioned `IndexSchema`. The complete schema
identity consists of its module namespace, entity name, and schema version.

```rust
use rustok_index::{
    EntityName, FieldCardinality, FieldName, IndexField, IndexSchema,
    IndexValueType, LocaleMode, ModuleName, SchemaRef, SchemaVersion,
};

fn listing_schema_v1() -> Result<IndexSchema, rustok_index::DomainError> {
    let schema = IndexSchema {
        reference: SchemaRef {
            module: ModuleName::new("classifieds")?,
            entity: EntityName::new("listing")?,
            version: SchemaVersion::new(1),
        },
        locale_mode: LocaleMode::None,
        fields: vec![
            IndexField {
                name: FieldName::new("id")?,
                value_type: IndexValueType::Uuid,
                cardinality: FieldCardinality::One,
                nullable: false,
                selectable: true,
                filterable: true,
                sortable: true,
            },
            IndexField {
                name: FieldName::new("title")?,
                value_type: IndexValueType::String,
                cardinality: FieldCardinality::One,
                nullable: false,
                selectable: true,
                filterable: true,
                sortable: true,
            },
            IndexField {
                name: FieldName::new("price_minor")?,
                value_type: IndexValueType::Integer,
                cardinality: FieldCardinality::One,
                nullable: false,
                selectable: true,
                filterable: true,
                sortable: true,
            },
        ],
        links: Vec::new(),
    };
    schema.validate()?;
    Ok(schema)
}
```

Register schemas during module/distribution composition:

```rust
use rustok_core::ModuleRuntimeExtensions;
use rustok_index::register_index_schema_source;

pub fn register_index_contracts(
    extensions: &mut ModuleRuntimeExtensions,
) -> rustok_core::Result<()> {
    register_index_schema_source(
        extensions,
        "classifieds",
        listing_schema_v1().map_err(|error| {
            rustok_core::Error::Validation(format!(
                "classifieds Index schema is invalid: {error}"
            ))
        })?,
    )
    .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "classifieds Index schema registration failed: {error}"
        ))
    })?;
    Ok(())
}
```

### Schema rules

A compatible module MUST:

- use a stable lowercase owner slug;
- keep one owner for every version of one schema identity;
- create a new schema version for semantic field/link changes;
- validate the schema before or during registration;
- mark only genuinely supported fields as selectable, filterable, or sortable;
- use a locale mode that matches the authoritative identity model;
- keep field and link types stable within a schema version.

A compatible module MUST NOT:

- reuse one schema version for a changed contract;
- split versions of one entity between different owners;
- register a link to a schema that is absent from the selected distribution;
- expose confidential fields merely because the storage model can hold them.

## 2. Convert authoritative state into generic records

Keep conversion deterministic and side-effect free. Given the same authoritative
row and schema version, conversion must produce the same key, fields, links, and
source version.

```rust
fn listing_record(row: &ListingRow) -> Result<rustok_index::IndexRecord, ListingIndexError> {
    // Convert the domain row into the generic Index envelope here.
    // Do not query Index storage or mutate domain state from this function.
    todo!()
}
```

The record identity MUST include the exact tenant, schema, entity identity, and
locale identity required by the schema. Links MUST represent the complete link
set for that source version because a normal upsert replaces the materialized
state for the entity.

## 3. Implement bounded replay and targeted load

`IndexSource` is the database-neutral source-owner boundary.

```rust
use async_trait::async_trait;
use rustok_index::{
    IndexSource, IndexSourceFailure, IndexSourceLoadBatch,
    IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
};

struct ClassifiedsIndexSource {
    // Usually an owner database/repository handle.
}

#[async_trait]
impl IndexSource for ClassifiedsIndexSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        // Use a bounded deterministic keyset scan owned by this module.
        // The returned cursor is opaque to Index but must advance.
        todo!()
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        // Load only the exact requested keys from the authoritative source.
        todo!()
    }
}
```

The engine validates the generic boundary, but the source remains responsible
for authoritative semantics.

### `scan` requirements

`scan` MUST:

- remain within the requested tenant and exact schema;
- return no more than the requested limit;
- use deterministic keyset ordering;
- return unique entity keys;
- return a non-null bounded cursor only when another page exists;
- advance the cursor whenever a continuation is returned;
- return no continuation for an empty final page;
- classify transient failures as retryable and invariant/data failures as
  permanent.

`scan` MUST NOT use an unbounded offset walk, return another tenant, or expose
raw credentials in its cursor.

### `load` requirements

`load` MUST:

- load only the exact requested keys;
- return no duplicate key;
- return no more mutations than requested keys;
- preserve the request tenant/schema scope;
- distinguish a current entity, a retained deletion/tombstone, and an unknown or
  not-authoritatively-proven absence according to the source contract.

A missing SQL row by itself is not automatically an authoritative deletion
proof.

## 4. Register the source or PostgreSQL source factory

For an already materialized source object:

```rust
rustok_index::register_index_source(
    extensions,
    "classifieds",
    "classifieds-listing-primary",
    [listing_schema_ref_v1()],
    ClassifiedsIndexSource::new(repository),
)?;
```

When the database connection exists only during server composition, register a
`PostgresIndexSourceFactory` instead. The factory should create the concrete
source from the host-owned connection and then call the same generic source
registration boundary.

Owner slug, source name, and schema ownership must match exactly. One schema can
have only one replay source, and all versions of one schema identity must remain
with the same owner/source pair.

## 5. Publish incremental mutations

Replay is not a replacement for ongoing ingestion. Every committed domain
change that affects indexed state should produce one generic mutation delivery
through the admitted event/outbox path.

A mutation delivery requires:

- a stable delivery/event UUID;
- the exact tenant and schema reference;
- the exact entity key and locale identity;
- a monotonic source version;
- a complete record for upsert or an explicit delete/tombstone;
- complete links for the admitted version;
- commit-before-ack behavior.

The same logical event MUST reuse the same delivery UUID on retry. A delivery
UUID MUST NOT be reused for a different payload. The Index inbox treats exact
redelivery as idempotent and rejects identity reuse with changed content.

### Source version

Use an authoritative version that advances whenever indexed state changes. It
must not be derived from wall-clock time unless the source can prove strict
monotonicity for the complete entity identity.

Good sources include:

- a transactionally incremented revision;
- an append-only sequence position;
- a source-owned durable version column.

Do not use random numbers or process-local counters.

### Deletes

Deletion MUST be retained as an explicit mutation/tombstone with a source
version newer than the last live version. Hard-deleting the source row without
retained version evidence prevents safe reconciliation and repair.

## 6. Cross-module links

Index schemas may link to schemas owned by other selected modules.

```text
classifieds.listing.seller
    -> identity.user.id
```

A cross-module link requires:

- the target schema to be registered in the same materialized catalog;
- compatible source and target field types;
- stable target identity;
- defined behavior when the target module is not selected;
- defined behavior when the target entity is deleted;
- complete ordered values when cardinality is `many`.

The source module owns the link contribution. The target module does not write
into the source module's Index record.

## 7. Absence, diagnosis, and repair readiness

Drift-aware integration requires more than replay.

The source owner must be able to prove:

- the exact current source version when an entity exists;
- an explicit absence/tombstone watermark when it is authoritatively deleted;
- stable link identity and target authority for orphan-link diagnosis;
- that evidence was read within the documented consistency boundary.

Do not advertise repair readiness until the concrete owner path and PostgreSQL
evidence have been admitted. Generic registration alone does not make a module
repair-ready.

## 8. Distribution composition

A typical bridge should register only when its source module is selected.

```rust
pub(crate) fn register(
    extensions: &mut rustok_core::ModuleRuntimeExtensions,
) -> rustok_core::Result<()> {
    if !extensions.contains::<ClassifiedsRuntimeSelected>() {
        return Ok(());
    }

    register_listing_schemas(extensions)?;
    register_listing_source_factory(extensions)?;
    register_listing_absence_provider(extensions)?; // optional level
    Ok(())
}
```

The server materializes and freezes the complete schema/source catalogs after
all selected modules have contributed. Registration is therefore a startup
composition contract, not hot-plug discovery.

## 9. Required tests

At minimum, add contract tests for:

- schema validation and stable fingerprint;
- owner/source conflict rejection;
- deterministic record conversion;
- bounded scan page size and cursor progression;
- targeted load scope and duplicate rejection;
- monotonic source versions;
- exact redelivery idempotency;
- delete/tombstone retention;
- link type/cardinality correctness;
- selection gating in distribution composition.

For production PostgreSQL adapters, add environment-gated evidence for real
migrations, replay, redelivery, deletion, restart, and any claimed drift/repair
behavior. Never replace production persistence with a fake store while claiming
PostgreSQL admission.

## 10. Compatibility checklist

### Required for schema compatibility

- [ ] Stable owner slug
- [ ] Versioned validated `IndexSchema`
- [ ] Correct locale mode
- [ ] Supported field capabilities only
- [ ] Cross-module targets present and type-compatible

### Required for replay compatibility

- [ ] Bounded deterministic `scan`
- [ ] Exact bounded `load`
- [ ] Stable opaque cursor
- [ ] Retryable/permanent failure classification
- [ ] Source registration or PostgreSQL source factory

### Required for incremental compatibility

- [ ] Stable delivery UUID
- [ ] Monotonic source version
- [ ] Complete upsert record and links
- [ ] Explicit delete/tombstone
- [ ] Commit-before-ack orchestration

### Required before claiming drift or repair readiness

- [ ] Authoritative absence/version evidence
- [ ] Retained PostgreSQL execution evidence
- [ ] Crash/redelivery behavior admitted
- [ ] Owner authorization and recovery policy admitted
- [ ] No direct public or automatic repair surface beyond the admitted boundary

## Prohibited shortcuts

A module is not Index-compatible if it:

- writes directly to `index_entities`, `index_links`, `index_inbox`, jobs, or
  findings tables;
- asks `rustok-index` to query source-domain tables directly;
- emits unstable event IDs or non-monotonic source versions;
- treats a missing row as proven deletion without an owner contract;
- registers different owners/sources for versions of one schema identity;
- claims standalone component compatibility through `platform.events` without
  an admitted host bridge;
- bypasses schema validation, tenant scope, or authorization.
