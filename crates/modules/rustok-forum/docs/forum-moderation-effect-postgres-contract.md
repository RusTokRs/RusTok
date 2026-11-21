# forum moderation effect postgres contract

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-moderation-effect-postgres-contract.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum moderation effect postgres contract`.
- Verifier: `scripts/verify/verify-forum-moderation-effect-postgres-contract.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- RejectPublication
- Removed accepted solution
- Unpublished remains distinct
- status=deleted
- failed owner-operation receipt
- fail closed instead of being silently remapped
- cargo test -p rustok-forum --test moderation_effect_contract_postgres
