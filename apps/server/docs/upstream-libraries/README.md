# Upstream snapshots for server core libraries

Этот каталог фиксирует **свежие ссылки на документацию** по ключевым библиотекам сервера.

- Источник версий: `Cargo.lock`
- Дата snapshot: `2026-10-08`
- Обновление: `make docs-sync-server-libs`
- Проверка свежести: `make docs-check-server-libs`
- Режим: `Снапшот содержит актуальные версии и прямые ссылки на docs.rs без скачивания HTML.`

## Текущие версии и ссылки

| Crate | Version (`Cargo.lock`) | Docs.rs crate page | Rustdoc index | Local metadata |
|---|---:|---|---|---|
| `axum` | `0.8.9` | [crate](https://docs.rs/crate/axum/0.8.9) | [rustdoc](https://docs.rs/axum/0.8.9/axum/) | `apps/server/docs/upstream-libraries/axum/metadata.json` |
| `sea-orm` | `2.0.3` | [crate](https://docs.rs/crate/sea-orm/2.0.3) | [rustdoc](https://docs.rs/sea-orm/2.0.3/sea_orm/) | `apps/server/docs/upstream-libraries/sea-orm/metadata.json` |
| `async-graphql` | `7.2.1` | [crate](https://docs.rs/crate/async-graphql/7.2.1) | [rustdoc](https://docs.rs/async-graphql/7.2.1/async_graphql/) | `apps/server/docs/upstream-libraries/async-graphql/metadata.json` |
| `tokio` | `1.53.1` | [crate](https://docs.rs/crate/tokio/1.53.1) | [rustdoc](https://docs.rs/tokio/1.53.1/tokio/) | `apps/server/docs/upstream-libraries/tokio/metadata.json` |
| `serde` | `1.0.229` | [crate](https://docs.rs/crate/serde/1.0.229) | [rustdoc](https://docs.rs/serde/1.0.229/serde/) | `apps/server/docs/upstream-libraries/serde/metadata.json` |
| `tracing` | `0.1.44` | [crate](https://docs.rs/crate/tracing/0.1.44) | [rustdoc](https://docs.rs/tracing/0.1.44/tracing/) | `apps/server/docs/upstream-libraries/tracing/metadata.json` |
| `utoipa` | `5.5.0` | [crate](https://docs.rs/crate/utoipa/5.5.0) | [rustdoc](https://docs.rs/utoipa/5.5.0/utoipa/) | `apps/server/docs/upstream-libraries/utoipa/metadata.json` |

Для попытки скачать HTML-копии docs.rs используйте:

```bash
python3 scripts/server_library_docs_sync.py sync --download-html
```
