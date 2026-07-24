from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new, 1)


sql_paths = [
    Path("ops/benches/src/index_storage/sql/jsonb.rs"),
    Path("ops/benches/src/index_storage/sql/eav.rs"),
    Path("ops/benches/src/index_storage/sql/hot.rs"),
]
for path in sql_paths:
    text = path.read_text(encoding="utf-8")
    text = replace_once(
        text,
        "product_variant.link_name = 'variants' JOIN",
        "product_variant.link_name = 'variants' AND product_variant.target_entity = 'variant' JOIN",
        f"{path}: Product-to-Variant target type",
    )
    text = replace_once(
        text,
        "variant_channel.link_name = 'sales_channels' JOIN",
        "variant_channel.link_name = 'sales_channels' AND variant_channel.target_entity = 'sales_channel' JOIN",
        f"{path}: Variant-to-Channel target type",
    )
    path.write_text(text, encoding="utf-8")

verifier_path = Path("scripts/verify/verify-index-fba.mjs")
verifier = verifier_path.read_text(encoding="utf-8")
verifier = replace_once(
    verifier,
    "  'two_hop_channel_filter',\n  'keyset_page',",
    "  'two_hop_channel_filter',\n  \"product_variant.target_entity = 'variant'\",\n  \"variant_channel.target_entity = 'sales_channel'\",\n  'keyset_page',",
    "verifier typed-link markers",
)
verifier_path.write_text(verifier, encoding="utf-8")

plan_path = Path("crates/rustok-index/docs/implementation-plan.md")
plan = plan_path.read_text(encoding="utf-8")
progress = (
    "- 2026-07-24: inspected the 100k `two_hop_channel_filter` EXPLAIN tree. The\n"
    "  Product-to-Variant reverse lookup performed about 1.64 million shared-hit\n"
    "  buffer accesses because both link hops omitted their known `target_entity`\n"
    "  discriminators. Added `variant` and `sales_channel` target predicates to all\n"
    "  three candidate queries and locked them in `verify-index-fba.mjs`; the prior\n"
    "  two-hop latency is retained only as pre-fix diagnostic evidence.\n"
)
if progress not in plan:
    if not plan.endswith("\n"):
        plan += "\n"
    plan += progress
plan_path.write_text(plan, encoding="utf-8")

benchmark_path = Path("crates/rustok-index/docs/storage-benchmark.md")
benchmark = benchmark_path.read_text(encoding="utf-8")
benchmark = replace_once(
    benchmark,
    "The two-hop workload is pathological for every candidate at this scale: it uses\nroughly 1.65-2.66 million shared-hit blocks and takes 10-15 seconds even though\nno shared-read or temporary blocks are recorded. This is a query/index design\nproblem that must be addressed before production regardless of the selected\nentity representation.\n",
    "The original two-hop workload was pathological for every candidate at this\nscale: it used roughly 1.65-2.66 million shared-hit blocks and took 10-15 seconds\neven though no shared-read or temporary blocks were recorded. EXPLAIN showed that\nthe query omitted the known `target_entity = 'variant'` and\n`target_entity = 'sales_channel'` discriminators, preventing full use of\n`link_target_lookup`. Those predicates are now part of all three candidate SQL\nqueries and are verifier-locked. The values above remain pre-fix diagnostics; a\nsame-commit 100k/1m rerun supplies the canonical cross-scale two-hop evidence.\n",
    "benchmark two-hop diagnosis",
)
benchmark_path.write_text(benchmark, encoding="utf-8")

adr_path = Path("DECISIONS/2026-07-24-index-storage-layout.md")
adr = adr_path.read_text(encoding="utf-8")
for old, new in [
    ("two-hop 11.516 s", "pre-fix two-hop 11.516 s; rerun pending"),
    ("two-hop 14.989 s", "pre-fix two-hop 14.989 s; rerun pending"),
    ("two-hop 10.305 s", "pre-fix two-hop 10.305 s; rerun pending"),
]:
    adr = replace_once(adr, old, new, f"ADR marker {old}")
adr = replace_once(
    adr,
    "- the two-hop workload is unacceptable for all three candidates and requires a\n  query/index redesign independent of the entity representation;",
    "- the pre-fix two-hop workload was unacceptable for all three candidates;\n  EXPLAIN identified missing typed-link target predicates, which are now fixed and\n  require a same-commit 100k/1m rerun before comparison;",
    "ADR two-hop finding",
)
adr_path.write_text(adr, encoding="utf-8")

for path in sql_paths:
    text = path.read_text(encoding="utf-8")
    for marker in (
        "product_variant.target_entity = 'variant'",
        "variant_channel.target_entity = 'sales_channel'",
    ):
        if marker not in text:
            raise SystemExit(f"{path}: missing {marker}")
