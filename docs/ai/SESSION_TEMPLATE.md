---
id: doc://docs/ai/SESSION_TEMPLATE.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# AI Session Template (RusToK)

Use this template before generating code in any module.

## Mandatory Preamble for AI

> Before generating code:
> 1) read `docs/AI_CONTEXT.md`;  
> 2) read `CRATE_API.md` of the target crate (if the file exists);  
> 3) read `README.md` of the target crate;  
> 4) if changes affect the historical Loco exit, first read `docs/architecture/loco-exit-plan.md` and the backend guides; for Iggy/MCP/Outbox/Telemetry, first check the corresponding reference package in `docs/references/`;
> 5) verify event invariants (`publish_in_tx`, `EventEnvelope`, handlers).

## Mini Prompt Template

```text
Context:
- Read docs/AI_CONTEXT.md.
- Read CRATE_API.md of the target crate (if present).
- Read README.md of the target crate.
- If changing the historical Loco exit, first read `docs/architecture/loco-exit-plan.md` and the backend guides. For Iggy/MCP/Outbox/Telemetry, first read the corresponding reference package in `docs/references/`.

Task:
- <briefly describe the task>

Constraints:
- Do not invent APIs of Axum, Iggy, or internal crates.
- For transactional flow use publish_in_tx.
- Verify EventEnvelope and handler compatibility.

Result:
- Provide a patch by file.
- Update documentation if contracts changed.
- Specify checks/tests performed.
```

## Pre-Answer Checklist

- [ ] Only existing public types/methods are used.
- [ ] For Loco/Iggy/MCP/Outbox/Telemetry changes, the reference package was checked first.
- [ ] For write + event flow, `publish_in_tx` is used (where required).
- [ ] Event handlers are compatible with the current `EventEnvelope`.
- [ ] Relevant docs (`docs/` and module-local docs) are updated.
