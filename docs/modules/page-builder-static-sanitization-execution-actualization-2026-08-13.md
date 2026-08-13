# Page Builder static sanitization execution evidence actualization — 2026-08-13

Status: `static-sanitization-execution-source-ready / maintainer-execution-pending / registry-update-pending / terminal-inventory-incomplete`.

## Fresh source recheck

This slice was prepared from `main@6c879d493c2a111a9288c4223f725a5b4aefc6e5` after PR #3492 defined the source-derived terminal evidence inventory. The two commits after #3492 are Blog/Taxonomy-only and do not overlap the Page Builder evidence paths.

The current terminal inventory still contains **12** recursive Page Builder FBA `executed_evidence: "pending"` blocker nodes. This slice targets exactly one of them:

```text
/provider/static_sanitization_contract/executed_evidence
```

The FBA registry remains `boundary_ready` and the value at that pointer remains `pending`.

## Why this blocker is next

Static publish sanitization is already implemented and source-guarded. The existing boundary includes:

- canonical `page_builder_static_publish_sanitization_v2` identity;
- fail-closed static publish policy validation;
- deterministic policy-bound sanitization hashing;
- secure public-resource rejection;
- global static publish resource limits;
- integrity revalidation of both policy and resource limits;
- focused sanitizer tests for deterministic stable ids/hash binding, excess global resources, insecure public resources, and renderer-dropped attributes/CSS;
- dedicated policy and resource-limit unit test modules;
- the existing `verify-page-builder-static-publish-resource-limits.mjs` anti-drift verifier.

What is missing for the FBA registry node is retained **executed evidence**, not another sanitizer implementation.

## New execution source

This continuation adds:

- `crates/rustok-page-builder/contracts/evidence/page-builder-static-sanitization-execution-source.json`;
- `scripts/evidence/record-page-builder-static-sanitization-execution.mjs`;
- `scripts/verify/verify-page-builder-static-sanitization-execution.mjs`;
- `.github/workflows/page-builder-static-sanitization-evidence.yml`.

The workflow first verifies the existing resource-limit boundary and the new execution source contract. It then inventories the Rust test list and refuses to continue unless all expected sanitizer tests plus policy/resource-limit test modules are present.

The execution job runs only the Page Builder core library tests relevant to this boundary:

```text
cargo test --locked -p rustok-page-builder --lib publish_sanitization::tests:: -- --nocapture
cargo test --locked -p rustok-page-builder --lib static_publish_policy::tests:: -- --nocapture
cargo test --locked -p rustok-page-builder --lib static_publish_resource_limits::tests:: -- --nocapture
```

No database, browser, HTTP server or tenant control plane is required for this blocker.

## Retained receipt

Only after all test steps succeed does the workflow invoke:

```text
node scripts/evidence/record-page-builder-static-sanitization-execution.mjs
```

The recorder requires `GITHUB_ACTIONS=true`, exact `GITHUB_SHA == checkout HEAD`, the FBA registry to remain `boundary_ready`, and the target pointer to remain `pending`.

A successful receipt has:

```text
format = page_builder_static_sanitization_execution_v1
status = static_sanitization_execution_passed_registry_update_pending
```

It retains:

- exact source commit;
- GitHub workflow/run/attempt/event identity;
- exact target JSON Pointer and pre-execution registry hash/value;
- exact test command set;
- hashes of all required source files.

It does **not** embed raw test logs, tenant identity, credentials, cookies, GraphQL payloads or browser data. Logs and the bounded receipt are archived together as a 90-day workflow artifact.

The packet deliberately records `cryptographic_ci_attestation_claimed=false`: a JSON file generated in Actions is useful retained provenance, but this source slice does not invent a cryptographic GitHub-run attestation scheme.

## Governance boundary

A successful execution receipt is still not the registry update itself. The workflow cannot edit `page-builder-fba-registry.json`, cannot set `transport_verified`, cannot complete the terminal inventory, and cannot claim owner/platform approval. This is not `transport_verified`.

The registry remains `pending` in this PR. A later evidence-containing PR must bind the exact successful receipt and exact source commit before changing this blocker node. All other Page Builder FBA blockers and the independent Pages `execution-rollout-pending` marker remain untouched.

## Validation boundary

No manual tests, Node verifiers, Cargo commands, workflow reruns, browsers, databases or live mutations were executed by this source slice. The workflow and verifiers are source-ready for maintainer/user execution.

## Next cursor

1. execute `Page Builder Static Sanitization Evidence` on the exact intended source;
2. retain the successful workflow artifact and `page_builder_static_sanitization_execution_v1` receipt;
3. verify receipt/source/run lineage;
4. only then change `/provider/static_sanitization_contract/executed_evidence` in an evidence-containing PR;
5. rerun the terminal inventory, which should reduce the Page Builder FBA blocker count by exactly one if no other source changed;
6. continue with the next canonical blocker rather than claiming global `transport_verified`.
