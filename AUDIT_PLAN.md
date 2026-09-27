# Full-Stack Audit Entry Point

Canonical trigger: `реализуй план аудита`

Execution rules and the complete sequential phase plan live in:
- `AGENTS.md` — mandatory governance and execution contract.
- `docs/standards/continuous-review-ledger.md` — single living audit-progress ledger.

Do not create or maintain a second audit checklist. Start from the next unchecked phase in the ledger, refresh `main`, create a dedicated phase branch, record findings before implementation, fix confirmed repository-owned root causes, then commit → PR → merge → refresh `main`.

Tests are run by the maintainer unless explicitly changed in `AGENTS.md`.
