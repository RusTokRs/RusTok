from pathlib import Path


def replace_span(text: str, start_marker: str, end_marker: str, replacement: str, label: str) -> str:
    start = text.find(start_marker)
    if start < 0:
        raise SystemExit(f"{label}: start marker not found")
    end = text.find(end_marker, start)
    if end < 0:
        raise SystemExit(f"{label}: end marker not found")
    end += len(end_marker)
    return text[:start] + replacement + text[end:]


def replace_paragraph(text: str, prefix: str, replacement: str, label: str) -> str:
    start = text.find(prefix)
    if start < 0:
        raise SystemExit(f"{label}: paragraph prefix not found")
    end = text.find("\n\n", start)
    if end < 0:
        end = len(text)
    return text[:start] + replacement + text[end:]


registry_path = Path("docs/modules/registry.md")
registry = registry_path.read_text(encoding="utf-8")
registry = replace_span(
    registry,
    "The former Product-to-Index projection path was removed with Index v1.",
    "without importing product internals.",
    "The former Product-to-Index projection path, virtual-category materialization, and Search consumption of Index-owned facets/sort keys were removed with Index v1. Product and Search must not depend on an Index projection again until the generic Index Engine reaches its first vertical slice and publishes replacement owner contracts.",
    "stale Product/Search Index v1 projection claims",
)
registry = registry.replace(
    "The former Index v1 registry, fallback evidence, and `npm run verify:index:fba` contribution were removed completely.",
    "The former Index v1 registry, fallback evidence, and verifier contribution were removed completely.",
)
for forbidden in (
    "crates/rustok-index/contracts/index-fba-registry.json",
    "crates/rustok-index/contracts/evidence/index-runtime-fallback-smoke.json",
    "reads channel-scoped normalized attribute facets/sort keys from the index projection",
):
    if forbidden in registry:
        raise SystemExit(f"central registry still contains removed Index v1 claim: {forbidden}")
registry_path.write_text(registry, encoding="utf-8")

overview_path = Path("docs/research/fluid-backend-architecture-unified-plan.md")
overview = overview_path.read_text(encoding="utf-8")
overview = replace_paragraph(
    overview,
    "As of 2026-06-20 `index` added as a provider track",
    "On 2026-06-20 Index v1 introduced a read-model/rebuild provider track. The destructive Index Engine rewrite removed that track completely: ports, contract identifiers, registry, evidence, verifier integration, projections, and runtime wiring no longer exist. Replacement Index FBA readiness is `in_progress` until new query/rebuild contracts and live provider-consumer evidence are published.",
    "historical Index v1 provider-track paragraph",
)
overview = overview.replace(
    "`SearchQueryPort` and `SearchSuggestionPort`. The former Index v1 document-ingestion/read-model boundary was removed",
    "`SearchQueryPort` and `SearchSuggestionPort`.\n\nThe former Index v1 document-ingestion/read-model boundary was removed",
)
overview = overview.replace(
    "4. Remove Search query-time SQL access to the removed Index v1 projection tables. Populate the needed category and facet fields in Search-owned projections during ingestion. Do not depend on removed Index v1 ports; reconnect only through replacement Index Engine contracts after the first vertical slice.",
    "4. Remove Search query-time SQL access to the removed Index v1 projection\n   tables. Populate the needed category and facet fields in Search-owned\n   projections during ingestion. Do not depend on removed Index v1 ports;\n   reconnect only through replacement Index Engine contracts after the first\n   vertical slice.",
)
for forbidden in (
    "IndexReadModelPort",
    "IndexRebuildPort",
    "index.read_model.v1",
    "index.rebuild.v1",
    "crates/rustok-index/contracts/index-fba-registry.json",
    "crates/rustok-index/contracts/evidence/index-contract-test-static-matrix.json",
    "npm run verify:index:fba",
):
    if forbidden in overview:
        raise SystemExit(f"FBA overview still contains active Index v1 reference: {forbidden}")
overview_path.write_text(overview, encoding="utf-8")
