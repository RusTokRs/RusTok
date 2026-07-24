from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new, 1)


plan_path = Path("crates/rustok-index/docs/implementation-plan.md")
plan = plan_path.read_text(encoding="utf-8")
plan = replace_once(
    plan,
    "  five churn cycles, and post-VACUUM cardinalities matched. The 1m stage remains\n  blocked until `INDEX_BENCH_LARGE_RUNNER` names a Linux runner with at least\n  35 GB free disk.\n",
    "  five churn cycles, and post-VACUUM cardinalities matched. That run's 1m stage\n  failed closed because `INDEX_BENCH_LARGE_RUNNER` was unset. The workflow now\n  prefers that explicit runner when configured and otherwise uses `ubuntu-latest`,\n  while the reusable job still rejects any runner below 35 GB free disk.\n",
    "implementation-plan 1m blocker",
)
progress = (
    "- 2026-07-24: runner evidence showed 93,030,404,096 free bytes before the 100k\n"
    "  packet and 88,893,792,256 after it. Enabled a guarded `ubuntu-latest` fallback\n"
    "  for the 1m stage while preserving `INDEX_BENCH_LARGE_RUNNER` as an override\n"
    "  and the existing 35 GB fail-closed disk check.\n"
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
    "- 1m evidence: blocked until `INDEX_BENCH_LARGE_RUNNER` names a Linux larger runner with at least 35 GB free disk",
    "- 1m evidence: enabled on `INDEX_BENCH_LARGE_RUNNER` when configured, otherwise `ubuntu-latest`, with a fail-closed 35 GB free-disk check",
    "storage benchmark status",
)
benchmark = replace_once(
    benchmark,
    "The workflow then failed closed before `1m` because repository variable\n`INDEX_BENCH_LARGE_RUNNER` is not configured. It must name a Linux runner label\nwith at least 35 GB free disk. The 100k result is therefore archived and accepted\nas evidence, while the storage ADR remains Proposed and M3 remains blocked.\n",
    "The inspected run failed closed before `1m` because repository variable\n`INDEX_BENCH_LARGE_RUNNER` was not configured. Its 100k resource snapshots showed\n93,030,404,096 free root-filesystem bytes before evidence and 88,893,792,256 after.\nThe scale workflow now prefers the configured runner when present and otherwise\nuses `ubuntu-latest`; the reusable job still rejects any runner with less than\n35,000,000,000 free bytes before the build. The 1m result remains pending, so the\nstorage ADR remains Proposed and M3 remains blocked.\n",
    "storage benchmark 1m policy",
)
benchmark_path.write_text(benchmark, encoding="utf-8")

adr_path = Path("DECISIONS/2026-07-24-index-storage-layout.md")
adr = adr_path.read_text(encoding="utf-8")
adr = replace_once(
    adr,
    "| 1m read/query | Pending larger runner | Pending larger runner | Pending larger runner |",
    "| 1m read/query | Pending guarded run | Pending guarded run | Pending guarded run |",
    "ADR 1m row",
)
adr = replace_once(
    adr,
    "The `1m` stage did not run. The workflow failed closed because repository variable\n`INDEX_BENCH_LARGE_RUNNER` is unset; it must name a Linux larger-runner label with\nat least 35 GB free disk. These findings are not sufficient to accept the ADR.\n",
    "The inspected run's `1m` stage did not run because repository variable\n`INDEX_BENCH_LARGE_RUNNER` was unset. The 100k runner nevertheless reported\n93,030,404,096 free root-filesystem bytes before evidence and 88,893,792,256 after.\nThe workflow now uses the configured larger-runner label when present and falls\nback to `ubuntu-latest` otherwise; the reusable job remains fail-closed below\n35,000,000,000 free bytes. The 1m packet is still pending, so these findings are\nnot sufficient to accept the ADR.\n",
    "ADR 1m runner policy",
)
adr_path.write_text(adr, encoding="utf-8")

for path in (plan_path, benchmark_path, adr_path):
    text = path.read_text(encoding="utf-8")
    if "93,030,404,096" not in text:
        raise SystemExit(f"{path}: missing observed runner capacity")
    if "35" not in text:
        raise SystemExit(f"{path}: missing fail-closed disk threshold")

workflow = Path(".github/workflows/index-storage-scale-evidence.yml").read_text(encoding="utf-8")
if "vars.INDEX_BENCH_LARGE_RUNNER || 'ubuntu-latest'" not in workflow:
    raise SystemExit("scale workflow does not contain the standard-runner fallback")
if "evidence-1m-runner-required" in workflow:
    raise SystemExit("obsolete 1m runner-required failure job remains")
