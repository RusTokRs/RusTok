from pathlib import Path

path = Path("docs/verification/PLATFORM_HARDENING_IMPLEMENTATION_PLAN.md")
source = path.read_text()


def replace_once(old: str, new: str, label: str) -> None:
    global source
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, got {count}")
    source = source.replace(old, new, 1)

replace_once(
    "The plan was initially revalidated against `main` on 2026-07-17 at commit `9c3a5f1b443d7fc0fa1dae8ee9b09a29d2edfb67`. The progress ledger was refreshed on 2026-07-17 after the security and tenant hardening batches through commit `5cbab58823b8cf1edb3698b7b549ddaa5645cc90`.",
    "The plan was initially revalidated against `main` on 2026-07-17 at commit `9c3a5f1b443d7fc0fa1dae8ee9b09a29d2edfb67`. The progress ledger was refreshed on 2026-07-17 after the canonical tenant-resolution and transport-unification work through commit `cead00ec16522257b7b3d0689aaf14238a160558`.",
    "ledger revision",
)
replace_once(
    "6. The private legacy resolver still contains an `unwrap_or_default()` timestamp helper. Public tenant-bound requests and durable generation publication now fail closed on a pre-epoch clock, but the private helper should be removed during resolver decomposition.\n",
    "",
    "close legacy clock finding",
)
replace_once(
    "3. Unknown tenant resolution modes now fail executable bootstrap and the public request-time tenant boundary.\n4. `DefaultTenant` fallback is forbidden in production, rejected outside header mode and emits telemetry plus a warning whenever it is used on a tenant-bound development request.\n5. Operator routes and the global read-only registry catalog are excluded from fallback telemetry and tenant clock checks.",
    "3. Tenant resolution is a typed enum with an exhaustive canonical resolver; unknown modes fail configuration deserialization and cannot reach a default-tenant catch-all.\n4. `DefaultTenant` fallback is forbidden in production, rejected outside header mode and emits telemetry plus a warning only when it is actually selected.\n5. HTTP and GraphQL WebSocket use one cache-aware tenant read-port loader with typed errors; transport code no longer queries tenant persistence or reconstructs `TenantContext` independently.\n6. Operator routes, self-resolving handshakes and the global read-only registry catalog are represented by one segment-safe route policy rather than duplicated bypass lists.",
    "closed tenant findings",
)
replace_once(
    "5. Complete `HARD-109` by removing the private legacy timestamp fallback and adding clock-skew cache tests.\n6. Complete `HARD-105` with an explicitly named development/single-tenant profile contract.",
    "5. Add required negative tenant-isolation integration coverage for malformed and conflicting assertions across all transports.\n6. Complete `HARD-105` with an explicitly named development/single-tenant profile contract.",
    "ordered backlog tenant item",
)
replace_once(
    "cargo test -p rustok-server middleware::tenant\n",
    "cargo test -p rustok-server middleware::tenant\ncargo test -p rustok-server --test tenant_resolver_invariants_test\nnode scripts/verify/verify-tenant-resolution-architecture.mjs\n",
    "tenant validation commands",
)
replace_once(
    "| `HARD-104` Tenant resolution fail-closed | Completed at public boundaries | Bootstrap `47c8003`; request-time boundary `ce315be`; legacy private cleanup remains |",
    "| `HARD-104` Tenant resolution fail-closed | Completed | Typed configuration and canonical resolver `adca4014`; route/header hardening `f3b475e0`; unified HTTP/WS loader `21ad3a99` |",
    "HARD-104 ledger",
)
replace_once(
    "| `HARD-109` Clock anomaly handling | Materially mitigated | Durable generation `07ed2ab`; tenant request guard `8965919`; private helper cleanup and skew tests remain |",
    "| `HARD-109` Clock anomaly handling | Implemented; runtime tests pending local execution | Durable generation `07ed2ab`; request/cache timestamps return errors; canonical loader `21ad3a99` |",
    "HARD-109 ledger",
)
replace_once(
    "| `HARD-110` Production JWT bootstrap policy | Implemented; rotation remains operational work | Bootstrap policy `ec5111b`; production example `c6cb4a3` |",
    "| Canonical tenant context loading | Completed | Shared HTTP/GraphQL WebSocket read-port pipeline `21ad3a99`; negative-cache degradation and WS source telemetry `cead00ec` |\n| `HARD-110` Production JWT bootstrap policy | Implemented; rotation remains operational work | Bootstrap policy `ec5111b`; production example `c6cb4a3` |",
    "canonical loader ledger row",
)

path.write_text(source)
