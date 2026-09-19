#!/usr/bin/env python3
"""RusToK Continuous Review Orchestrator (ACRE).

Coordinates the Scan -> Queue -> Remediate -> Gate lifecycle.
Used by contributors, automated scheduled tasks (/schedule), and AI agents.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

# Ensure UTF-8 output on Windows consoles
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent.parent
BACKLOG_PATH = ROOT / ".review_backlog.json"
SCANNER_PATH = ROOT / "scripts" / "maintenance" / "scan_codebase.py"
GATE_PATH = ROOT / "scripts" / "verify" / "verify-remediation-gate.py"


def run_scan(target: str | None = None, tier: int | None = None, max_findings: int = 50) -> dict:
    cmd = [sys.executable, str(SCANNER_PATH), "--max-findings", str(max_findings), "--summary"]
    if target:
        cmd.extend(["--target", target])
    if tier is not None:
        cmd.extend(["--tier", str(tier)])

    res = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace")
    print(res.stdout)
    if res.stderr:
        print(res.stderr, file=sys.stderr)

    if BACKLOG_PATH.exists():
        with BACKLOG_PATH.open("r", encoding="utf-8") as f:
            return json.load(f)
    return {}


def load_backlog() -> dict:
    if not BACKLOG_PATH.exists():
        print("[ACRE] No existing backlog found. Running initial scan...", flush=True)
        return run_scan()
    with BACKLOG_PATH.open("r", encoding="utf-8") as f:
        return json.load(f)


def get_next_finding(backlog: dict, tier: int | None = None, rule_id: str | None = None) -> dict | None:
    findings = backlog.get("findings", [])
    for f in findings:
        if tier is not None and f.get("tier") != tier:
            continue
        if rule_id and f.get("rule_id") != rule_id:
            continue
        return f
    return None


def generate_task_brief(finding: dict) -> str:
    file_path = finding["file"]
    line_num = finding["line"]
    rule_id = finding["rule_id"]
    tier = finding["tier"]
    severity = finding["severity"]
    snippet = finding["snippet"]
    remediation = finding["remediation"]
    reference = finding["reference"]

    # Identify crate path
    p = Path(file_path)
    crate_dir = ""
    for parent in p.parents:
        if (ROOT / parent / "Cargo.toml").exists():
            crate_dir = str(parent).replace("\\", "/")
            break

    brief = f"""# RusToK Auto-Remediation Task: [{rule_id}] {p.name}:{line_num}

## Target Information
- **File**: `{file_path}`
- **Line**: `{line_num}`
- **Crate**: `{crate_dir}`
- **Tier**: `Tier {tier} ({severity.upper()})`
- **Rule ID**: `{rule_id}`

## Code Context
```rust
// {file_path}:{line_num}
{snippet}
```

## Remediation Contract
- **Guidance**: {remediation}
- **Reference**: {reference}
- **Governance**: Follow `AGENTS.md` strictly:
  1. No temporary stubs, fake implementations, or broad lints (`#[allow(...)]` or `@ts-ignore` are prohibited).
  2. Zero-legacy policy: implement genuine target behavior or remove obsolete code cleanly.
  3. Never delete existing unit or integration tests.
  4. Ensure multi-tenant safety and strong typing.

## Verification Command
Run the gatekeeper before finalizing the change:
```bash
python scripts/verify/verify-remediation-gate.py --files {file_path}
```
"""
    return brief


def main() -> int:
    parser = argparse.ArgumentParser(description="RusToK Review Orchestrator")
    subparsers = parser.add_subparsers(dest="command", help="Sub-command to execute")

    # Scan command
    scan_parser = subparsers.add_parser("scan", help="Run scan and update backlog")
    scan_parser.add_argument("--target", type=str, help="Target path to scan")
    scan_parser.add_argument("--tier", type=int, help="Scan only specific tier (0, 1, 2)")
    scan_parser.add_argument("--max", type=int, default=50, help="Max findings to collect")

    # Next command
    next_parser = subparsers.add_parser("next", help="Show next highest-priority finding")
    next_parser.add_argument("--tier", type=int, help="Filter by tier")
    next_parser.add_argument("--rule", type=str, help="Filter by rule ID")

    # Brief command (for agents)
    brief_parser = subparsers.add_parser("brief", help="Generate task brief for the next finding")
    brief_parser.add_argument("--tier", type=int, help="Filter by tier")
    brief_parser.add_argument("--rule", type=str, help="Filter by rule ID")
    brief_parser.add_argument("--out", type=Path, help="Write task brief to file")

    # Gate command
    gate_parser = subparsers.add_parser("gate", help="Run remediation gatekeeper")
    gate_parser.add_argument("--files", nargs="*", help="Files to gate")
LEDGER_DATA_PATH = ROOT / ".review_ledger.json"
LEDGER_DOC_PATH = ROOT / "docs" / "standards" / "continuous-review-ledger.md"


def discover_components() -> dict[str, dict]:
    """Discovers all crates and apps with their file counts and LOC, pruning build dirs."""
    components: dict[str, dict] = {}
    ignored_dirs = {"target", ".git", "node_modules", "dist", ".next", "build", "coverage", ".gemini"}

    # Search in crates and apps
    search_roots = [ROOT / "crates", ROOT / "apps"]
    for s_root in search_roots:
        if not s_root.exists():
            continue
        for root_dir, dirs, files in os.walk(s_root):
            dirs[:] = [d for d in dirs if d not in ignored_dirs]
            # Check if this directory is a component root (has Cargo.toml or package.json)
            cur_path = Path(root_dir)
            if (cur_path / "Cargo.toml").exists() or (cur_path / "package.json").exists():
                rel_path = str(cur_path.relative_to(ROOT)).replace("\\", "/")
                # Avoid counting sub-crates multiple times if already parent
                if rel_path in components:
                    continue

                # Count files & loc
                code_files = 0
                loc = 0
                for sub_root, sub_dirs, sub_files in os.walk(cur_path):
                    sub_dirs[:] = [d for d in sub_dirs if d not in ignored_dirs]
                    for f in sub_files:
                        ext = os.path.splitext(f)[1]
                        if ext in (".rs", ".ts", ".tsx"):
                            code_files += 1
                            try:
                                loc += len((Path(sub_root) / f).read_text(encoding="utf-8", errors="replace").splitlines())
                            except Exception:
                                pass

                # Categorize
                parts = rel_path.split("/")
                category = parts[1] if parts[0] == "crates" and len(parts) > 1 else parts[0]

                components[rel_path] = {
                    "name": cur_path.name,
                    "path": rel_path,
                    "category": category,
                    "files": code_files,
                    "loc": loc,
                }
    return components


def load_ledger() -> dict:
    if LEDGER_DATA_PATH.exists():
        try:
            with LEDGER_DATA_PATH.open("r", encoding="utf-8") as f:
                return json.load(f)
        except Exception:
            pass

    import time
    return {
        "current_round": 1,
        "round_started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "completed_rounds": [],
        "components": {},
    }


def sync_ledger() -> dict:
    import time
    ledger = load_ledger()
    discovered = discover_components()

    current_round = ledger.get("current_round", 1)
    stored_comps = ledger.setdefault("components", {})

    # Update or add components
    for comp_path, disc in discovered.items():
        if comp_path in stored_comps:
            stored = stored_comps[comp_path]
            stored["files"] = disc["files"]
            stored["loc"] = disc["loc"]
            stored["category"] = disc["category"]
        else:
            stored_comps[comp_path] = {
                "name": disc["name"],
                "path": comp_path,
                "category": disc["category"],
                "files": disc["files"],
                "loc": disc["loc"],
                "completed": False,
                "completed_at": None,
                "notes": "",
            }

    # Check if all components are completed in current round
    all_done = len(stored_comps) > 0 and all(c.get("completed", False) for c in stored_comps.values())
    if all_done:
        # Rollover to next round!
        print(f"\n🎉 [ACRE] Round {current_round} is 100% COMPLETE! Rolling over to Round {current_round + 1}...")
        round_summary = {
            "round": current_round,
            "started_at": ledger.get("round_started_at"),
            "completed_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "total_components": len(stored_comps),
            "total_loc": sum(c.get("loc", 0) for c in stored_comps.values()),
        }
        ledger.setdefault("completed_rounds", []).append(round_summary)
        ledger["current_round"] = current_round + 1
        ledger["round_started_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

        # Reset all components for the new round
        for c in stored_comps.values():
            c["completed"] = False
            c["completed_at"] = None
            c["notes"] = ""

    # Save data
    with LEDGER_DATA_PATH.open("w", encoding="utf-8") as f:
        json.dump(ledger, f, indent=2, ensure_ascii=False)

    # Render Markdown documentation
    md_content = render_ledger_markdown(ledger)
    LEDGER_DOC_PATH.write_text(md_content, encoding="utf-8")
    print(f"[ACRE] Ledger synchronized: {LEDGER_DOC_PATH.name} updated.")
    return ledger


def render_ledger_markdown(ledger: dict) -> str:
    current_round = ledger.get("current_round", 1)
    started_at = ledger.get("round_started_at", "N/A")
    comps = ledger.get("components", {})
    completed_rounds = ledger.get("completed_rounds", [])

    total_comps = len(comps)
    done_comps = sum(1 for c in comps.values() if c.get("completed", False))
    pct = int((done_comps / total_comps * 100)) if total_comps > 0 else 0
    total_loc = sum(c.get("loc", 0) for c in comps.values())

    lines = [
        "---",
        "id: doc://docs/standards/continuous-review-ledger.md",
        "kind: project_overview",
        "language: markdown",
        "last_verified_snapshot: snap_jsonl_00000021",
        "source_language: markdown",
        "status: active",
        "---",
        "",
        "# Continuous Code Review & Remediation Ledger (ACRE)",
        "",
        f"Tracking persistent progress across cyclical review rounds for all modules in RusToK.",
        "",
        "## Current Cycle Status",
        f"- **Active Round:** Round {current_round}",
        f"- **Cycle Started:** `{started_at}`",
        f"- **Progress:** `{done_comps} / {total_comps}` components audited (**{pct}%**)",
        f"- **Total Workspace Codebase:** `{total_loc:,}` LOC across `{total_comps}` modules/apps",
        "",
        "---",
        "",
        "## Components Review Status",
        "",
        "| Status | Component | Category | Files | LOC | Last Audited | Notes |",
        "|:---:|---|---|---:|---:|---|---|",
    ]

    # Group components by category
    for path, c in sorted(comps.items(), key=lambda x: (x[1].get("category", ""), x[0])):
        status_box = "[x]" if c.get("completed", False) else "[ ]"
        status_badge = "DONE" if c.get("completed", False) else "PENDING"
        comp_link = f"[{c['name']}](../../{path})"
        cat = c.get("category", "")
        files = c.get("files", 0)
        loc = c.get("loc", 0)
        audited_at = c.get("completed_at", "—")
        notes = c.get("notes", "—")
        lines.append(f"| {status_box} | {comp_link} | `{cat}` | {files} | {loc:,} | {audited_at} | {notes} |")

    lines.append("")
    lines.append("---")
    lines.append("")
    lines.append("## Completed Rounds Archive")
    if not completed_rounds:
        lines.append("_No completed rounds yet. Round 1 is currently in progress._")
    else:
        for r in completed_rounds:
            lines.append(f"- **Round {r['round']}**: Completed on `{r['completed_at']}` ({r['total_components']} components, {r['total_loc']:,} LOC).")

    lines.append("")
    return "\n".join(lines)


def mark_component_done(comp_name_or_path: str, notes: str = "") -> bool:
    import time
    ledger = sync_ledger()
    comps = ledger.get("components", {})

    target_key = None
    for path, c in comps.items():
        if comp_name_or_path in (path, c["name"], c["path"]):
            target_key = path
            break

    if not target_key:
        print(f"[ACRE] Error: Component '{comp_name_or_path}' not found in ledger.", file=sys.stderr)
        return False

    c = comps[target_key]
    c["completed"] = True
    c["completed_at"] = time.strftime("%Y-%m-%d %H:%M", time.gmtime())
    c["notes"] = notes or "Audited and cleaned."

    with LEDGER_DATA_PATH.open("w", encoding="utf-8") as f:
        json.dump(ledger, f, indent=2, ensure_ascii=False)

    # Re-sync to update markdown and check for round rollover
    sync_ledger()
    print(f"[ACRE] ✓ Component '{target_key}' marked as COMPLETED in Round {ledger['current_round']}.")
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description="RusToK Review Orchestrator")
    subparsers = parser.add_subparsers(dest="command", help="Sub-command to execute")

    # Scan command
    scan_parser = subparsers.add_parser("scan", help="Run scan and update backlog")
    scan_parser.add_argument("--target", type=str, help="Target path to scan")
    scan_parser.add_argument("--tier", type=int, help="Scan only specific tier (0, 1, 2)")
    scan_parser.add_argument("--max", type=int, default=50, help="Max findings to collect")

    # Next command
    next_parser = subparsers.add_parser("next", help="Show next highest-priority finding")
    next_parser.add_argument("--tier", type=int, help="Filter by tier")
    next_parser.add_argument("--rule", type=str, help="Filter by rule ID")

    # Brief command (for agents)
    brief_parser = subparsers.add_parser("brief", help="Generate task brief for the next finding")
    brief_parser.add_argument("--tier", type=int, help="Filter by tier")
    brief_parser.add_argument("--rule", type=str, help="Filter by rule ID")
    brief_parser.add_argument("--out", type=Path, help="Write task brief to file")

    # Gate command
    gate_parser = subparsers.add_parser("gate", help="Run remediation gatekeeper")
    gate_parser.add_argument("--files", nargs="*", help="Files to gate")
    gate_parser.add_argument("--skip-cargo", action="store_true", help="Skip cargo clippy")

    # Ledger commands
    ledger_sync_parser = subparsers.add_parser("ledger-sync", help="Synchronize components inventory and ledger document")
    ledger_status_parser = subparsers.add_parser("ledger-status", help="Show ledger status and active round progress")
    mark_parser = subparsers.add_parser("mark-done", help="Mark a component audited in the active round")
    mark_parser.add_argument("component", type=str, help="Component name or relative path")
    mark_parser.add_argument("--notes", type=str, default="", help="Remediation notes")

    args = parser.parse_args()

    if args.command == "scan":
        run_scan(target=args.target, tier=args.tier, max_findings=args.max)
        return 0

    elif args.command == "next":
        backlog = load_backlog()
        finding = get_next_finding(backlog, tier=args.tier, rule_id=args.rule)
        if not finding:
            print("[ACRE] No matching findings found in backlog. Code is clean for given criteria!")
            return 0
        print(f"\n[Next Finding] {finding['id']}")
        print(f"  Tier: {finding['tier']} ({finding['severity']})")
        print(f"  File: {finding['file']}:{finding['line']}")
        print(f"  Snippet: {finding['snippet']}")
        print(f"  Remediation: {finding['remediation']}")
        return 0

    elif args.command == "brief":
        backlog = load_backlog()
        finding = get_next_finding(backlog, tier=args.tier, rule_id=args.rule)
        if not finding:
            print("[ACRE] Backlog empty. No pending remediation tasks.")
            return 0
        brief = generate_task_brief(finding)
        if args.out:
            args.out.write_text(brief, encoding="utf-8")
            print(f"[ACRE] Task brief written to {args.out}")
        else:
            print(brief)
        return 0

    elif args.command == "gate":
        cmd = [sys.executable, str(GATE_PATH)]
        if args.files:
            cmd.extend(["--files"] + args.files)
        if args.skip_cargo:
            cmd.append("--skip-cargo")
        res = subprocess.run(cmd, cwd=ROOT)
        return res.returncode

    elif args.command == "ledger-sync":
        sync_ledger()
        return 0

    elif args.command == "ledger-status":
        ledger = sync_ledger()
        round_num = ledger.get("current_round", 1)
        comps = ledger.get("components", {})
        total = len(comps)
        done = sum(1 for c in comps.values() if c.get("completed", False))
        pct = int((done / total * 100)) if total > 0 else 0
        print(f"\n[ACRE Ledger] Round {round_num}: {done}/{total} completed ({pct}%)")
        pending = [c["path"] for c in comps.values() if not c.get("completed", False)]
        if pending:
            print(f"  Next pending components to review:")
            for p in pending[:5]:
                print(f"    - {p}")
        return 0

    elif args.command == "mark-done":
        ok = mark_component_done(args.component, notes=args.notes)
        return 0 if ok else 1

    else:
        parser.print_help()
        return 0


if __name__ == "__main__":
    sys.exit(main())
