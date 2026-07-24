from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new, 1)


def insert_before(text: str, marker: str, addition: str, label: str) -> str:
    count = text.count(marker)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one marker, found {count}")
    return text.replace(marker, addition + marker, 1)


plan_path = Path("crates/rustok-index/docs/implementation-plan.md")
plan = plan_path.read_text(encoding="utf-8")
plan = replace_once(
    plan,
    "- [ ] Run and archive 100k Product-locale row read, mutation, and maintenance\n      evidence.",
    "- [x] Run and archive 100k Product-locale row read, mutation, and maintenance\n      evidence.",
    "M2 100k checklist",
)
progress = (
    "- 2026-07-24: inspected Actions run `30051321255` and artifact\n"
    "  `index-storage-100k-84a11b147689b226ca161f5a0287990c1e8489d4`.\n"
    "  PostgreSQL 16 preserved 300,080 entities and 600,000 links across JSONB,\n"
    "  typed EAV, and hot projection candidates; all read digests, mutation effects,\n"
    "  five churn cycles, and post-VACUUM cardinalities matched. The 1m stage remains\n"
    "  blocked until `INDEX_BENCH_LARGE_RUNNER` names a Linux runner with at least\n"
    "  35 GB free disk.\n"
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
    "- 100k/1m evidence: pending repository-owner execution\n- Storage decision ADR: pending scale evidence and comparison",
    "- 100k evidence: archived and inspected from Actions run `30051321255`\n- 1m evidence: blocked until `INDEX_BENCH_LARGE_RUNNER` names a Linux larger runner with at least 35 GB free disk\n- Storage decision ADR: Proposed; 100k evidence is populated, acceptance still waits on 1m and the cross-scale comparison",
    "benchmark status",
)
benchmark = replace_once(
    benchmark,
    "This smoke packet proves harness sanity only. Its small-scale latency, size, WAL,\nand VACUUM values must not select a production candidate; the 100k and 1m runs\nand the cross-scale comparison remain required.\n\n",
    "This smoke packet proves harness sanity only. Its small-scale latency, size, WAL,\nand VACUUM values must not select a production candidate. The inspected 100k\npacket establishes the first scale baseline; the 1m packet and cross-scale\ncomparison remain required before a production model is selected.\n\n",
    "smoke decision boundary",
)
evidence_section = """### Inspected 100k scale evidence

Actions run `30051321255` archived artifact
`index-storage-100k-84a11b147689b226ca161f5a0287990c1e8489d4` for
PostgreSQL 16, three repetitions, and five committed churn cycles. Provenance
records PR merge commit `84a11b147689b226ca161f5a0287990c1e8489d4`.
The packet contains the three JSON reports plus before/after runner resource
snapshots.

The validated dataset contains 100,000 Product-locale rows, 300,080 total entity
rows, and 600,000 links. Every candidate preserved exact cardinality, produced
identical result rows and digests for all six read workloads, affected the same
1,000 Product entities and 2,000 outgoing links in mutation validation, and
returned to exact cardinality after five churn cycles and `VACUUM (ANALYZE)`.
Every read and mutation workload retained one plan shape across its three
repetitions.

| Candidate | Load | Baseline size | Churn growth | Dead tuples after churn | VACUUM |
|---|---:|---:|---:|---:|---:|
| JSONB entity rows | 9.499 s | 385.58 MiB | 6.80 MiB (1.76%) | 20,000 | 800 ms |
| Typed EAV | 17.441 s | 687.23 MiB | 10.97 MiB (1.60%) | 69,934 | 921 ms |
| Hot typed projection | 6.132 s | 295.56 MiB | 4.61 MiB (1.56%) | 20,000 | 728 ms |

Warm-median read execution in milliseconds:

| Candidate | Status equality | Price range | Multi-value tag | Two-hop channel | Keyset page | Exact count |
|---|---:|---:|---:|---:|---:|---:|
| JSONB entity rows | 0.222 | 0.105 | 1.895 | 11,515.678 | 0.563 | 1.483 |
| Typed EAV | 7.074 | 6.102 | 4.742 | 14,989.380 | 20.814 | 4.074 |
| Hot typed projection | 0.073 | 0.071 | 1.394 | 10,305.135 | 0.032 | 0.456 |

The two-hop workload is pathological for every candidate at this scale: it uses
roughly 1.65-2.66 million shared-hit blocks and takes 10-15 seconds even though
no shared-read or temporary blocks are recorded. This is a query/index design
problem that must be addressed before production regardless of the selected
entity representation.

Median mutation execution and maximum-node WAL bytes:

| Candidate | Update 1,000 Products | Update WAL | Delete 1,000 Products + 2,000 links | Delete WAL |
|---|---:|---:|---:|---:|
| JSONB entity rows | 51.060 ms | 1,054,238 B | 27.165 ms | 162,000 B |
| Typed EAV | 62.207 ms | 1,238,933 B | 46.305 ms | 594,000 B |
| Hot typed projection | 43.672 ms | 834,784 B | 24.683 ms | 162,000 B |

Ordinary VACUUM reduced estimated dead tuples to zero for every candidate but did
not shrink relation files; after-VACUUM size deltas were small positive values,
which is valid under the benchmark's neutral size-delta rule.

The workflow then failed closed before `1m` because repository variable
`INDEX_BENCH_LARGE_RUNNER` is not configured. It must name a Linux runner label
with at least 35 GB free disk. The 100k result is therefore archived and accepted
as evidence, while the storage ADR remains Proposed and M3 remains blocked.

"""
if "### Inspected 100k scale evidence" not in benchmark:
    benchmark = insert_before(benchmark, "Optional settings:\n", evidence_section, "100k evidence section")
benchmark_path.write_text(benchmark, encoding="utf-8")

adr_path = Path("DECISIONS/2026-07-24-index-storage-layout.md")
adr = adr_path.read_text(encoding="utf-8")
old_table = """| Criterion | JSONB entity rows | Typed EAV | Hot typed projection |
| --- | --- | --- | --- |
| 100k read/query | Pending | Pending | Pending |
| 1m read/query | Pending | Pending | Pending |
| Relation-size scaling | Pending | Pending | Pending |
| Mutation/WAL | Pending | Pending | Pending |
| Churn/VACUUM | Pending | Pending | Pending |
| Planner stability | Pending | Pending | Pending |
| Dynamic schema evolution | Pending | Pending | Baseline limitation |
| Operational complexity | Pending | Pending | Baseline limitation |
"""
new_table = """| Criterion | JSONB entity rows | Typed EAV | Hot typed projection |
| --- | --- | --- | --- |
| 100k read/query | Status/keyset/count warm medians 0.222/0.563/1.483 ms; two-hop 11.516 s | 7.074/20.814/4.074 ms; two-hop 14.989 s | 0.073/0.032/0.456 ms; two-hop 10.305 s |
| 1m read/query | Pending larger runner | Pending larger runner | Pending larger runner |
| Relation-size scaling | 385.58 MiB at 100k; 1m ratio pending | 687.23 MiB at 100k; 1m ratio pending | 295.56 MiB at 100k; 1m ratio pending |
| Mutation/WAL | Update 51.060 ms / 1,054,238 B; delete 27.165 ms / 162,000 B | Update 62.207 ms / 1,238,933 B; delete 46.305 ms / 594,000 B | Update 43.672 ms / 834,784 B; delete 24.683 ms / 162,000 B |
| Churn/VACUUM | +6.80 MiB, 20,000 dead, 800 ms | +10.97 MiB, 69,934 dead, 921 ms | +4.61 MiB, 20,000 dead, 728 ms |
| Planner stability | One shape per read/mutation workload at 100k; 1m pending | One shape per read/mutation workload at 100k; 1m pending | One shape per read/mutation workload at 100k; 1m pending |
| Dynamic schema evolution | Pending operational review | Pending operational review | Baseline limitation |
| Operational complexity | Pending final review | Pending final review | Baseline limitation |
"""
adr = replace_once(adr, old_table, new_table, "ADR comparison table")
inspection = """## Inspected 100k evidence

Actions run `30051321255` and artifact
`index-storage-100k-84a11b147689b226ca161f5a0287990c1e8489d4`
passed the scale validator for PostgreSQL 16, three repetitions, and five churn
cycles. All candidates preserved 300,080 entities and 600,000 links, returned
identical read rows/digests, matched mutation effects, and preserved cardinality
through churn and VACUUM.

The 100k packet establishes these provisional findings:

- the hot typed projection is the size/load/read/write best-case baseline, but it
  does not satisfy the generic dynamic-schema requirement by itself;
- JSONB is materially smaller and faster than typed EAV across this packet;
- typed EAV has the largest relation size, slowest load, highest delete WAL, and
  highest post-churn dead-tuple estimate;
- the two-hop workload is unacceptable for all three candidates and requires a
  query/index redesign independent of the entity representation;
- all plan shapes are stable across the three repetitions at 100k;
- ordinary VACUUM clears dead-tuple estimates but does not shrink relation files.

The `1m` stage did not run. The workflow failed closed because repository variable
`INDEX_BENCH_LARGE_RUNNER` is unset; it must name a Linux larger-runner label with
at least 35 GB free disk. These findings are not sufficient to accept the ADR.

"""
if "## Inspected 100k evidence" not in adr:
    adr = insert_before(adr, "## Decision\n", inspection, "ADR 100k evidence section")
adr_path.write_text(adr, encoding="utf-8")

for path in (plan_path, benchmark_path, adr_path):
    text = path.read_text(encoding="utf-8")
    if "30051321255" not in text:
        raise SystemExit(f"{path}: missing inspected run id")

if "- [x] Run and archive 100k Product-locale row" not in plan_path.read_text(encoding="utf-8"):
    raise SystemExit("implementation plan did not close the 100k evidence item")
if "- [ ] Run and archive 1m Product-locale row" not in plan_path.read_text(encoding="utf-8"):
    raise SystemExit("implementation plan must keep the 1m evidence item open")
