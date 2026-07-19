# Repository ruleset smoke probe

This temporary draft-PR-only file triggers the base-owned migration approval evaluator, sandboxed migration preflight, standalone ruleset contract suite and live repository ruleset audit without changing protected migration infrastructure.

Expected before administrative activation:

- `Migration harness approval`: success on this PR head SHA;
- `Repository ruleset contract`: failure on this PR head SHA because the checked active ruleset has not been enabled yet.

Synchronization probe: this commit was created after PR #1838 reported base `257afb6f6d6fda0791e0763b3fb680b06c11db06`.

It must never be merged into `main`.
