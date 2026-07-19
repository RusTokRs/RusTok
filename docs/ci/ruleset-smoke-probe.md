# Repository ruleset smoke probe

This temporary draft-PR-only file triggers the base-owned migration approval evaluator, sandboxed migration preflight, standalone ruleset contract suite and live repository ruleset audit while a protected migration infrastructure change is explicitly approved.

Expected before administrative activation:

- `Migration harness approval`: success on this PR head SHA because `migration-infra-approved` is present;
- `Repository ruleset contract`: failure on this PR head SHA because the checked active ruleset has not been enabled yet.

Approved-change probe: the branch intentionally changes a protected policy file and must pass only while the approval label exists.

It must never be merged into `main`.
