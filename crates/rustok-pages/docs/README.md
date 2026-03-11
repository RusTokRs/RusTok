# rustok-pages docs

В этой папке хранится документация модуля `crates/rustok-pages`.

## Documents

- [Implementation plan](./implementation-plan.md)

## Event contracts

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)

## API contract notes

- `PageBodyInput` поддерживает `format=markdown|rt_json_v1`.
- Для `markdown` обязательное поле — `content` (непустой текст).
- Для `rt_json_v1` ожидается `content_json`; `content` можно использовать как raw JSON fallback для совместимости клиентов.
- Перед записью payload проходит server-side sanitize/validation через `rustok_core::prepare_content_payload`.
- `BlockService` валидирует `data` по `BlockType` (schema-first DTO payload) и отклоняет неизвестные поля.
- Для `Video`/embed и URL-полей действует whitelist policy: `http/https` для ссылок, а embed только `https` + домены `youtube|youtu.be|vimeo|player.vimeo.com`.
- Для `Html` блоков запрещены опасные теги/протоколы (`<script>`, `<iframe>`, `javascript:`) и inline event handlers (`on*=`).
