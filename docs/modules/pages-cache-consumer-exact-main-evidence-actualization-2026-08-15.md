# Pages / Page Builder cache-consumer exact-main evidence actualization — 2026-08-15, revalidated 2026-08-16

Status: `source-ready / exact-main-execution-pending / registry-admission-separate`.

## Fresh boundary

This replacement execution slice starts from fresh `main@db29e7e27f9f4a438774b5d0e7f10a35ec34bb62` and supersedes the stale working branches behind PRs #3604 and #3605 without rebasing either branch.

The immediately preceding canonical terminal inventory still has two recursive pending evidence nodes:

1. `/provider/consumer_properties_contract/executed_evidence`;
2. `/consumers/0/cache_consumer/executed_evidence`.

A fresh-base audit compared the #3604 merge base `bfbe9cd9a92dcf8224e9a7d7068a86f0506a7a12` with `main@db29e7e...`. None of the ten Pages test/verifier files carried by the old evidence packet changed on `main` across that interval, so their still-needed fixture and source-guard corrections can be transplanted without overwriting concurrent work. The temporary diagnostic workflow from #3604 is intentionally not retained.

The canonical target remains `pending`. Provider consumer-properties remains independently `pending` and is not inferred by this packet.

## Canonical cache-consumer scope

The retained packet exercises the existing Pages cache-consumer boundary: generation rotation, production `ServerPagesCachePort` composition, durable outbox delivery, generation-aware native storefront and artifact HTTP reads, explicit repair cache behavior, and fail-open reads back to authoritative owner source.

No second cache policy or production runtime behavior is introduced by this evidence slice.

## Source guard repairs

The old packet exposed source-guard drift rather than a new runtime architecture gap. In particular the current parity plan records:

- `production-relay-generation-gate-source-ready` — synchronous generation gate source-ready;
- `production-relay-native-route-source-ready` — gate-to-native-route composition source-ready;
- `production-gate-postgres-restart-source-ready` — PostgreSQL retry source-ready.

The retained verifier updates bind to those current markers and retain the existing production/test ordering assertions.

## Execution packet

The workflow executes these existing source guards:

- `verify-pages-cache-invalidation.mjs`;
- `verify-pages-publish-rollback-outbox-cache-postgres.mjs`;
- `verify-pages-outbox-relay-restart-postgres.mjs`;
- `verify-pages-production-relay-generation-gate.mjs`;
- `verify-pages-production-relay-native-route.mjs`;
- `verify-pages-native-storefront-cache.mjs`;
- `verify-pages-native-storefront-relay-continuity.mjs`;
- `verify-pages-artifact-http-cache.mjs`;
- `verify-pages-explicit-artifact-repair-cache.mjs`.

The runtime packet executes PostgreSQL publish/rollback and relay-restart cache cases, SQLite/Axum artifact HTTP cache behavior, native storefront cache behavior, explicit repair regression, native storefront relay continuity, the production relay-to-native-route integration, production cache-provider/generation-gate unit packets, and locked Cargo checks for Pages, Pages storefront SSR and server `mod-pages` targets.

## Evidence semantics

A successful PR run validates only the exact PR head. It does not mutate the FBA registry and does not create an exact-main receipt.

After squash merge, the same workflow runs on exact `push/main`. Only a successful exact-main preflight may create the bounded receipt containing the exact source SHA, workflow provenance, target/sibling registry pre-state, execution facts and SHA-256 hashes of the required source files.

`push/main.paths` deliberately excludes the canonical registry. Therefore a later registry-only admission cannot create a misleading post-admission receipt that still expects the target pre-state to be `pending`.

## Non-claims

This slice does not:

- set `/consumers/0/cache_consumer/executed_evidence` to `verified`;
- verify provider consumer-properties;
- remove Pages `execution-rollout-pending`;
- recompute the terminal inventory;
- claim owner/platform readiness;
- promote Pages FFA or Page Builder FBA.

## Next cursor

Only after a successful retained exact-main receipt for the merged source, create a separate registry admission PR for `/consumers/0/cache_consumer/executed_evidence: pending -> verified`. Then re-read the canonical registry and, if the provider node is the only remaining blocker, recompute terminal inventory `2 -> 1` in another PR.
