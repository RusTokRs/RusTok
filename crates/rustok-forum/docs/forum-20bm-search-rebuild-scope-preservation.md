# FORUM-20BM Search rebuild scope preservation

`FORUM-20BM` closes the destructive failure mode that remained after Forum
Search projection composition: a full tenant rebuild used to commit a direct
Search projector transaction that deleted every `search_documents` row for the
tenant before Blog and Forum rebuilt their own scopes. A later Blog or Forum
failure could therefore leave that source empty even though its previous
projection had been valid.

The canonical Forum roadmap remains
[`implementation-plan.md`](./implementation-plan.md). This note records the
stable owner boundary delivered by this slice; it is not a second roadmap.

## Active projector boundary

`crates/rustok-search/src/projector.rs` is now the public `SearchProjector`
facade. It owns only the direct `node` and `product` rebuild sequence and delegates
targeted operations to the previous implementation.

The previous implementation is retained byte-for-byte as
`projector_legacy.rs`, declared as a private crate module. The active facade does
not call its destructive `rebuild_tenant` method. It calls only the existing
transactional `rebuild_content_scope` and `rebuild_product_scope` methods.
Keeping the previous implementation private avoids a large conflict-prone source
rewrite while preserving every public `SearchProjector` type and method name.

The bootstrap presence query is also scoped to `node` and `product`. Existing
Blog or Forum documents no longer suppress bootstrap of the direct Search
scopes.

## Sequential replacement semantics

The ingestion order remains:

1. direct Search `node` scope;
2. direct Search `product` scope;
3. Blog scope;
4. Forum scope when the Forum projection source is registered.

This is source-scoped preservation, not a new cross-source transaction.

- content and product retain their existing per-scope transactions;
- Blog continues to delete and repopulate only its scope in one transaction;
- Forum continues to scan into its temporary stage and replace only its scope in
  one transaction;
- the active core projector never deletes Blog, Forum, or an unknown future
  external projection scope.

If Blog fails, its transaction rolls back and its previous Blog documents remain.
If Forum fails, its staging transaction rolls back and its previous Forum
documents remain. A source that completed successfully before a later source
failed may already expose its newer projection. Global all-source atomicity is
therefore still not claimed.

This model is retry-safe because every source rebuild derives current owner state
and replaces only its own idempotent storage scope.

## Compatibility

No Search query, event, Forum source, REST, GraphQL, response DTO, migration,
workspace dependency, or `Cargo.lock` contract changes in this slice. The public
`rustok_search::SearchProjector` path and method names remain unchanged.

The legacy all-tenant implementation is deliberately private and quarantined for
delegation of targeted/per-scope methods. It is not exported and is not reachable
from Search ingestion.

## Verification and remaining evidence

Source contract coverage lives in:

- `crates/rustok-search/tests/search_scope_preservation_contract.rs`;
- `scripts/verify/verify-forum-search-rebuild-scope-preservation.mjs`;
- `crates/rustok-forum/contracts/forum-search-rebuild-scope-preservation.json`.

The implementation agent did not run tests, Cargo commands, formatting,
verifiers, workflows, or CI. Maintainer-executed PostgreSQL evidence should still
exercise a full rebuild where a later Forum source fails and confirm that the
previous Forum rows remain queryable.

Large Search/Forum plan and `CRATE_API.md` files remain conflict-sensitive
repository-local synchronization debt. The machine contract records the exact
handoff to `FORUM-20BN`.
