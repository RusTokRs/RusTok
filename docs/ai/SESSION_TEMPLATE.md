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
> 4) if changes affect Loco/Iggy/MCP/Outbox/Telemetry — first check the reference package (`docs/references/loco/README.md`, `docs/references/iggy/README.md`, `docs/references/mcp/README.md`, `docs/references/outbox/README.md`, `docs/references/telemetry/README.md`);
> 5) verify event invariants (`publish_in_tx`, `EventEnvelope`, handlers).

## Mini Prompt Template

```text
Context:
- Read docs/AI_CONTEXT.md.
- Read CRATE_API.md of the target crate (if present).
- Read README.md of the target crate.
- If changing Loco/Iggy/MCP/Outbox/Telemetry — first read the corresponding reference package in `docs/references/`.

Task:
- <briefly describe the task>

Constraints:
- Do not invent API of Loco/Iggy/internal crates.
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
