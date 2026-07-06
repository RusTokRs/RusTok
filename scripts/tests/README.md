# Scripts tests

Local smoke/contract tests for operational scripts.

## Running

```bash
scripts/tests/check_dependabot_directories_test.sh
scripts/tests/check_lifecycle_runbook_doc_links_test.sh
scripts/tests/auth_release_gate_test.sh
scripts/tests/page_builder_fba_verify_test.sh
```

## Rules

- Tests must use isolated fixture directories (`mktemp`/`tempfile`) and must not depend on the current repository state.
- For new verify scripts, first add a smoke test with a positive and negative scenario.
