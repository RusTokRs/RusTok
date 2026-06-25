# RUSTSEC-2026-0098 / RUSTSEC-2026-0099 / RUSTSEC-2026-0104: устранение

## Статус

Закрыто: 2026-06-25.

Workspace больше не содержит `rustls-webpki 0.101.7` и `rustls 0.21.12`. Ветка AWS SDK/S3 переведена на modern TLS path через `aws-smithy-http-client` с `rustls 0.23.40` и `rustls-webpki 0.103.13`.

## Что изменено

- В `crates/rustok-storage/Cargo.toml` для `aws-sdk-s3` и `aws-config` отключены default features.
- Для S3 backend явно включены только необходимые runtime/TLS features:
  - `behavior-version-latest`;
  - `default-https-client`;
  - `rt-tokio`;
  - `sigv4a` для `aws-sdk-s3`.
- Legacy feature `rustls`, которая тянула `rustls 0.21.x`, больше не включается.
- Из `deny.toml` удалены временные ignores:
  - `RUSTSEC-2026-0098`;
  - `RUSTSEC-2026-0099`;
  - `RUSTSEC-2026-0104`.

## Проверка

Выполнены локальные проверки:

```bash
cargo check -p rustok-storage --features s3
cargo tree -i rustls-webpki@0.101.7 --workspace
cargo tree -i rustls@0.21.12 --workspace
cargo tree -i rustls-webpki --workspace
```

Ожидаемый результат:

- `rustls-webpki@0.101.7` не найден;
- `rustls@0.21.12` не найден;
- `rustls-webpki` резолвится в `0.103.13`;
- S3 backend компилируется с обновлённым набором features.

Финальный gate для advisory policy:

```bash
cargo deny check advisories
```
