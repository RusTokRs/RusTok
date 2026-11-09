"""
Clean and reset continuous-review-ledger.md, cross-cutting-review-ledger.md, and .review_ledger.json
for a fresh pass from scratch, removing all past audit descriptions and findings.
"""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent

def clean_cross_cutting_ledger():
    path = ROOT / "docs" / "standards" / "cross-cutting-review-ledger.md"
    text = path.read_text(encoding="utf-8")

    # 1. Update Current Cycle Status
    idx_status = text.find("## Current Cycle Status\n")
    idx_proto = text.find("## Cross-Cutting Audit Execution Protocol")
    assert idx_status != -1 and idx_proto != -1, "Could not find cycle status section"

    new_status = """## Current Cycle Status

- **Status:** ACTIVE
- **Active Cycle:** Cross-Cutting Cycle 2 (Fresh pass from scratch)
- **Active Phase:** CC-01: Fluid Backend Architecture (FBA) & Module Sovereignty
- **Completed Phases:** None (0/17 completed — 0%)
- **Active Branch:** `main`

---

"""
    text = text[:idx_status] + new_status + text[idx_proto:]

    # 2. Update Phase Backlog lines
    def repl_backlog(m):
        return f"- [ ] **[{m.group(1)}]({m.group(2)})** (0/{m.group(3)} completed)"

    text = re.sub(
        r"- \[[ x]\] \*\*\[(CC-\d+: [^\]]+)\]\((#[^\)]+)\)\*\* \(\d+/(\d+) completed[^\)]*\)",
        repl_backlog,
        text,
    )

    # 3. Update Sub-Audit Checklist items: strip description
    def repl_subitem(m):
        title = m.group(1).rstrip(":")
        return f"- [ ] **{title}**"

    text = re.sub(
        r"- \[[ x]\] \*\*(CC-\d+\.\d+ — .*?):?\*\*.*",
        repl_subitem,
        text,
    )

    path.write_text(text, encoding="utf-8")
    print(f"Updated {path.name}")


def clean_continuous_review_ledger():
    path = ROOT / "docs" / "standards" / "continuous-review-ledger.md"
    text = path.read_text(encoding="utf-8")

    # Find where the phase backlog begins
    marker_backlog = "### Phase Backlog — Architectural Sequence"
    idx_backlog = text.find(marker_backlog)
    assert idx_backlog != -1, "Could not find Phase Backlog marker"

    # Find where the components table begins
    marker_table = "## Components Review Status"
    idx_table = text.find(marker_table)
    assert idx_table != -1, "Could not find Components Review Status marker"

    # New preamble replacing all previous completed audit reports and the Forum notes
    new_preamble = """# Continuous Full-Stack Review & Architecture Audit Ledger

## Deep Full-Stack Audit Cycle — Round 2 (Fresh Start from Scratch)

**Status:** ACTIVE  
**Active phase:** Phase FS-01 — Entrypoint & Composition Root (`apps/server`)  
**Next pending component:** [`apps/server`](../../apps/server) (485 files, 190,468 LOC)  
**Current main SHA:** `60267e392b`  
**Active branch:** `main`

**Purpose:** sequential, top-down root-to-leaf audit of the entire RusToK codebase across all **285 components** (~2.5M LOC) structured by architectural layer: Host -> Foundation Libraries -> Core Platform Modules -> Commerce -> Content & Social -> Catalog & Extensibility -> Search & AI -> Workers & Execution Fleet -> Platform Utilities -> Frontend Framework -> Client Applications.

### Deep Audit Execution Protocol — mandatory for every component

Every audit iteration targets exactly **one primary production crate/package** and executes the strict closed-loop protocol defined by `AGENTS.md` §2.1 and `CONTINUOUS_CODE_REVIEW.md`:

1. **Exhaustive Discovery Pass (No Surface Skimming):** Read the complete in-scope crate tree end-to-end across all source files in `src/`, not merely 1-2 files flagged by a scanner. Understand the crate's purpose, public API, domain model, persistence, handlers, and dependency contracts.
2. **Behavioral & Logical Verification (6 Universal Dimensions):** Verify preconditions and extreme bounds (0, negative, empty, overflow), branch completeness & anti-swallowing (no silent `let _ =` or fake `Ok(())` suppression), state transition integrity, side-effect completeness (DB mutation <-> outbox <-> cache <-> balances), calculation precision (money, cursor `<` vs `<=`), and concurrency guards (TOCTOU elimination).
3. **Zero AI Slop:** eliminate `include!("...")` macros, phantom facades, nested pyramid `match`/`if let` blocks, redundant `.clone()`, and captain-obvious comments.
4. **Exhaustive Remediation to Zero Defects ("Косяков нет — теперь иду дальше"):** Fix ALL discovered defects across the entire crate in the same change set. Never advance to the next component while known defects or unverified files remain in the current crate. Add pure unit tests for verified logic branches.
5. **FFA Boundary Integrity (for UI packages):** verify framework-agnostic `core/` contains 0 UI framework imports and pure unit tests; verify `ui/leptos` and `ui/dioxus` contain 0 domain or validation logic.
6. **Scoped Verification:** run crate-targeted checks (`cargo check -p <crate>`, `cargo test -p <crate>`); run `python scripts/verify/verify-remediation-gate.py` on modified files.
7. **Commit & Ledger Update:** commit cleanly, record the specific verified logic chains and remediated defects in this ledger, and mark the component completed in `.review_ledger.json`.

### Canonical Trigger
The single canonical trigger to continue this audit sequence is:
```text
реализуй план аудита
```

---

"""

    # Middle section: Phase Backlog and Phase items
    middle_text = text[idx_backlog:idx_table]

    # Reset Phase Backlog header and checkboxes
    middle_text = re.sub(
        r"### Phase Backlog — Architectural Sequence \(.*?\)",
        "### Phase Backlog — Architectural Sequence (285 Components Total — 0% COMPLETE)",
        middle_text,
    )
    middle_text = re.sub(
        r"- \[[ x]\] \*\*(\[Phase FS-\d+ — [^\]]+\]\([^\)]+\))\*\* \(\d+/\d+ completed[^\)]*\)",
        lambda m: f"- [ ] **{m.group(1)}** (0/{m.group(0).split('/')[-1].split()[0]} completed)",
        middle_text,
    )

    # In phase headings, reset completed counts: e.g. (10/10 completed) -> (0/10 completed)
    middle_text = re.sub(
        r"\(\d+/(\d+) completed[^\)]*\)",
        r"(0/\1 completed)",
        middle_text,
    )

    # In component items, strip descriptions!
    # Pattern: - [x] **[link](path)** (stats) — description
    # or: - [x] ~~eliminated~~ — description
    def repl_comp_item(m):
        full_line = m.group(0)
        # If eliminated crate, preserve the note
        if "~~" in full_line:
            return full_line.replace("- [x]", "- [ ]")
        comp = m.group(1)
        stats = m.group(2)
        if stats:
            return f"- [ ] {comp} ({stats})"
        return f"- [ ] {comp}"

    middle_text = re.sub(
        r"- \[[ x]\] (\*\*\[.*?\]\(.*?\)\*\*|\~\~.*?\~\~)(?: \((.*?)\))?(?: — .*)?",
        repl_comp_item,
        middle_text,
    )

    # Table section: clean rows
    table_text = text[idx_table:]
    # Replace checked rows and clear notes/audited date
    table_lines = table_text.splitlines()
    new_table_lines = []
    in_table = False
    for line in table_lines:
        if line.startswith("| Status |"):
            in_table = True
            new_table_lines.append(line)
            continue
        if in_table and line.startswith("|:---:"):
            new_table_lines.append(line)
            continue
        if in_table and line.startswith("| ["):
            parts = [p.strip() for p in line.split("|")]
            # parts[0] is empty, parts[1] is Status, parts[2] is Component, parts[3] is Category,
            # parts[4] is Files, parts[5] is LOC, parts[6] is Last Audited, parts[7] is Notes, parts[8] is empty
            if len(parts) >= 8:
                comp = parts[2]
                cat = parts[3]
                files = parts[4]
                loc = parts[5]
                new_table_lines.append(f"| [ ] | {comp} | {cat} | {files} | {loc} | None |  |")
            else:
                new_table_lines.append(line)
            continue
        if in_table and not line.startswith("|"):
            in_table = False
        new_table_lines.append(line)

    new_content = new_preamble + middle_text.rstrip() + "\n\n" + "\n".join(new_table_lines) + "\n"
    path.write_text(new_content, encoding="utf-8")
    print(f"Updated {path.name}")


def clean_review_ledger_json():
    path = ROOT / ".review_ledger.json"
    with path.open("r", encoding="utf-8") as f:
        data = json.load(f)

    data["current_round"] = 2
    import time
    data["round_started_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

    for comp in data.get("components", {}).values():
        comp["completed"] = False
        comp["completed_at"] = None
        comp["notes"] = ""

    with path.open("w", encoding="utf-8") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)
    print(f"Updated {path.name}")


if __name__ == "__main__":
    clean_cross_cutting_ledger()
    clean_continuous_review_ledger()
    clean_review_ledger_json()

