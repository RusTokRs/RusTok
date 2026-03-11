# Спецификация `rt_json_v1`

`rt_json_v1` — канонический JSON-формат rich-text для RusToK (blog/forum).

## Формат payload

```json
{
  "version": "rt_json_v1",
  "locale": "ru",
  "doc": {
    "type": "doc",
    "content": []
  }
}
```

- `version` обязателен и должен быть `rt_json_v1`.
- `locale` обязателен, валиден в формате `ll` или `ll-RR` и должен совпадать с locale запроса.
- `doc` обязателен и содержит дерево узлов.

## Allowed nodes

Поддерживаемые типы узлов:

- `doc`
- `paragraph`
- `heading` (`attrs.level` от 1 до 6)
- `bullet_list`
- `ordered_list`
- `list_item`
- `blockquote`
- `code_block`
- `horizontal_rule`
- `hard_break`
- `text`
- `image` (`attrs.src`)
- `embed` (`attrs.provider`, `attrs.url`)

## Allowed marks

Поддерживаемые marks:

- `bold`
- `italic`
- `strike`
- `code`
- `link` (`attrs.href`)

Для одного `text`-узла допускается не более 8 marks.

## Ограничения глубины и размера

- Максимальная глубина дерева: `8`.
- Максимальное число узлов: `2000`.
- Максимальный суммарный размер текстового содержимого: `100000` символов.

## URL / embed policy

- Для `link.attrs.href`: разрешены только `http`, `https`, `mailto`.
- Для `image.attrs.src`: разрешены только `http`, `https`.
- Для `embed` разрешены только провайдеры:
  - `youtube` (`youtube.com`, `www.youtube.com`, `youtu.be`)
  - `vimeo` (`vimeo.com`, `player.vimeo.com`)
- Для `embed.attrs.url` обязателен `https`.

## Unknown node/mark handling

- Unknown nodes и marks не сохраняются (drop во время sanitize).
- Если после sanitize документ становится пустым/некорректным — запрос отклоняется как validation error.


## Форматные версии

- `rt_json_v1` — текущая поддерживаемая версия для записи и рендера в backend.
- `rt_json_v2` — зарезервирована (known-but-unsupported): backend v1 её распознаёт, но отклоняет с явной ошибкой несовместимости.
- Любая другая версия считается неизвестной и отклоняется (`reject`).

## Версионирование и совместимость

- **Backward compatibility (legacy -> v1)**: если `version` отсутствует, backend пытается трансформировать payload в `rt_json_v1`:
  - если payload уже выглядит как `doc`, он оборачивается в `{"version":"rt_json_v1","locale":"<request-locale>","doc":...}`;
  - если payload — объект с `doc`, добавляются отсутствующие `version/locale`.
- **Forward compatibility (v2+ -> v1 backend)**: неизвестные версии (`version != rt_json_v1`) отклоняются (`reject`).

## Backend enforcement

В blog/forum клиентская валидация считается **advisory only**.

Каждый входящий rich-text JSON на backend проходит:

1. schema validation (`version/locale/doc`, allowed nodes/marks, limits, URL/embed policy),
2. sanitize (drop unknown nodes/marks, normalize attrs),
3. сохранение только sanitized JSON.

## План миграции `markdown -> rt_json_v1` (без ломающего релиза)

Миграция выполняется поэтапно, с сохранением обратной совместимости:

1. **Dual-write-ready API (текущий этап)**
   - DTO create/update принимают `body_format`/`content_format` и `content_json`.
   - Backend принимает оба формата: `markdown` и `rt_json_v1`.
   - Для rich-контента хранится sanitизированный `rt_json_v1`.
2. **Dual-read + legacy fallback**
   - При чтении старых записей без формата или с историческим markdown применяется fallback `markdown`.
   - Миграция БД на этом шаге не требуется.
3. **Фоновая конверсия исторических данных**
   - Batch-job конвертирует markdown-записи в `rt_json_v1` в фоне (tenant-by-tenant / module-by-module).
   - Для каждой записи сохраняется both audit trail и возможность безопасного retry.
4. **Gradual rollout по каналам записи**
   - UI/API клиенты постепенно переключаются на отправку `rt_json_v1`.
   - Метрики: доля `rt_json_v1` записей, ошибки валидации, sanitize-drop rate.
5. **Ограничение legacy writes (после saturation)**
   - После достижения целевого порога (>95% rich writes) вводится soft-warning для новых markdown-записей.
   - Жёсткое отключение markdown допускается только отдельным ADR и релиз-нотом.

Ключевой принцип: **read compatibility сохраняется на всех этапах**, поэтому нет необходимости в «big bang» миграции.
