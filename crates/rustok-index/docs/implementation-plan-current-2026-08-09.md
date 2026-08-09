# Current `rustok-index` implementation plan — 2026-08-09

Status: `m5_product_refresh_family_source_ready_digest_regeneration_pending`.

This overlay supersedes `implementation-plan-current-2026-08-08.md` as the live execution cursor. The older file
remains detailed architecture/source history.

Rechecked against `main@96c8886738c3df22e176c808fd04d27d8eedb552`. The only mainline change after the
canonical event-digest admission merge is Page Builder/Telemetry work and does not overlap Product, Events or
Index refresh wire paths.

## M5 — Product Index typed refresh family

The stale event-contract baseline gate is complete.

Maintainer execution on admitted `main@7983092f96e14c002c57451709de936e40c01356` reported:

- `verify-event-contract-digest-admission.mjs` passed;
- the canonical `event_contract_digests -- --write` generator passed;
- `crates/rustok-events/contracts/event-contract-digests.json` had an empty diff.

No GitHub Actions verification packet is claimed; this is the maintainer-provided exact-SHA local canonical
execution result.

The newly unblocked source slice is `ProductIndexRefreshEvent`. The family is source-ready on the current review
branch with two exact event types:

```text
product.index.locale_refresh_requested
product.index.variant_refresh_requested
```

Payload ownership remains narrow:

- locale: `product_id`, `locale`, `source_version`;
- variant: `product_id`, `variant_id`, `source_version`.

The existing Product ledger/canonical writer still owns envelope identity, tenant, actor and causation:

```text
id = correlation_id = refresh_id
causation_id = root_event_id
```

`CanonicalProductIndexRefreshEventFactory` binds immutable Product locale/ProductVariant ledger rows to this
sealed family. No alternate JSON/event-name route or compatibility family is added.

### M5 merge gate

Adding this family changes the complete typed contract schema. Before this source PR may merge, the maintainer
must run the repository generator on the exact PR head and the resulting
`crates/rustok-events/contracts/event-contract-digests.json` must be committed in the same reviewed PR.

After that admission, the next independent M5 source boundary is Product/ProductVariant typed delivery into the
existing generic `IndexSourceRefreshEventWorker`: exact event-route registration, canonical target-key decoding
and commit-before-ack consumption. That later slice must not start before this family/digest PR is admitted.

## M6 — concrete repair PostgreSQL evidence

M6 source remains complete and execution/admission-gated. The latest maintainer attempt stopped before any
PostgreSQL work because neither `RUSTOK_INDEX_TEST_DATABASE_URL` nor `DATABASE_URL` was present. No evidence packet
or logs were created, so this is an environment/configuration blocker, not a source defect.

The required next M6 action remains an exact rerun with a real opt-in PostgreSQL URL, followed by the retained
evidence/verifier/Cargo command set in `m6-repair-retained-evidence-admission.md`.

Do not add another M6 source slice unless that execution exposes a concrete source failure.

## M7 — Product Storefront

M7 remains evidence/admission-gated. Mounted Storefront stays Product owner-native. Existing timeout, Product
key-4 promotion/restart, current-key core/EAV/collation, deployment collation, stale/readiness/restart evidence
must be executed/admitted before any serving traffic composition changes.

## Partition replay

Partition replay remains blocked until a real source contract filters the requested partition before pagination.
Do not populate `partition_key` without that source capability.

## Compatibility rule

Repository-owned pre-release contracts have one current shape. Do not introduce legacy readers, v2 families,
fallback decoders, dual formats or compatibility publication paths unless an explicit external compatibility
bridge is approved.

## Validation boundary

The implementation agent performed static GitHub source/diff review only for the Product refresh family slice.
The new event family changes the canonical digest by definition, but its digest values have not been guessed or
hand-authored. Rust tests, Node verifiers, Cargo checks, formatting, PostgreSQL execution, workflows and CI are
not claimed on the review branch.
