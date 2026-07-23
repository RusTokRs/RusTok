from pathlib import Path


def replace_matching_line(text: str, predicate, replacement: str, label: str) -> str:
    lines = text.splitlines(keepends=True)
    matches = [index for index, line in enumerate(lines) if predicate(line)]
    if len(matches) != 1:
        raise SystemExit(f"{label}: expected exactly one line, found {len(matches)}")
    index = matches[0]
    newline = "\n" if lines[index].endswith("\n") else ""
    lines[index] = replacement + newline
    return "".join(lines)


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
registry = registry_path.read_text()
registry = replace_matching_line(
    registry,
    lambda line: line.startswith("| `index` |") and "index-fba-registry.json" in line,
    "| `index` | admin | `in_progress` | `in_progress` | `core_transport_ui` | [Live plan](../../crates/rustok-index/docs/implementation-plan.md); Index v1 ports, registry, runtime evidence, projections, and read-model contracts were removed completely. The replacement is the generic cross-module relational Index Engine; FBA remains `in_progress` until new query/rebuild contracts and provider-consumer evidence are published. |",
    "central Index readiness row",
)
registry = replace_span(
    registry,
    "`rustok-index` transactionally updates",
    "without locale fallback.",
    "The former Product-to-Index projection path was removed with Index v1. Product and Search must not depend on an Index projection again until the generic Index Engine reaches its first vertical slice and publishes replacement owner contracts.",
    "historical Product-to-Index projection claim",
)
registry = replace_paragraph(
    registry,
    "Foundation FBA runtime-smoke batch evidence:",
    "Foundation FBA runtime-smoke batch evidence remains current for `channel`, `tenant`, and `email`. The former Index v1 registry, fallback evidence, and `npm run verify:index:fba` contribution were removed completely. Index is reset to `in_progress` until replacement query/rebuild contracts and new provider-consumer evidence exist.",
    "foundation FBA evidence paragraph",
)
for forbidden in (
    "crates/rustok-index/contracts/index-fba-registry.json",
    "crates/rustok-index/contracts/evidence/index-runtime-fallback-smoke.json",
    "Index remains the canonical ingestion/read-model owner",
):
    if forbidden in registry:
        raise SystemExit(f"central registry still contains removed Index v1 reference: {forbidden}")
registry_path.write_text(registry)

overview_path = Path("docs/research/fluid-backend-architecture-unified-plan.md")
overview = overview_path.read_text()
index_track = "| `index` | generic cross-module relational index and query engine | `in_progress` | Complete M2 scale evidence and storage selection, then publish replacement query/rebuild contracts. All Index v1 ports, registry, evidence, projections, and runtime wiring are removed. | `crates/rustok-index/docs/implementation-plan.md`, `DECISIONS/2026-07-23-index-engine-rewrite.md` |"
if index_track not in overview:
    lines = overview.splitlines(keepends=True)
    matches = [index for index, line in enumerate(lines) if line.startswith("| `search` |")]
    if len(matches) != 1:
        raise SystemExit(f"Index track insertion: expected one Search row, found {len(matches)}")
    index = matches[0]
    newline = "\n" if lines[index].endswith("\n") else ""
    lines.insert(index + 1, index_track + newline)
    overview = "".join(lines)
overview = replace_span(
    overview,
    "`rustok-index` owns canonical",
    "query-service pilot.",
    "The former Index v1 document-ingestion/read-model boundary was removed\ncompletely. The replacement `rustok-index` is a generic cross-module relational\nIndex Engine whose query/rebuild contracts remain `in_progress`; Search integration\nand any remote ingestion split are deferred until the replacement contracts have\ncompiled and live replay, lag, rebuild, and recovery evidence.",
    "Search/Index extraction description",
)
overview = replace_span(
    overview,
    "4. Remove Search query-time SQL access to `index_product_categories`",
    "`IndexReadModelPort` only for optional enrichment.",
    "4. Remove Search query-time SQL access to the removed Index v1 projection tables. Populate the needed category and facet fields in Search-owned projections during ingestion. Do not depend on removed Index v1 ports; reconnect only through replacement Index Engine contracts after the first vertical slice.",
    "Search extraction Index v1 port step",
)
overview = overview.replace("index.read_model.v1", "removed Index v1 read-model contract")
overview = overview.replace("index.rebuild.v1", "removed Index v1 rebuild contract")
for forbidden in (
    "index.read_model.v1",
    "index.rebuild.v1",
    "IndexReadModelPort",
    "owns canonical document ingestion and read models",
):
    if forbidden in overview:
        raise SystemExit(f"FBA overview still contains removed Index v1 reference: {forbidden}")
overview_path.write_text(overview)

plan_path = Path("crates/rustok-index/docs/implementation-plan.md")
plan = plan_path.read_text()
plan = replace_matching_line(
    plan,
    lambda line: line.startswith("- [ ] Synchronize the central module registry and historical FBA overview."),
    "- [x] Synchronize the central module registry and historical FBA overview.",
    "M0 checkbox",
)
plan = replace_paragraph(
    plan,
    "The code acceptance criterion is complete.",
    "M0 is complete. The central module registry and historical FBA overview now\nrecord the full removal of Index v1 and keep replacement FBA readiness at\n`in_progress` until new contracts and runtime evidence exist.",
    "M0 completion paragraph",
)
progress = (
    "- 2026-07-24: synchronized the central module registry and FBA overview with\n"
    "  complete Index v1 removal, removed references to deleted registry/evidence\n"
    "  and read-model contracts, and reset central FBA readiness to `in_progress`.\n"
)
if progress not in plan:
    if not plan.endswith("\n"):
        plan += "\n"
    plan += progress
plan_path.write_text(plan)
