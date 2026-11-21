---
name: gitnexus-audit
description: "Use when the user requests executing, running, or continuing the GitNexus deep graph audit plan. Examples: \"реализуй план gitnexus\", \"реализуй план аудита gitnexus\", \"execute gitnexus audit plan\""
---

# GitNexus Deep Graph Audit Protocol (GDAC)

This skill guides contributors and AI agents in executing the canonical 18-vector knowledge-graph audit across all 245 packages in RusToK.

## Canonical Trigger

The instruction that activates this protocol is:
```text
реализуй план gitnexus
```
*(or `реализуй план аудита gitnexus` / `execute gitnexus audit plan`)*

---

## Workflow for Each Component

1. **Read Core Contracts:**
   * [AGENTS.md §2.1.3](../../AGENTS.md) — canonical audit contract and infinite cycle rules.
   * [docs/standards/gitnexus-deep-audit-plan.md](../../docs/standards/gitnexus-deep-audit-plan.md) — active cycle, phase, and pending component.
   * [docs/standards/gitnexus-capabilities-reference.md](../../docs/standards/gitnexus-capabilities-reference.md) — Cypher recipes and CLI reference.

2. **Identify Target Component:**
   * Locate `Active Phase` and `Next Pending Component` in the header of `gitnexus-deep-audit-plan.md`.
   * Focus strictly on this one component (do not skip ahead).

3. **Verify Across All 18 Vectors:**
   * **V1:** Cycles (`check --cycles`)
   * **V2:** Database Sovereignty & Layering (`QUERIES`, `P-OWNER-READ-BOUNDARY`)
   * **V3:** Cluster Cohesion & God Elements (`clusters`, Cohesion Score, Fan-in/out)
   * **V4:** Process Traces (`processes`, `trace`, `status: "no_path"` checks)
   * **V5:** Zero-Callers Triage (WIP vs Public SDK vs Macros vs True Dead Code)
   * **V6:** Field Encapsulation (`ACCESSES` vs `HAS_METHOD`)
   * **V7:** Tri-UI Parity (`route_map`: Next.js ↔ Leptos ↔ Dioxus)
   * **V8:** Shape Drift (`shape_check`: `MISMATCH` keys)
   * **V9:** Control Guards (`pdg_query controls`: `guard: true`, `TenantContext`, RBAC)
   * **V10:** Data Flow & Anti-Swallowing (`pdg_query flows`: `P-ANTI-SWALLOW`, `let _ =`)
   * **V11:** Taint Security (`explain`: SQLi, Path Traversal, SSRF, Command Injection)
   * **V12:** Event Bus & Outbox (AsyncAPI `Destination`, Orphan Events, Cache Order)
   * **V13:** Blast Radius (`impact`, `detect_changes`)
   * **V14:** Temporal Coupling & Copy Drift (`P-COPY-DRIFT-MOUNT`)
   * **V15:** Agent Tools (`tool_map`, `HANDLES_TOOL`)
   * **V16:** Trait Completeness (`IMPLEMENTS`, `METHOD_IMPLEMENTS`, zero `todo!` / `unimplemented!`)
   * **V17:** Middleware Wrappers (`WRAPS`, `INJECTS`)
   * **V18:** External Contracts (`group_sync`, `CONTRACT_LINK`)

4. **Triage & Preserve Good Code:**
   * Follow the Decision Trees in §1 of the plan.
   * Connect and wire up WIP features. Delete only confirmed legacy dead code.

5. **Scoped Verification:**
   * Run targeted checks: `cargo check -p <crate>`, `cargo test -p <crate> --lib`.
   * Run pre-commit diff analysis: `node .gitnexus/run.cjs detect-changes --scope all --repo .`.
   * **Do NOT compile `apps/server` or run `--workspace` builds.**

6. **Update Ledger:**
   * In `gitnexus-deep-audit-plan.md`, check all 18 boxes (`[x]`) for the component.
   * Update `Next Pending Component` to the next package in the sequence.
   * If all 245 packages are complete, execute the **Infinite Cycle Reset Protocol** (§5 of the plan).
