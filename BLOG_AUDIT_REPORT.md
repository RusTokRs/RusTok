# Audit: логика блог-функционала RusToK

**Область:** `crates/modules/rustok-blog` (родной модуль: domain, services, controllers, GraphQL,
integrations, preview-токены, comment projection), оба UI (Leptos admin/storefront,
Next.js admin/storefront), мостовой оркестратор `rustok-content-orchestration`
(promote/demote), серверные адаптеры (`apps/server`).
**Основа:** ветка `arena/75049fe7-rustok`, commit `a94875e`.
**Критерии:** контракт `AGENTS.md` (6 Универсальных логических измерений, FBA/FFA, ownership,
§13 zero-legacy, §14 no-stubs / ания подавления ошибок).

---

## Итог

Слой domain и persistence блога написан **очень аккуратно**: состояние переходов по
хардкоду́ (4 ребра, без самопереходов), нормализация slug/route-key, keyset-курсоры с
ограничением длины токена, явная теневой политика читания, preview-токены только как
SHA-256, проекция комментариев с циклической защитой. Основные косяки сосредоточены
в **UI-координации (admin Leptos)** и в **мостовом оркестраторе**, где недостают
события/проекции, которые в обычном CRUD-пути есть.

| № | Ваш | Плоскость | Поражение |
|---|------|-----------|-----------|
| D1 | Высокий | Admin UI (Leptos) | Устаревший запрос сбрасывает busy-key никогда → редактор залипает навсегда |
| D2 | Средний | Оркестратор promote/demote | Мост не публикует события Blog → продвинутый пост не индексируется в поиске |
| D3 | Средний | Tag relations | Один «осиротевший» тег делает нечитаемым весь список постов |
| D4 | Средний | Comment port | Хардкодовый 2-секундный дедлайн для чтения и записи → фиктивный «недоступен» |
| D5 | Низкий | Оркестратор | Contract `moved_comments` всегда 0 — обманчивый API |
| D6 | Низкий | Storefront core | Неверно стояще док-комментарий |
| D7 | Низкий | RSS transport | Имена env-переменных и невалидированная вставка locale в путь |
| E1 | Средний | Admin UI (список постов) | Busy-key применялся ко всем строкам: любое действие отключало действия во всех строках |
| E2 | Низкий | Storefront UI (фильтры) | Гарблед-символ «Г—» вместо крестика закрытия у чипов фильтров |
| E3 | Низкий | Admin UI (DataGrid) | Битый комментарий про Effect, обновляющий pagination total |

---

## Найденные ошибки (первый проход — D1–D7)

## Дополнительные ошибки второго прохода (E1–E3)

### E1. Admin-список: busy-key применялся ко всем строкам, блокируя действия на всём списке
**Ваш:** Средний · **Плоскость:** UI/согласованность

**Место:** `crates/modules/rustok-blog/admin/src/core/presentation.rs` —
`blog_post_admin_table_row_view`.

**Что происходит.** Функция строила `is_busy` как `busy_key.is_some()`: при любом
в-летящем action (редактирование, сохранение, публикация поста А) **каждая** строка
списка получала `is_busy = true`, а в `components.rs` все кнопки строки
(`disabled=row.is_busy`) отключались. В результате при работе с одним постом весь
список «замораживался»: нельзя было открыть, опубликовать или удалить любой другой пост.
При этом в `commands.rs` уже существовал корректный пер-строковый хелпер
`row_is_busy_for_post` (проверяет последний сегмент busy-key на совпадение с post_id),
но он **не использовался** в презентации.

**Фикс.** `is_busy = row_is_busy_for_post(busy_key, post_id.as_str())` — строка уходит в
busy только тогда, когда busy-key принадлежит именно этому посту. Крейторийский
busy-key (`create`, без id) ни одну строку не блокирует. Добавлен unit-тест
`busy_key_is_scoped_to_its_own_row`.

### E2. Storefront: гарблед-символ «Г—» вместо крестика закрытия у чипов фильтров
**Ваш:** Низкий · **Плоскость:** UI

**Место:** `crates/modules/rustok-blog/storefront/src/ui/leptos/showcase.rs` —
три «удалить фильтр»-ссылки (категория, тег, поиск).

**Что происходит.** В шапке активных фильтров кнопки-исключения renderили литерал
`"Г—"` (бракованный символ), а не символ закрытия. Чипы показывали
запутанный текст вместо крестика.

**Фикс.** Замена на корректный символ закрытия `×` во всех трёх местах.

### E3. Admin UI: битый комментарий про обновление pagination total
**Ваш:** Низкий · **Плоскость:** DRY/ясность

**Место:** `crates/modules/rustok-blog/admin/src/ui/components.rs` —
`BlogPostsTable`, `pagination`.

**Что происходит.** Комментарий утверждал: «the effect below sets the total from the
rows on screen», но такого Effect нет. На практике leptos-DataGrid в client-side режиме
(без `on_page_change`) сам derive `total` из `data.len()` при рендере
(`rustok-grid/leptos/src/grid.rs:90-91`), поэтому исходный `total = 0` безвредно — но
комментарий вводил в заблуждение.

**Фикс.** Комментарий переписан так, чтобы описывать реальное поведение (grid сам
derives total), и не вводил будущего maintainer в ошибку.

---

## Найденные ошибки (первый проход)

### D1. Admin-редактор (Leptos): устаревший запрос стирает busy-key, но не сбрасывает — редактор блокируется навсегда
**Ваш:** Высокий · **Плоскость:** UI/согласованность

**Место:**
- `crates/modules/rustok-blog/admin/src/ui/leptos/lifecycle.rs` — `toggle_publish` (стр. 80–84, 111),
  `toggle_pin` (181–185, 211), `archive_post` (269–273, 299), `restore_post` (357–361, 387).
- `crates/modules/rustok-blog/admin/src/ui/leptos.rs` — `edit_post` (295), `submit_post` (509, 546),
  `delete_post` (634), `create_preview_link` (709).

**Что происходит.** Каждый lifecycle-callback ставит в начало `set_busy_key.set(Some(...))`,
выполняет запрос в `spawn_local`, а в конце — `set_busy_key.set(None)`. Для защиты от
«ответа устаревшего запроса» есть ветка `_ if !request.is_current(...) => return`. Но этот
`return` **выходит из async-блока до строчки сброса busy-key**. Таких устаревших
возвратов **17**, и все они пропускают `set_busy_key.set(None)`.

**Поражение.** `busy_key` — один глобальный сигнал на странице: он и блокирует все
callback (`if busy_key.get_untracked().is_some() { return; }`), и дизейблит поля формы
(`fields_disabled`). Достаточно, чтобы одна старая операция завершилась после того, как
пользователь уже начал новую (например: нажал «Publish» на пост A, быстро переключился
на пост B и нажал «Save» — completion-запроса A уже не актуален), как busy-key остаётся
`Some(...)` **на постоянной основе**. Редактор переходит в состояние «замороженный»:
кнопки и поля disabled, никакие действия не выполняются. Восстановление возможно только
по перезагрузке страницы.

**Фикс.** В каждом устарешем пути сбрасывать busy-key, но только если он всё ещё принадлежит
этому запросу (чтобы не сбросить busy-key нового, уже поставленного запроса):

```rust
// вместо `return;`
if set_busy_key
    .get_untracked()
    .is_some_and(|key| key == command.busy_key)
{
    set_busy_key.set(None);
}
return;
```

Идеально — сделать busy-key «запросно-скоупленным» (токен запроса + проверка текущего),
а не одной общей строки. Также стоит добавить regression-тест на «стареющий completion
не оставляет busy-key зажатым».

---

### D2. Оркестратор promote/demote не публикует события Blog — продвинутые посты не попадают в поиск/проекции
**Ваш:** Средний · **Плоскость:** Архитектура/события

**Место:** `crates/modules/content-orchestration/src/bridge/promote.rs` (`create_promoted_post_in_tx`
insert + `sync_promoted_post_tags_in_tx`, без `publish_blog_reindex_in_tx`),
`crates/modules/rustok-blog/src/services/orchestration.rs` (`create_promoted_post_in_tx`,
`delete_demoted_post_in_tx`).

**Что происходит.** Путь обычного создания/редактирования поста через `PostService`
публикует события (`DomainEvent::ReindexRequested`, `BlogPostUpdated` и др.) и запускает
проекции (поиск, счётчик комментариев). Мост forum→blog (`promote_topic_to_post`) делает
raw-insert через `create_promoted_post_in_tx`, синхронизирует теги через
`sync_post_tags_in_tx`, меняет маршруты — **но ни одного доменного события не публикует**:
ни `ReindexRequested`, ни `BlogPostCreated`. Аналогично при демоции
(`delete_demoted_post_in_tx`) не публикуется сигнал о удалении/ресайндексации.

**Поражение.**
- Продвинутый пост **не попадает в поисковую индексацию** до тех пор, пока какой-либо другой
  маршрут (например, правка тега или категории, которая публикует reindex для всего
  модуля) случайно не запустит переиндексацию. То есть продвинутая статья может быть
  невидима в поиске.
- Проекции, полагающиеся на события Blog (например, доставка `comment_count` /
  `blog_comment_projection_delivery`), не видят создания продвинутого поста.
- При демоции удалённая статья может остаться в поисковом индексе до следующего reindex.

**Фикс.** Inside `promote_topic_to_post` (в той же транзакции, после создания поста)
опубликовать `DomainEvent::ReindexRequested { target_type: "blog", target_id: Some(post_id) }`,
и, по желанию, доменное событие создания/продвижения. В `delete_demoted_post_in_tx` —
событие ремайнда/удаления. Лучше вынести сам publish в `create_promoted_post_in_tx` /
`delete_demoted_post_in_tx`, чтобы мост не мог его пропустить.

---

### D3. Один «осиротевший» тег делает нечитаемым весь список постов
**Ваш:** Средний · **Плоскость:** Robustness / availability

**Место:** `crates/modules/rustok-blog/src/services/tag/relations.rs`,
`load_post_tags_map` (стр. ~409–448), ветка `return Err(BlogError::invariant(...))`.

**Что происходит.** При подготовке tag-колонки для списка постов `load_post_tags_map`
читает все `blog_post_tag`-relations и по терминам, не найденным в
`load_tags_by_ids`, бросает `BlogError::invariant("Blog post {id} references missing
Taxonomy tag {id}")`.

**Поражение.** orphan-relation (тег удалён прямо через Taxonomy без cleanup блога, или
запись осталась после сбоя) не просто «скрывает тег» — она **роняет весь список постов**
(admin-лист и storefront-украшение тегами) с внутренней ошибкой (500). Одна Bad Row
парализует чтение. Это fail-closed по умолчанию, но здесь не пользовательская ошибка,
а внутреннее расхождение данных — для него целесообразна локальная деградация
(пропустить тег + `tracing::warn`), а не отказ всего запроса.

**Фикс.** Заменить `return Err(invariant)` на: пропустить relation, прологировать warning
с tenant/post/tag id, оставить список читаемым. Optional: добавить background-чистку
орфанных relations + метрика `blog.orphan_tag_relations_total`.

---

### D4. Хардкодовый 2-секундный дедлайн для чтения и записи комментариев
**Ваш:** Средний · **Плоскость:** Perf/UX contract

**Место:** `crates/modules/rustok-blog/src/services/comment.rs` —
`comments_public_read_port_context` (стр. 567) и `comments_port_context` (стр. 608):
`.with_deadline(Duration::from_secs(2))`.

**Что происходит.** Один и тот же 2s budget используется и для публичного чтения
комментариев, и для всех порт-вызовов (чтение + запись).

**Поражение.** При обычной нагрузке Comments-провайдера чтение начнёт падать в
`Timeout`/`Unavailable` → storefront показывает «комментарии временно недоступны»,
хотя провайдер просто слегка медленный. Запись при 2s budget может фоллить, хотя
комментарий в итоге бывает создан. Никакого конфига, и нет различия между бюджетом
чтения и записи.

**Фикс.** Вынести budget в configurable (модульные настройки / host-конфиг), разделить
`read_deadline` и `write_deadline`. Добавить метрики попадания в таймаут и эмпирически
поднять порог; в качестве degradation-линии дать storefront-у «замедленное, но живое»
состояние вместо полного «недоступно».

---

### D5. Контракт `moved_comments` всегда 0 — обманчивый API
**Ваш:** Низкий · **Плоскость:** Contract/dead code

**Место:** `crates/modules/content-orchestration/src/bridge/promote.rs`
(`move_forum_replies_to_comments_in_tx`, стр. 233–244; `moved_comments: reply_records.len()`,
стр. 228) и `bridge/demote.rs` (`move_comments_to_forum_replies_in_tx`, стр. 242–252;
`moved_comments: comment_records.len()`, стр. 188).

**Что происходит.** `move_*` — guard-stub, который при наличии данных возвращает
validation-ошибку («нельзя перенести, пока Forum не перейдёт на canonical richtext»). Поэтому
код никогда не доходит до вывода: `moved_comments` **всегда 0**, `active_comments`
(promote, стр. 98–100, 140) всегда 0.

**Поражение.** Поля в `PromoteTopicToPostOutput` / `DemotePostToTopicOutput`
и `comment_count` продвинутого поста — формально «верные», но на практике всегда 0, что
вводит в заблуждение API-потребителей и маскирует функциональный пробел (продвижение
постов с комментариями заблокировано, а об этом говорит только текст ошибки).

**Фикс.** Either доделать transfer (смотри D2/guard-события) и сделать `moved_comments`
реальным, либо убрать поле/временно пометить `#[deprecated]`/вернуть ошибку
`unsupported(reason)` вместо «успешного» результата с нулевым счётчиком. Важно не
оставлять «успех со нулевым исходом» как тихий обман.

---

### D6. Неверно стоящий док-комментарий
**Ваш:** Низкий · **Плоскость:** Documentation

**Место:** `crates/modules/rustok-blog/storefront/src/core/routing.rs` стр. 103–106.

**Что происходит.** Комментарий

```rust
/// Preserve the selected post and other active filters while changing one filter.
/// Cursor state is deliberately excluded: a cursor belongs to the old filter set.
pub fn feed_href(locale: Option<&str>) -> String {
```

описывает **`filter_query_href`**, а стоит над `feed_href`. `feed_href` просто
строит `/{locale}/blog/feed.xml`.

**Фикс.** Переносить комментарий к `filter_query_href`; для `feed_href` написать
собственное описание.

---

### D7. RSS-транспорт: имена env-переменных и невалидированный locale в пути
**Ваш:** Низкий · **Плоскость:** Transport

**Место:** `crates/modules/rustok-blog/storefront/src/transport/feed.rs` стр. 54 и 71.

**Что происходит.**
- Серверный Rust-код читает `NEXT_PUBLIC_STOREFRONT_URL` / `NEXT_PUBLIC_SITE_URL` — prefix
  `NEXT_PUBLIC_*` в Next.js означает «встраивается в клиентский бандл». На сервере это
  просто env-var, но такая нотация вводит в заблуждение (и risk: если в том же процессе
  идёт сборка Next-приложения, переменная может быть клиентской).
- `locale` не валидируется перед вставкой в путь:
  `root.join(&format!("/{locale}/modules/blog"))`. С `Url::join` ввод вроде `../foo`
  или символов с песочницей может привести к неожиданному resolved-path.

**Фикс.** Использовать server-имя переменной (`RUSTOK_STOREFRONT_URL`); пропускать через
валидатор locale (или `percent_encode`/`Url::from`-конструкцию), чтобы путь не был
уязвим к path-traversal-style входам.

---

## Рекомендации по улучшению (не ошибки, но улучшения)

1. **Целевая ресайндексация вместо полной.**
   `publish_blog_reindex_in_tx` в служебных путях тегов/категорий публикует
   `ReindexRequested { target_id: None }` — переиндексация **всего модуля**. Для редактирования
   одного тега/поста оптимальнее `target_id: Some(post_id)` (как делают команды поста).
   Снизит нагрузку на индексацию и ускорит отклик.

2. **Конфигурируемость бджета комментариев.**
   Разделить deadline на read/write (см. D4) и поднять с запасом; добавить метрики
   `blog.comments_port_timeout_total{direction}`.

3. **Очистка orphan-relations.**
   Для D3 добавить periodic-job (или event-driven cleanup) по удалению `blog_post_tag`
   за прошедшие N-дней с empty `blog_tag_usage`, чтобы не накапливаться Bad Row.

4. **Идемпотентность мостового продвижения.**
   В `promote_topic_to_post` явно переиспользовать `command_id`-идемпотентность и
   `lock_tag_attachments_writer` (уже есть), но также зафиксировать `slug` claim
   before insert, чтобы при рокете не оставалось «промежуточное» состояние route/slug.

5. **Аудит дубликатов тегов в UI.**
   `parse_ui_csv` убирает пустые, но **не дедуплицирует** теги. `sync_post_tags_in_tx`
   дедуплицирует на уровне term-id, поэтому избыток не ломает, но в админ-списке и
   storefront-отображении теги могут дублироваться из-за ввода
   `rust, rust`. Дедуплицировать на этапе `parse_tags`.

6. **Покрытие паритета dual-UI.**
   У Leptos-storefront и Next.js-storefront есть сходные правила (коммент-пейджинг,
   `show_section`/`can_reply`, переключение `posts_after` vs `q`). Лучше явно зафиксировать
   эти правила в одном чистом модуле и сослаться из обоих UI, чтобы при изменении
   политики не забыть второй UI (для admin это уже сделано через `core/`, для storefront
   частично — но Next.js storefront дублирует часть правил в TS).

7. **Тесты на устаревшие completion.**
   Для D1: unit-тест для `LifecycleCallbacks`/`submit_post`, подтверждающий, что после
   устаревающего completion busy-key очищен. Для D3: интег-тест с orphan relation,
   подтверждающий, что список всё ещё отдаётся с warn.

---

## Что в порядке (позитивные проверки)

- **State machine поста:** `BlogPostStatus` {Draft, Published, Archived}, ровно 4 ребра,
  без самопереходов; `published_at` ставится лишь при первой публикации и не сбрасывается;
  публичный corpus = `status=published AND published_at IS NOT NULL`.
- **Слой чтения:** fail-closed tenant-override, каналная видимость решается на уровне читателя,
  keyset-курсоры с явным ограничением длины токена (`MAX_LIST_CURSOR_TOKEN_BYTES`) и
  отказом legacy-токенов.
- **Preview-токены:** хранятся только как SHA-256; plaintext выдаётся один раз;
  ограничение количества и eviction/cleanup с clamp; токены аннулируются в той же транзакции
  при выходе из draft.
- **Проекция комментариев (`comment_projection.rs`):** событийно-driven, с циклической
  защитой, monotonic projection_revision, идемпотентная доставка, derived-counter не трогает
  бизнес-версию (не ломает CAS-токены), unknown-status fail-closed.
- **Условия жизни UI:** stale-request-guards (несмотря на D1) реализованы последовательно;
  admin-graphql-адаптеры и storefront-адаптеры (native + graphql) согласованы
  (`per_page: 6`, `COMMENTS_PAGE_SIZE`).
- **RSS:** корректная CDATA-эскейп, atom:link self, проверка origin и channel gating;
  publish-only visible posts.
- **Второй проход (UI + storefront):** UI-слои admin (page/components/lifecycle/moderation/
  categories/richtext) и storefront (showcase/selected_post_card/comments/draft_preview/
  share/published_posts_list), транспорт-адаптеры и core-презентации проверены на
  согласованность. Зафиксированы E1–E3; остальной поток данных (tenant-gate,
  cross-tenant binding, channel-visibility, comment-snapshot, author-profiles,
  cursor-guard на load-more, поиск-маршаллинг, пагинация комментариев) корректен.
