# `rustok-index` implementation recheck — 2026-08-01

## Audited baseline

- Repository: `RusTokRs/RusTok`
- Target branch: `main`
- Audited commit: `1cc032d6ceba0e55b5f3a1dfa2a64b066dc73f71`
- Validation owner: repository maintainer

This recheck compares the canonical implementation plan with the source and recent Index pull-request history. Tests, verifiers, Cargo commands, CI, and PostgreSQL execution were not run, per maintainer instruction.

## Confirmed on `main`

- Product v1/v2 bounded replay sources and stable Product event identities.
- ProductVariant v1/v2 bounded replay sources.
- Product v2 to ProductVariant v2 graph links and bounded graph projection fields.
- Product, translation, and ProductVariant retained hard-delete mutations.
- SalesChannel v1 bounded current-state replay source.
- Bounded multi-pass source reconciliation with durable pass/source cursor state.

The pre-existing canonical plan still described several of these delivered slices as open. The plan update in this branch corrects those entries without promoting owner-executed evidence.

## Confirmed missing on `main`

SalesChannel hard-delete identities were not retained. A physical Channel deletion could therefore disappear from current-state replay without producing a generic Index delete, and identity reuse had no retained-delete revision fence.

## Continued slice

This branch adds the missing bounded SalesChannel delete path:

- Channel-owned `channel_index_tombstones` storage;
- delete, identity-move, and identity-reuse revision fencing;
- replay of live rows and retained deletes through the existing `sales-channel-postgres-primary` source;
- fail-closed rejection when one identity exists as both live and tombstoned;
- synchronized Channel, Index, and verifier documentation.

The following contracts remain unchanged:

- `rustok-channel::sales_channel@1` schema fingerprint;
- replay source and factory name;
- stable `channel_id` cursor ordering;
- targeted-load key shape;
- replay event domain;
- Index core ownership boundaries.

## Remaining work

Incremental mutation-event acknowledgement, persisted per-tenant schema readiness, durable Product-to-SalesChannel relation revisions, tombstone purge admission, authoritative Storefront cutover, and retained PostgreSQL replay/reconciliation/delete-recreate evidence remain open.
