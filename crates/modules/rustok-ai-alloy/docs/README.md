# rustok-ai-alloy

Adapter for Alloy AI verticals and code-agent workflows.
Owns descriptors and validation of runtime payload for executing Alloy scripts (`alloy_code`).

## Contract Surface

- `ALLOY_CODE_TASK_SLUG` / `ALLOY_CODE_TOOL_NAME` fix the public identity for Alloy Assist.
- `AlloyAiVerticalDescriptor` fixes `runtime_operation = run_script`, `runtime_payload_json_shape = absent_blank_or_json_object` and `transport_owner = rustok-ai`.
- `alloy_script_execution_policy()` fixes Alloy runtime ownership, `allowed_operations` (`list_scripts`, `get_script`, `validate_script`, `run_script`) and remote transport status `not_started`.
- `AlloyOperation` is the single typed catalog for those operation slugs; the
  `rustok-ai` runtime dispatches it without defining a duplicate operation enum.
- `alloy_code_agents()` publishes planner, implementer, reviewer, and verifier
  roles; `alloy_swarm_workflows()` publishes the approval-gated
  `alloy_change_review` stage graph.
- `validate_runtime_payload()` allows absent/empty payload or JSON object and rejects arrays/scalars/invalid JSON.

Plan and evidence: [`docs/implementation-plan.md`](./implementation-plan.md), [`contracts/ai-alloy-policy-registry.json`](../contracts/ai-alloy-policy-registry.json).
