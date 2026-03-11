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
3. сохранение только sanitized JSON,
4. повторный sanitize/normalization перед HTML-рендером (defense in depth).
