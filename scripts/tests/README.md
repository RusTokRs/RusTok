# Scripts tests

Локальные smoke/contract тесты для operational scripts.

## Запуск

```bash
scripts/tests/check_dependabot_directories_test.sh
scripts/tests/check_lifecycle_runbook_doc_links_test.sh
scripts/tests/auth_release_gate_test.sh
scripts/tests/page_builder_fba_verify_test.sh
scripts/tests/control_plane_remediation_minimal_runner_test.sh
scripts/tests/control_plane_remediation_verify_all_alias_test.sh
```

## Правила

- Тесты обязаны использовать изолированные fixture-каталоги (`mktemp`/`tempfile`) и не зависеть от текущего состояния репозитория.
- Для новых verify-скриптов сначала добавляйте smoke-тест с позитивным и негативным сценарием.
