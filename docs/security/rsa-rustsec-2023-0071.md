---
id: doc://docs/security/rsa-rustsec-2023-0071.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# RUSTSEC-2023-0071 remediation note

## Статус

Устранено 25 июня 2026 года.

Workspace переведён с backend `jsonwebtoken/rust_crypto` на
`jsonwebtoken/aws_lc_rs`. Поддержка `HS256` и `RS256` сохранена, а транзитивная
зависимость от `rsa 0.9.10` удалена.

## Проверка

```bash
cargo tree -i rsa@0.9.10 --workspace
cargo deny check advisories
cargo test -p rustok-auth --lib
```

Первая команда не должна находить обратных зависимостей. Исключение
`RUSTSEC-2023-0071` удалено из `deny.toml`.
