# Аудит логики Page Builder RusToK — ошибки, косяки и рекомендации

**Дата:** 2026-11-10
**Ветка:** `arena/c37b5f4e-rustok` (base `dd9b1d4`)
**Объём:** `crates/modules/rustok-page-builder` (123 файла, ~38.5k LOC), движок `crates/ui/fly` + `fly/browser` (~45k LOC), хост-интеграция `crates/modules/rustok-pages` (`services/page/*`, `admin/*`), `apps/admin/src/main.rs` (intent endpoint), Next.js поверхности `apps/next-admin/packages/pages` и `apps/next-frontend/packages/rustok-pages`, ADR 2026-07-13 / 2026-10-09 / 2026-10-14 / 2026-10-16, backlog `docs/modules/page-builder-backlog.md`.
**Метод:** статический проход по исполняемым путям (publish-пайплайн, санитизация, intent-протокол, draft-сессии, авторизация, undo/history, кодек, рантайм действий, scheduled publish), проверка 6 Universal Logic Dimensions (AGENTS.md §2.1). Изменений в коде не делал; тесты и компиляцию не запускал — это чисто ревью, все ссылки на файлы/строки проверены по текущему HEAD.

> Отчёт — не фикс. Каждый пункт снабжён путём к файлу и строкой. Приоритеты: **P1** — функциональный баг / безопасность / конкурентность, **P2** — качество, производительность, целостность контрактов, **P3** — заметки.

---

## 1. Executive Summary

Page Builder — самая зрелая подсистема из аудированных мной в этом репозитории. Пайплайн публикации образцовый: fail-closed политика, канонические SHA-256 хэши, верификация целостности артефактов, повторная валидация после runtime-материализации, идемпотентные receipts, CAS по ревизиям, транзакционный outbox. Экранирование HTML/CSS/JSON-LD корректно во всех проверенных местах.

Тем не менее найдено **7 дефектов уровня P1** и ~16 позиций P2/P3. Главные темы:

* **Авторизация построена на строках ролей, а не на каноническом RBAC** — прямое нарушение инварианта `rustok-rbac` из AGENTS.md. Кастомные роли с реальными грантами `pages:update` получают read-only, а роль `manager` может редактировать, но **не может сохранить ни одного изменения** (каждая правка падает с Forbidden на автосейве).
* **Синхронная расходимость двух сторфронтов**: рантайм действий на Next.js — подмножество рантайма на Rust; табы/аккордеоны/слайдеры, собранные в билдере, на Next.js не работают.
* **`srcset` валидируется ~150 строками грамматики, но рендерер его выбрасывает** — валидация мертва относительно выхода, README утверждает обратное.
* **Конкурентность draft-сессий атомарна только в памяти**: файловый и KV бэкенды — классический check-then-act, CAS по generation проигран на мультиподной сборке.
* **Scheduled publish может навсегда застрять в `executing`** после крэша воркера — рипера нет.

Ниже — находки, сгруппированные по доменам.

---

## 2. Авторизация и безопасность (P1)

### 2.1. Авторизация по имени роли вместо канонического RBAC [P1]

**Где:** `crates/modules/rustok-pages/admin/src/access.rs:30-46` (`pages_editor_permissions_for_role`: `Some("super_admin") | Some("admin") => full`, `manager => без publish`, прочее — read-only), `crates/modules/rustok-pages/admin/src/builder.rs:419-426` (`page_builder_permissions_for_role` синтезирует `Vec<Permission>` из строки роли), `apps/admin/src/main.rs:96-99`.

**Проблема:** инвариант AGENTS.md (Layer 1, `rustok-rbac`): *«MUST NOT hardcode role checks (`if role == "admin"`) or local permission tables»* — здесь это сделано дословно. Реальный набор разрешений пользователя (`rustok-rbac`) никогда не запрашивается: `fetch_current_user` возвращает только `role`, из неё синтезируются `Permission::new(Resource::Pages, …)`.

Последствия:
- кастомная роль `editor` с настоящим грантом `pages:update` получает read-only в билдере — функционально сломано и молча;
- переименование ролей ломает редактор незаметно (unknown → read-only);
- постраничная гранулярность (owner-scope) на этом уровне отсутствует — её спасает только `enforce_owned_scope` в самом `PageService` (data-слой проверяет корректно, см. `services/page/document.rs:52-57`);
- fail-closed для unknown ролей — единственное, что сделано правильно.

**Рекомендация:** в intent-endpoint'е и фасаде запрашивать эффективные разрешения пользователя через канонический RBAC-порт и маппить `CapabilityState` из них (`pages:read` → чтение, `pages:update` → edit/properties/assets, `pages:publish` → publish). Ролевой маппинг оставить только как явно задокументированный fallback для хостов без RBAC-порта.

### 2.2. Роль `manager`: редактировать можно, сохранить нельзя [P1]

**Где:** `crates/modules/rustok-pages/admin/src/browser_intent.rs:155-173` (синтезированный `save`), `crates/modules/rustok-page-builder/admin/src/capability_access.rs:135` (`BrowserIntentKind::Save => vec![EditorCapability::Publish]`), `builder.rs:419-426` (manager → только `pages:read`).

**Проблема:** цепочка. (1) Preflight пропускает мутирующий интент — у manager `edit = true`. (2) После мутации `result.dirty == true` → хост **синтезирует** intent `save` (автосейв каждой правки). (3) Синтезированный конверт диспатчится через `dispatch_browser_intent` **минуя** `preflight_pages_intent` (нет ни capability-проверки, ни palette-проверки — см. §2.3). (4) Фасад доходит до `AuthorizedPageBuilderHandlers` → publish требует `pages:publish` → **Forbidden**. Итог: каждая правка менеджера возвращается ошибкой; полезной работы роль выполнить не может, а ошибка приходит поздно и не объясняет причину.

**Рекомендация:** развести семантику: сохранение черновика — это `pages:update` (draft-save), реальная публикация — `pages:publish`. В автосейве пропускать синтез `save`, если у пользователя нет соответствующей capability, оставляя канвас dirty с индикатором «не сохранено». И прогонять синтезированный конверт через тот же preflight.

### 2.3. Синтезированный save обходит capability/palette preflight [P1→P2 после фикса прав]

**Где:** `crates/modules/rustok-pages/admin/src/browser_intent.rs:155-173`; серверный контроллер канваса создаётся с полными правами: `crates/modules/rustok-page-builder/admin/src/model.rs:38` (`FlyUiStateMachine::new(Presentation::Full)`).

**Проблема:** единственное место, где `CapabilityState` пользователя проверяется — preflight **оригинального** конверта (`contribution_browser_intent.rs:84-95`). Серверный `AdminCanvasController` всегда Full. Любой новый путь, диспатчащий интенты в обход preflight (сегодня — синтезированный save; завтра — любой новый хост, вызывающий публичный `dispatch_pages_browser_intent` с `CapabilityState::full()` из `contribution_browser_intent.rs:38-41`), исполняется без capability-гейта. Защита остаётся только на фасаде (permission) — но она (а) не знает про palette-доступ, (б) падает поздно и без типизированного denial-ответа.

**Рекомендация:** сузить `Presentation`/capability у серверного контроллера до запрошенных; единая точка входа `dispatch(intent)` должна валидировать capability для **каждого** конверта, включая синтезированные.

### 2.4. `page_builder_permissions_for_role` никогда не выдаёт `pages:update` [P2, латентно]

**Где:** `crates/modules/rustok-pages/admin/src/builder.rs:419-426`; контракт возможностей: `crates/modules/rustok-page-builder/src/service.rs:23-46` (`Properties` требует `pages:update`); семантика импликации: `crates/libs/rustok-api/src/permissions.rs:284-287` (`Manage` имплицирует всё, `Publish` **не** имплицирует `Update`).

**Проблема:** список синтезированных прав содержит только `Read` и (для admin) `Publish`. Capability `Properties` (`pages:update`) недостижима ни для одной роли через `AuthorizedPageBuilderHandlers`. Сегодня латентно (фасад Pages маршрутизирует только Preview/Publish), но первый же хост, отправляющий Properties-запрос через `compose_fly_page_builder_handlers`, получит вечный Forbidden.

### 2.5. Draft-сессии не привязаны к пользователю; создание сессий read-only пользователями [P1]

**Где:** `crates/modules/rustok-page-builder/admin/src/draft_session.rs:15-24` (запись не содержит user/tenant), `:293-296` (load проверяет только `page_id`); `crates/modules/rustok-pages/admin/src/browser_intent.rs:189-193` (безусловный `commit_with_context` в конце **каждого** успешного интента, включая read-only `select` пользователя с ролью `customer`).

**Проблема (две):
1. **Ресурсная амплификация:** каждый `select` от аутентифицированного читателя создаёт новую draft-сессию с полным контроллером (весь проект). Лимит стора — только количество записей (10 000) без ограничения размера записи; проект до 16 МБ (`max_project_bytes`) → теоретически ~160 ГБ в памяти/на диске от одного низкопривилегированного пользователя.
2. **Отсутствие владельца:** токен — единственный «credential» сессии. Два редактора одного тенанта, обменявшиеся токеном (логи, referrer, скриншот), молча перехватывают чужой несохранённый черновик; аудит-атtribution отсутствует. Cross-tenant недоступен (страница fetched токеном вызывающего) — поэтому P1, а не P0.

**Рекомендация:** привязать сессию к владельцу: добавить в запись `tenant_id` + `user_id`, проверять при load/commit; не создавать сессию для пользователей без edit-capability; ввести лимиты на размер записи и на количество сессий на пользователя.

### 2.6. Файловый draft-стор пишет контент тенанта в OS temp dir [P2]

**Где:** `crates/modules/rustok-page-builder/admin/src/draft_session.rs:270-273` (`default_dir() = /tmp/rustok_draft_sessions`), `:1049-1071` (`atomic_write_file` — remove-then-rename, окно без файла при крэше; `fs::write` с дефолтными правами 0644).

**Проблема:** несохранённые черновики страниц (в т.ч. непубличный контент) лежат 8 часов в world-readable `/tmp` без tenant-скопинга; любой локальный процесс может их читать/подменять/удалять. На Unix `fs::rename` перезаписывает атомарно — предварительный `remove_file` не нужен и снижает durability.

**Рекомендация:** каталог хранилища — конфигурируемый, с правами 0700/0600 и per-tenant подкаталогами; убрать pre-remove на Unix (оставить только в Windows-ветке); добавить `fsync` перед rename.

---

## 3. Функциональные баги ядра (P1)

### 3.1. `srcset` валидируется, но никогда не рендерится [P1]

**Где:** валидация — `crates/modules/rustok-page-builder/src/static_publish_policy.rs:553-570` + `validate_srcset` (~100 строк грамматики, дескрипторы `w`/`x`, запрет `data:`, уникальность); рендер — `crates/ui/fly/src/render.rs:503`: атрибут `srcset` безусловно выбрасывается из выхода вместе с `style`/`srcdoc`/`xlink:href`/`ping`/`background`. При этом `sizes` **не** выбрасывается — в опубликованный HTML попадает осиротевший `sizes` без `srcset`.

**Проблема:** адаптивные изображения проходят publish-политику (README модуля прямо обещает «default policy permits validated `srcset`»), но в артефакте `srcset` исчезает молча. Это (а) потеря функциональности, (б) мёртвый код валидации, (в) рассинхрон документации и поведения, (г) непоследовательность `sizes`/`srcset`.

**Рекомендация:** добавить srcset в рендерер с той же per-URL проверкой (`UrlAttributeKind::ResourceImage`), либо до тех пор — выбрасывать и `sizes`, и убрать обещание из README. Покрыть end-to-end тестом «srcset в проекте → srcset в артефакте».

### 3.2. Next.js storefront: рантайм действий — подмножество Rust-рантайма [P1]

**Где:** Rust (полный): `crates/ui/fly/browser/assets/fly-actions.js` — 7 видов действий (`emit_event`, `provider_action`, `toggle_visibility`, `switch_tab`, `toggle_accordion`, `switch_slide`, `scroll_to`, `animate`) + touch/swipe + fallback `panel_id` в `findTarget`. Next.js (подмножество): `apps/next-frontend/packages/rustok-pages/src/components/fly-actions-runtime.ts` — только `emit_event`/`provider_action`/`toggle_visibility`/`scroll_to`/`animate`; нет `switch_tab`/`toggle_accordion`/`switch_slide`, нет touch-жестов, нет `panel_id`-fallback'а.

**Проблема:** компоненты «табы», «аккордеон», «слайдер», собранные в билдере, на Rust-сторфронте работают, на Next.js — мёртвые (клик ничего не делает). Это прямое нарушение Dual-Implementation Parity (AGENTS.md §5.6) и дублирование source of truth: два рантайма уже разошлись и продолжат расходиться.

**Рекомендация:** сделать `fly-actions.js` единым каноническим артефактом, который Next-пакет потребляет как статический ассет (копия в сборке с хэш-сверкой против Rust-версии), а не переписывает на TS. Добавить parity-тест, сравнивающий множество поддерживаемых `kind` в обоих рантаймах.

### 3.3. Scheduled publish: задача может навсегда застрять в `executing` [P1]

**Где:** `crates/modules/rustok-pages/src/services/page/schedule.rs:229-260` — claim это оптимистичный `scheduled → executing`; выборка задач — только `state = "scheduled"` (`:236`); никакого рипера/lease/timeout для `executing` по всему модулю нет (grep подтверждает).

**Проблема:** крэш пода после claim'а и до `published/failed` оставляет job в `executing` навсегда — публикация страницы не происходит и не повторяется. Нарушение State Transition Integrity («no entity can become permanently stranded in processing upon mid-chain failure»).

**Рекомендация:** lease-поле (`claimed_at`/`lease_until`), рипер, возвращающий просроченные `executing` в `scheduled` (с учётом `attempts < MAX`), и метрика на stranded-состояние.

---

## 4. Конкурентность и целостность (P1/P2)

### 4.1. CAS по generation в draft-сторе атомарен только в памяти [P1]

**Где:** in-memory — `draft_session.rs:169-224` (единый `RwLock`, корректно); файл — `draft_session.rs:521-577` (`commit_with_context`: read-file → check → write-file, без блокировок); KV — `draft_session.rs:737-752` (`save_record` — безусловный `set`, без compare-and-set); дефолт хоста `DurableSsrDraftSessionStore` (`:896-901`) = memory + file.

**Проблема:** классический TOCTOU. Два пода читают generation N, оба проходят проверку `expected_generation`, оба пишут N+1 — lost update, защита от конфликта поколений молча обходится. На single-pod инсталляции работает (память), на мультиподной — нет.

**Рекомендация:** для KV — `SETNX`-подобный условный запись/Lua-CAS по (token, generation); для файла — `flock` или полный переход на KV-бэкенд в мультиподной сборке; тест на гонку двух «подов» (в тестах стор уже эмулируется двумя инстансами — `pod1_store`/`pod2_store`, добавить сценарий одновременного commit).

### 4.2. publish_reviewed держит DB-локи во время CPU-тяжёлой компиляции [P2]

**Где:** `crates/modules/rustok-pages/src/services/page/reviewed_publish.rs:68-95` — `find_page_for_update` (эксклюзивный лок строки страницы), затем внутри транзакции: parse JSON → sanitize → materialize → hash по каждой локали.

**Проблема:** по бенчмаркам backlog'а validate ~300 мс на 10k узлов; на большой мультиязычной странице транзакция с эксклюзивным локом живёт секунды. Конкуррентные редакторы/публикаторы одной страницы сериализуются; растёт риск lock-таймаутов и очередей на «горячих» страницах.

**Рекомендация:** компилировать из reviewed-снапшота под shared-локом вне эксклюзивной транзакции, затем короткая эксклюзивная фаза: перепроверить `expected_body_revisions`/version и выполнить bind/switch. Альтернатива — pre-compile на save и публикация уже staging-артефакта.

### 4.3. `RevisionState::mark_changed` сбрасывает `save_failed` [P3]

**Где:** `crates/ui/fly/src/command/model.rs:331-338`.

**Проблема:** после неудачного сохранения первая же новая правка стирает индикатор отказа (`save_failed = false`), хотя изменения всё ещё не на сервере. `dirty` остаётся — но UI-сигнал «последний сейв упал» теряется.

---

## 5. Производительность (P2)

### 5.1. Каждый intent отвечает полным JSON проекта + page-fetch на каждый intent [P2]

**Где:** `crates/modules/rustok-page-builder/admin/src/browser_intent.rs:502-516` (`result()` — `GrapesJsCodec::encode_value` всего документа в каждом ответе, включая `select`/`hover`); `crates/modules/rustok-pages/admin/src/browser_intent.rs:73-77` (`transport::fetch_page` — HTTP round-trip на каждый intent, включая не-мутирующие).

**Проблема:** на странице в 1 МБ каждый клик в редакторе = fetch страницы + полная сериализация проекта + полный project_data в ответе (сеть). Автосейв каждой правки (dirty → save) добавляет ещё и полный save round-trip. Для интерактивного редактирования это главный масштабный барьер.

**Рекомендация:** для не-мутирующих интентов — lightweight-ответ (selection, hash, sequence) без project_data; мутирующие — дифф или клиентская реконструкция из effects; кэшировать fetch_page в пределах generation сессии.

### 5.2. Двойной clone+apply на slow-path `FlyEditor::apply` [P3]

**Где:** `crates/ui/fly/src/command/editor.rs:103-150` — speculative clone+apply в fast-path выполняется, а при `validate_patch_incremental → None` выбрасывается и повторяется заново на полном пути. Корректно (клоны независимы), но это лишняя полная копия+применение команды на каждый не-Patch fallback.

### 5.3. `InMemorySsrDraftSessionStore::load` берёт write-lock ради TTL-refresh [P3]

**Где:** `draft_session.rs:293-296`; `DurableSsrDraftSessionStore::commit_with_context` (`:996-1001`) использует `memory.load` как staleness-пробу — с побочным эффектом продления TTL. Мелочь, но на горячем пути чтения — лишняя эксклюзивность.

---

## 6. Целостность контрактов и поведение (P2)

### 6.1. «Настраиваемая» publish-политика не существует в рантайме [P2]

**Где:** README модуля: «operators can explicitly ban it through `forbidden_attributes`»; код: `static_publish_policy.rs:281-285` — `validate_static_publish_document` всегда `PageBuilderStaticPublishPolicy::default()`; `verify_integrity` evidence (`:217-231`) прибит к хэшу дефолта. Не-дефолтные конструкции политики встречаются только в тестах (`:1705`, `:1735`).

**Проблема:** поля `forbidden_attributes`/`forbidden_css_tokens` сериализуемы и задокументированы как операторские ручки, но ни один путь не загружает настроенную политику — и evidence-верификация её отвергла бы. Документация обещает несуществующее.

**Рекомендация:** либо реализовать загрузку политики из tenant settings (с включением хэша политики в evidence, как уже спроектировано), либо убрать заявления про операторскую настройку из README/комментариев.

### 6.2. Preview допускает то, что publish отвергает [P2]

**Где:** `crates/modules/rustok-pages/admin/src/builder.rs:569-573` — `PagesPageBuilderRenderer::render_preview` использует `RenderPolicy::default()` (`allow_http: true`), тогда как статическая публикация отвергает `http://` (`static_landing.rs:186-189`). Аналогично `load_project` (`:484-505`): ошибка инъекции шаблона глотается `if let Ok(...)`, превью молча деградирует к нетемплированному проекту.

**Проблема:** автор видит HTTP-картинку в превью и поздний сбой публикации — ровно тот «late, confusing publish failure», против которого написаны комментарии в самой политике (про `url(` и CSS-когерентность). Молчаливая деградация шаблона в превью скрывает поломку композиции.

**Рекомендация:** единая политика превью/публикации (`allow_http: false`) + диагностика вместо молчаливого fallback'а шаблона.

### 6.3. Выполняется только первый Request-effect [P2]

**Где:** `crates/modules/rustok-pages/admin/src/browser_intent.rs:174` — `requests.into_iter().next()`; остальные запросы молча отбрасываются. Сегодня контракт «≤1 запроса на диспатч» соблюдается эмпирически, но это не asserted — добавление интента, порождающего 2+ Request-effect, даст тихую потерю второго.

**Рекомендация:** либо явно `debug_assert`/error на >1 запроса, либо исполнить все (последовательно, с обработкой ошибок).

### 6.4. Ошибка сохранения оставляет клиент расходившимся с сервером [P2]

**Где:** `crates/modules/rustok-pages/admin/src/browser_intent.rs:174-188` — при ошибке фасада весь обработчик возвращает Err: draft не коммитится, `reload` не возвращается. Мутирующий интент уже применён в браузерном канвасе (оптимистично), серверная сессия откатилась к прошлому состоянию — клиент и сервер расходятся до перезагрузки страницы пользователем.

**Рекомендация:** на ошибку save возвращать структурированный ответ (не HTTP-ошибку) с `reload: true`/`revert_to` + revision, чтобы рантайм канваса восстановил серверное состояние.

### 6.5. Trait-метод `commit()` стирает runtime_context в File/Durable сторах [P2, латентно]

**Где:** `draft_session.rs:515-519` (File) и `:975-982` (Durable) — `commit` делегирует `commit_with_context(..., Value::Object(Map::new()))`, что **перезаписывает** сохранённый контекст пустым объектом; in-memory `commit` (`:112-118`) контекст **сохраняет**. Продакшн-вызовы используют `commit_with_context`, но trait-контракт модульно-зависим — мина для следующего хоста.

**Рекомендация:** в `commit` передавать `None`-семантику (сохранить прежний контекст) во всех бэкендах; зафиксировать тестом на всех трёх сторах.

### 6.6. `DurableSsrDraftSessionStore::prune` возвращает `max` вместо суммы [P3]

**Где:** `draft_session.rs:1053-1060` — `Ok(mem.max(persistent))` — статистика занижает число удалённых записей.

### 6.7. `FileSsrDraftSessionStore::load` глотает сбой продления TTL [P3]

**Где:** `draft_session.rs:505` — `let _ = self.save_snapshot(&snapshot)`: сессия не продлилась, но клиент об этом не узнает и через TTL молча потеряет черновик.

---

## 7. Next.js-сторфронт: детали (P2)

### 7.1. `scopeLandingCss` — рукописный CSS-токенайзер и `@scope`-зависимость [P2]

**Где:** `apps/next-frontend/packages/rustok-pages/src/components/css-scope.ts`.

**Проблема:**
- `@scope` поддерживается только современными браузерами (Chrome 118+/Safari 17.4+/Firefox 128+); на старых — CSS лендинга выйдет за контейнер и поломает shell хоста (деградация, не безопасность);
- hand-rolled парсер: `@keyframes` не входит в `isCssGroupRule`, но выживает за счёт балансировки стека; экранирование кавычек `trimmed[i-1] !== "\\"` некорректно для `\\\\"`; `passthrough` при уже-`@scope` префиксе — без проверки корректности;
- `contain: style layout` не содержит CSS-стили (`style` — только counters/quotes) — комментарий в коде обещает больше, чем делает.

**Рекомендация:** fallback для старых браузеров (postcss-префиксация селекторов) или явный QGIS-порог поддержки; заменить ручной парсер на постпроцесс селекторов через проверенный парсер (у Next-стека есть готовые), либо минимизировать грамматику (сегодня Rust-политика уже ограничивает CSS — можно транслировать селекторы механически).

### 7.2. `parseArtifactHtml` — regex-парсинг HTML [P3]

**Где:** `apps/next-frontend/packages/rustok-pages/src/api/artifact.ts:20-35` — regex для `<style>`/`<body>`. Работает на артефактах, которые генерирует Rust-рендерер (предсказуемая форма), но хрупок к любому изменению огибающей артефакта. Лучше передавать body/css отдельными полями из GraphQL (они уже есть в `StaticLandingPage`), а не вырезать regex'ом.

---

## 8. Замечания к структуре (P2/P3)

1. **FFA-расхождение:** 34 файла в `admin/src/editor/` смешивают Leptos `view!` с доменной логикой на верхнем уровне адаптера, тогда как канонический layout — `core/` (нейтрально) + `ui/<framework>`; нейтральное ядро есть (`model.rs`, `core/session.rs`, `draft_session.rs`), но бо́льшая часть поверхности — фреймворк-связанная. Dioxus запаркован (задокументировано) — ок, но Leptos-слой стоит продолжать мигрировать в workspaces (`fly-leptos FlyEditorWorkspace`), как планировал backlog.
2. **Семантика «Publish = save»:** capability `Publish` в сервисном слое фактически выполняет **сохранение черновика** (`PagesPageBuilderProjectStore::save_project` → `save_page_document`), а реальная публикация — отдельный `publish_reviewed` через Pages GraphQL. Именование сбивает с толку и уже привело к багу §2.2. Рекомендуется переименовать capability в `Save`/`Persist` на следующем major-этапе (zero-legacy позволяет).
3. **`PortContext.tenant_id` несёт slug:** `ensure_port_context` сравнивает `context.tenant_id.as_str() != tenant_slug` — поле называется `tenant_id`, содержит slug. Работает, но противоречит канонической типизации тенанта.
4. **`next-admin/packages/pages`:** hardcoded Tailwind-палитра (`bg-emerald-100` и т.п.) вместо общих design-token классов — мелочь против токен-контракта.
5. **`PagesBuilderFacade::with_provider_flags`** помечен `#[expect(dead_code)]` с честным комментарием — ок по §14, но «retires the attribute»-семантика сработает только при появлении вызова; следить.

---

## 9. Что сделано хорошо (сохранить как эталон)

- **Санитизация/публикация:** fail-closed политика с полным покрытием (теги, атрибуты, URL-виды, CSS-токены, media-запросы, srcset/sizes грамматика), канонические хэши, `verify_integrity` на каждом уровне, повторная проверка **после** runtime-материализации (`static_landing.rs:130-140`), ресурсные лимиты до рекурсивного прохода.
- **Экранирование:** `push_escaped_html`/`push_escaped_attribute` без исключений; `escape_style_element_text` (raw-text breakout через `\00003c`); JSON-LD `</script>`-эскейпинг; `escape_css_attribute` allow-list; `strip_validated_css_urls` с корректным порядком (data-URL с `;` до структурной проверки) — редкая аккуратность.
- **Транзакция публикации:** idempotency receipts с exact-replay, CAS по body-ревизиям и версии страницы, `FOR UPDATE`, транзакционный outbox, immutable staging артефактов, квоты размеров.
- **Undo/history:** Arc-sharing before/after (память ~½), инвертируемые команды, байтовый бюджет истории, `VecDeque`-эвикция, iterative depth-check в кодеке (стек-оверфлоу исключён), сохранение GrapesJS frames без второго дерева-авторитета.
- **CSRF:** деривация SHA-256 от session-id, constant-time compare, fail-closed на пустом секрете (`verify_csrf_token` отвергает `""`).
- **Canvas iframe:** `sandbox` без `allow-same-origin`, подписка прибита к expected origin, protocol+instance+sequence валидация сообщений.
- **Каталог символов:** CAS через revision-токен на save + `assert_reviewed_symbol_snapshot` на publish —lost-update между редакторами пойман.
- **Scheduler-claim:** оптимистичный `scheduled→executing` с фильтром по состоянию — конкурентные воркеры безопасны (не хватает только рипера, §3.3).

---

## 10. Приоритетный план исправлений

| # | Что | Приоритет | Оценка |
|---|-----|-----------|--------|
| 1 | RBAC-маппинг вместо строк ролей (§2.1) + выдать `pages:update` (§2.4) | P1 | средняя |
| 2 | Менеджер/автосейв: развести save-draft и publish (§2.2), preflight для синтезированного save (§2.3) | P1 | малая |
| 3 | Единый action-runtime для обоих storefront'ов (§3.2) | P1 | малая-средняя |
| 4 | Рендер `srcset` (§3.1) + e2e-тест | P1 | малая |
| 5 | Рипер `executing`-задач публикации (§3.3) | P1 | малая |
| 6 | CAS для durable draft-стора (§4.1) + binding user/tenant (§2.5) + лимиты сессий | P1 | средняя |
| 7 | Lightweight-ответы интентов + кэш fetch_page (§5.1) | P2 | средняя |
| 8 | Компиляция вне эксклюзивного лока (§4.2) | P2 | средняя |
| 9 | Согласовать preview/publish политику (§6.2) и убрать/реализовать «настраиваемую» политику (§6.1) | P2 | малая |
| 10 | Обработка ошибки save без расхождения клиент/сервер (§6.4) | P2 | малая |
| 11 | Файловый стор: права, атомарность, per-tenant (§2.6) | P2 | малая |
| 12 | Parity-тест рантаймов, srcset, stranded-job, draft-race — добавить в scoped-тесты соответствующих крейтов | — | — |

**Верификация:** аудит статический; код не изменялся, `cargo check/test` и Playwright-наборы не запускались. Все ссылки на файлы:строки сверены с HEAD `dd9b1d4`. Перед фиксами из таблицы — соответствующие scoped-проверки: `cargo test -p rustok-page-builder -p rustok-page-builder-admin -p rustok-pages`, `npm run verify:fly-interaction-capabilities`, parity-скрипты из `.github/workflows/pages-page-builder-parity.yml`.

## 11. Журнал реализации фиксов

### Фикс 1 (2026-11-10): RBAC-маппинг вместо строк ролей (§2.1) + выдача `pages:update` (§2.4) — РЕАЛИЗОВАН

Root cause: UI-тип `rustok_ui_auth::AuthUser` не нёс permissions, и все потребители (pages access/builder, intent-эндпоинт, CSR-фолбэки) угадывали капабилити по role-строке; порт Page Builder получал `[pages:read]` без `pages:update`, поэтому Properties-капабилита (save) всегда падала в Forbidden у не-админов, а менеджеры не видели вкладки авторинга.

Реализация (21 файл, единая сквозная семантика «эффективные permissions»):

- **Контракт UI**: `rustok_ui_auth::AuthUser` + `permissions: Vec<String>` (`#[serde(default)]` — старые пейлоады десериализуются в пустой снапшот = fail-closed). Туда же контракт-тест обратной совместимости.
- **Транспорты leptos-auth**: `permissions` добавлены в GraphQL `CurrentUserQuery`/`AuthUserGraphql`, REST `RestUserInfo`/`RestUserResponse`, cookie `CookieUserInfo` + все маппинги в `AuthUser`. Покрыты все пути получения юзера: GraphQL `me`, REST `/api/auth/me`, REST login/register/refresh, cookie session login/register/refresh, SSR `exchange_refresh_token_ssr`.
- **Сервер**: `UserInfo`/`UserResponse` (rustok-auth rest.rs) + `permissions`; резолв внутри транзакции выпуска сессии (`AuthLifecycleService::resolve_effective_permission_strings` через `get_user_permissions_authoritative`, как и `resolve_effective_role`) → поле в `AuthTokens`; GraphQL-провайдер переиспользует `tokens.permissions` (удалены дублирующий резолв и post-issuance compensation — они стали структурно не нужны); `me`/`update_profile` отдают снапшот из `CurrentUser` (авторитетный + отфильтрованный по доступности модулей).
- **pages-admin**: `access.rs` — `pages_editor_permissions_from_effective(&[String])` (manage→всё; update→авторинг; publish→только publish; иначе read-only) + `effective_permissions_grant` (зеркало `has_effective_permission`: точное совпадение или resource-manage); builder.rs — порт получает `[read] + update? + publish?` из единого маппинга (§2.4 закрыт: Properties теперь авторизован `pages:update`); composition.rs / inline_edit_launch.rs / lib.rs / contract_tests.rs / CRATE_API.md переведены на новый API; ролевые функции удалены (zero-legacy).
- **apps/admin**: intent-эндпоинт (main.rs) — политика из `verified_user.permissions`; CSR-фолбэк contribution host — вычисляет granted-подмножество как SSR server fn (убран role-гейт `super_admin|admin`).

Верификация: песочница без Rust-тулчейна, а `static.rust-lang.org`/`crates.io` вне allowlist хостов — компиляция здесь невозможна. Выполнено строгое статическое ревью: полный проход по `git diff` всех 21 хунков; grep-полнота всех конструкторов `AuthUser`/`AuthTokens`/`UserInfo`/`UserResponse` (включая тесты); подтверждение, что `create_user_in_tx` создаёт role-биндинги в той же транзакции (резолвер видит их); удалённые функции нигде не упоминаются. **`cargo check/test` НЕ запускались** — мейнтейнеру/CI: `cargo check -p rustok-ui-auth -p leptos-auth -p rustok-pages-admin -p rustok-admin -p rustok-server -p rustok-auth && cargo test -p rustok-ui-auth -p rustok-pages-admin`.

Follow-up (вне скоупа фикса, новое наблюдение): сайдбар админки (`apps/admin/src/widgets/app_shell/sidebar.rs:81`) гейтит группу «Management» по role-строке — нужен per-section permission map (users:read / modules:read / …), отдельный пункт плана.

Следующий по плану: фикс 2 — развести save-draft и publish (§2.2) + preflight синтезированного save (§2.3).

### Фикс 2 (2026-11-10): разводим save-draft и publish (§2.2) + preflight синтезированного save (§2.3) — РЕАЛИЗОВАН

Root cause: «save» в редакторе сквозь моделировался как publish (4 слоя: машина состояний `RequestSave` → require `publish`; кнопка Save в тулбаре; `BrowserIntentKind::Save`/`EditorShortcut::Save` → `EditorCapability::Publish`; порт `BuilderCapabilityKind::Publish` → `pages:publish`), при этом бэкенд-обработчик — `PageService::save_document` — на самом деле сохраняет working copy и сам требует `pages:update` (для опубликованной страницы публичный контент не меняется до отдельного publish_reviewed). И синтезированный автосейв диспатчился мимо capability-проверок.

Реализация (10 файлов, семантика «save = авторинг = pages:update; publish — отдельный флоу»):

- **fly_ui (machine.rs)**: `UiIntent::RequestSave` требует `edit` вместо `publish` (+ сообщение диагностики «save blocked…»). Тест переписан: автор (edit без publish) сохраняет; publish-only ревьюер — не мутирует и не сохраняет (publish-капабилитета остаётся независимой для настоящего publish-флоу — это прямо документировано в `CapabilityState::normalized`).
- **rustok-page-builder-admin (capability_access.rs)**: `BrowserIntentKind::Save` и `EditorShortcut::Save` → `vec![EditorCapability::Edit]`. Тесты переписаны: менеджер-профиль проходит save-preflight; restricted-профиль получает типизированный denial `edit`.
- **toolbar.rs**: кнопка Save гейтится по `capabilities.edit`, а не `publish`.
- **rustok-pages/admin (browser_intent.rs)**: `capabilities: CapabilityState` протаскивается через оба диспатча; SSR-контроллер сужается per-request через `UiIntent::SetEditableCapabilities` (контроллер создаётся с Full — теперь каждый конверт, идущий через его машину состояний, гейтится таблицей капабилитетов вызывающего); синтезированный автосейв валидируется `validate_browser_capability_access` и **пропускается** (не ошибка) при отказе — канвас остаётся dirty, клиент показывает индикатор «не сохранено».
- **contribution_browser_intent.rs**: обе with_capabilities-обёртки передают capabilities дальше; тест preflight переписан под новую семантику. browser_problem.rs: тест problem-контракта отражает реальную форму denial (`edit`).
- **builder.rs (фасад Pages)**: композиция порта переведена на `compose_configured_fly_page_builder_handlers` с кастомным авторизером: `publish: pages:update` (зеркалит собственный enforcement `save_document`), `properties: pages:update`, `preview/tree: pages:read`. Шаренный дефолт (publish→pages:publish) не тронут — хосты, чей сторе реально публикует, сохраняют его. Реальная публикация (`publish_reviewed`) по-прежнему требует `pages:publish` в модуле Pages.
- **TS parity**: моки denial в `apps/next-admin/tests/e2e/fly-browser-capability-problem.spec.ts` и `fly-browser-response-order.spec.ts` обновлены на `requires editor capability \`edit\`` / `capability: 'edit'`.

Сквозная цепочка после фикса (менеджер с `pages:update`): preflight пропускает мутирующий интент (edit) → автосейв валиден (edit) → SSR-машина пропускает (edit) → порт авторизован (pages:update) → бэкенд `save_document` (pages:update + owned scope). §2.2 закрыт полностью; §2.3 — синтезированный конверт проходит тот же capability-гейт, серверный контроллер сужен до запрошенных прав.

Верификация: как и в фиксе 1 — компиляция в песочнице невозможна (нет тулчейна, crates.io заблокирован). Статическое ревью: полный проход по `git diff` (10 файлов), проверка согласованности capability-таблиц (машина и preflight используют один `CommandCapabilityRequirement`), `validate_navigation_intent` не rejecting SetEditableCapabilities, idempotentность сужения между запросами draft-сессии, отсутствие остаточных упоминаний старой семантики (grep по `requires editor capability \`publish\`` и save/publish-линкам). **`cargo test -p fly-ui -p rustok-page-builder-admin -p rustok-pages-admin` и Playwright-наборы (`fly-browser-capability-problem`, `fly-browser-response-order`) НЕ запускались** — оставить CI. Путь синтеза автосейва (транспорт-зависимый) не покрыт unit-тестом — кандидат на integration-тест в CI.

Следующий по плану: фикс 3 — единый action-runtime (§3.2).

### Фикс 3 (2026-11-10): единый канонический action-runtime для обоих сторфронтов (§3.2) — РЕАЛИЗОВАН

Root cause: Next-пакет `@rustok/pages-frontend` не потреблял канонический рантайм действий, а содержал TS-переписывание (`fly-actions-runtime.ts`, 194 строки), которое поддерживало только 5 из 8 видов действий: отсутствовали `switch_tab`, `toggle_accordion`, `switch_slide` (+ touch/swipe-жесты, `panel_id`-fallback в `findTarget`, синхронизация aria-selected/is-active, exclusive-режим аккордеона, `globalThis.FlyActions`). Компоненты «табы/аккордеон/слайдер» на Rust-сторфронте работали, на Next.js — были мёртвыми; два рантайма продолжали расходиться.

Реализация (10 файлов: семантика «один канонический артефакт — `crates/ui/fly/browser/assets/fly-actions.js`; Next потребляет его байт-в-байт»):

- **`components/fly-actions.js`** (новый): byte-exact копия канонического ассета (sha256 `460bcd6e…`, `cmp` = 0). Ассет — самостартующий idempotent IIFE с `typeof document`/`typeof globalThis`-guard'ами → безопасен как side-effect import в SSR-контексте клиентского компонента; `ADAPTER_KEY = Symbol.for("fly.actions.adapter")` гарантирует один listener-set на документ.
- **`components/fly-actions.d.ts`** (новый): типы для side-effect модуля (`tsc` с `allowJs: false` + `moduleResolution: "bundler"` резолвит `./fly-actions.js` → `.d.ts`; сам `.js` в `include` не попадает и не компилируется) + типизация глобального API `FlyActions { start, handleAction, parseAction }` как `| undefined` (forced feature-detect).
- **`components/fly-action-bridge.tsx`**: удалён `initFlyActions`/импорт TS-рантайма; вместо него `import "./fly-actions.js"` в module scope. Остальная логика bridge (cart/navigate/islands/link-intercept) не менялась. В `packages/rustok-pages/package.json` нет `"sideEffects": false` → webpack не выкинет side-effect import.
- **`components/fly-actions-runtime.ts`**: УДАЛЁН (−194 строки; единственный импортёр — bridge). Grep: упоминания `initFlyActions`/`fly-actions-runtime` в коде отсутствуют, обновлены README.md и CRATE_API.md пакета.
- **`apps/next-frontend/scripts/verify-fly-actions-parity.mjs`** (новый): (1) byte-exact сравнение копии с crates-оригиналом (Buffer.equals + sha256 в сообщении об ошибке); (2) извлечение множества `case "kind":` из ассета и сверка с `ComponentAction::kind()` из `crates/ui/fly/src/action/model.rs` минус нативные `navigate_page`/`navigate_url`/`submit_form` (материализуются как href/form, до рантайма не доходят) — новый Rust-kind без runtime-поддержки валит проверку и наоборот; (3) bridge обязан side-effect-импортировать ассет и не ссылаться на удалённый рантайм; (4) `fly-actions-runtime.ts` не должен существовать (защита от resurrection); (5) `.d.ts` существует.
- **`package.json`**: скрипт `verify:fly-actions-parity` + включён в цепочку `test` (после `verify:seo-runtime-fixtures`).
- **`eslint.config.mjs`**: vendored-ассет в ignores — линт/autofix не должен менять байты (парity — byte-for-byte).
- **`.github/workflows/fly-page-builder.yml`**: шаг «Verify fly-actions asset parity with Next.js storefront» (`node apps/next-frontend/scripts/verify-fly-actions-parity.mjs`, скрипт путерезолится через `import.meta.url` — cwd-независим) + paths-триггеры на копию ассета и verifier в push/pull_request.
- **`pages-frontend.test.mjs`**: тест «fly-actions runtime is the canonical asset and handles every non-native action kind» — byte-parity + pinned-множество из 8 kinds на уровне `test:pages`.

Верификация (выполнено в песочнице, node доступен): `node scripts/verify-fly-actions-parity.mjs` → OK (8 runtime kinds = 11 ComponentAction − 3 native); `node --experimental-strip-types --test packages/rustok-pages/src/pages-frontend.test.mjs` → **9/9 pass** (было 8). Мутационное тестирование verifier'а: (а) однобуквенная правка копии → падение с sha256-диффом и инструкцией re-copy; (б) фейковый `Self::MysteryKind` в Rust enum → падение «missing case "mystery_kind"»; обе мутации откачены, `git status` канона/model.rs чист. Статическое ревью compile-рисков: tsconfig-резолв `.js`→`.d.ts`, отсутствие `sideEffects:false` в пакете, SSR-guard'ы ассета, BOM workflow сохранён. **`tsc --noEmit`, `next build`, `eslint`, Playwright НЕ запускались** (node_modules не устанавливается в песочнице) — CI: `npm --prefix apps/next-frontend run verify:fly-actions-parity && npm --prefix apps/next-frontend run test:pages && npm --prefix apps/next-frontend test && npm --prefix apps/next-frontend run typecheck`; Rust-сторона не менялась (`cargo test -p fly-browser` — существующий контракт `action_runtime_contract.rs`).

Замечание по дизайну: физически ассет дублируется (vendored copy), но verifier делает дрейф структурно невозможным для мержа — это ровно рекомендация §3.2 («копия в сборке с хэш-сверкой против Rust-версии»), альтернативы (public/ + next/script, webpack ?raw) отклонены ранее как ломающие пакетную инкапсуляцию/требующие next.config.

Следующий по плану: фикс 4 — рипер/lease для `executing`-задач публикации (§3.3; `schedule.rs:229-260` — claim scheduled→executing без таймаута, выборка рипера только `state='scheduled'`). srcset §3.1 (пункт 4 таблицы §10) остаётся в очереди.

### Фикс 4 (2026-11-10): lease + рипер для `executing`-задач публикации (§3.3) — РЕАЛИЗОВАН

Root cause: claim в `process_due_publish_jobs_as_of` — оптимистичный `scheduled → executing` без lease/таймаута, а выборка задач (и, значит, любая повторная обработка) фильтрует только `state='scheduled'`. Крэш пода между claim'ом и терминальным апдейтом оставлял job в `executing` навсегда: публикация не происходила и не повторялась — нарушение State Transition Integrity («no entity can become permanently stranded in processing upon mid-chain failure»).

Реализация (9 файлов: семантика «claim держит lease; свип сначала жнёт просроченные leases»):

- **Миграция** `m20261110_000001_add_page_publish_job_lease_until`: nullable `lease_until TIMESTAMPTZ` + partial index `idx_page_publish_jobs_lease ON page_publish_jobs (lease_until) WHERE state='executing'` (Postgres + SQLite, одинаковый синтаксис). Backfill-контракт `pages-page-publish-job-lease` (mode `none`) добавлен в `docs/migrations/backfill-contracts.json` — обязательный по CI-верификатору `verify-migration-backfill-contracts.mjs` (каждая appended-миграция требует контракт).
- **Entity**: `lease_until: Option<DateTimeWithTimeZone>` (NULL на `executing`-строке = claim до появления колонки; рипер трактует как просроченный).
- **schedule.rs**:
  - `PAGE_PUBLISH_JOB_LEASE_SECS = 600` — lease 10 минут ≫ худшего времени `publish_reviewed`; replay безопасен (захваченная команда несёт `idempotency_key`).
  - Claim (`scheduled → executing`) теперь пишет `lease_until = now + lease`; все терминальные/повторные переходы (published, failed, retry→scheduled) и requeue чистят lease.
  - `reap_expired_publish_jobs_as_of(now)`: просроченные `executing` (lease ≤ now ИЛИ NULL) → `scheduled` при `attempts < MAX`; терминальный `failed` при `attempts ≥ MAX`; терминальный `canceled`, если для страницы уже есть более новый `scheduled`-job — пока job «висел», автор мог создать новый schedule (pending-строки нет → `schedule_publish` вставляет новую), и requeue старого нарушил бы partial unique index `uq_page_publish_jobs_active`; новый schedule выигрывает. Каждый переход — оптимистичный (UPDATE re-check'ает state и просроченность lease; `rows_affected == 0` → пропустить), с маркером `PAGE_PUBLISH_JOB_LEASE_EXPIRED` + сообщением о восстановлении.
  - `sweep_due_publish_jobs_as_of(now, limit) → PagePublishSweepOutcome { published, reaped }` — канонический свип «reap → claim → execute»; рипер встроен в свип, поэтому любой хост свипа самовосстанавливается без отдельного процесса. Прежние `process_due_publish_jobs` / `process_due_publish_jobs_as_of` — обёртки с прежними сигнатурами (u64 published), существующие вызовы не тронуты.
  - **Метрики** (по образцу outbox `RelayMetrics`): `PagePublishSchedulerMetrics` (AtomicU64) + `PagePublishScheduler::metrics() → PagePublishSchedulerMetricsSnapshot { sweeps_total, published_total, requeued_total, reaped_failed_total, superseded_total }` — `requeued_total` детектирует crash-recovery, `reaped_failed_total` — stranded-публикации, требующие внимания оператора.
- **Реэкспорты** (services/page/mod.rs, services/mod.rs): outcome-типы, снапшот метрик, константы lease. API-ответы (`PagePublishScheduleResponse`) не менялись.
- **Доки**: README модуля (обязанности + behavior notes) и `DECISIONS/2026-10-09-scheduled-page-publishing.md` (Failure semantics: lease-семантика и три исхода рипера; Migration: вторая append-only миграция; Verification: покрытие рипера).

Тесты (`crates/modules/rustok-pages/tests/page_publish_scheduler_sqlite.rs`, +7 к 8 существующим = 15): один свип самовосстанавливает stranded-задачу и публикует её (requeued=1 → published=1, attempts=2, lease сброшен, success очищает маркер); requeue с маркером `PAGE_PUBLISH_JOB_LEASE_EXPIRED`; живой lease не трогается; NULL lease трактуется как просроченный; attempts=MAX → терминальный `failed`, позже не воскрешается; superseded → `canceled`, новый schedule исполняется; метрики шедулера считают свипы/восстановления и не растут при повторном свипе без strand'ов. Крэш симулируется helper'ом `strand_job` (прямая постановка `executing` + lease + attempts).

Верификация: компиляция в песочнице невозможна (тулчейна нет, crates.io вне allowlist). Статическое ревью: полный проход по диффу; оба литерала `page_publish_job::ActiveModel` в `schedule_publish` дополнены `lease_until: Set(None)` (иначе literal не компилируется); `Expr::value(Option<DateTimeWithTimeZone>)` подтверждён по документации sea-query 1.0.2 (`From<DateTime<FixedOffset>> for Value` + generic `From<Option<T>> where T: Nullable`); `Condition::any().add(...)` сверен с локальной идиомой (не `.filter`); `ConnectionTrait` импортирован в миграцию (по образцу соседней); rustfmt-моделирование (max_width 100 / fn_call_width 60 / chain_width 60) сверено с эмпирикой соседнего кода, строк > 100 нет. Прогнаны доступные node-гейты: `node scripts/verify/verify-adrs.mjs` (178 решений, ок), `node scripts/verify/verify-migration-backfill-contracts.mjs --self-test` (ок), `node scripts/verify/verify-migration-compatibility-contract.mjs` (ок). **`cargo fmt/clippy/test` НЕ запускались** — CI: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p rustok-pages --test page_publish_scheduler_sqlite` (workspace-покрытие — через `cargo llvm-cov nextest --workspace --all-targets`).

Замечания по дизайну: (1) рипер встроен в свип, а не отдельным процессом — единая точка восстановления, pub-метод `reap_expired_publish_jobs_as_of` доступен для ops; (2) гонка «старый воркер дописывает после requeue+re-claim» закрыта идемпотентностью `publish_reviewed` (idempotency_key) + lease ≫ худшего publish; (3) `latest_publish_job`/`page_publish_schedule` не exposes lease — поверхность API не расширена намеренно.

Следующий по плану: фикс 5 — CAS для durable draft-стора (§4.1) + binding user/tenant (§2.5) + лимиты сессий. srcset §3.1 (пункт 4 таблицы §10) остаётся в очереди.

### Фикс 5 (2026-11-10): CAS для durable draft-стора (§4.1) + owner-binding и лимиты сессий (§2.5) — РЕАЛИЗОВАН

Root cause (две независимые дыры одного стора): (1) TOCTOU — файловый `commit_with_context` делал read-file → check → write-file без блокировок, KV `save_record` — безусловный `set`; два пода читали generation N, оба проходили `expected_generation`, оба писали N+1 → lost update, оптимистичная блокировка молча обходилась на мультиподе; (2) запись сессии не содержала владельца — токен был единственным «credential» (обмен токеном = перехват чужого черновика), а безусловный commit в конце каждого интента (включая read-only `select` от `customer`) + отсутствие лимитов размера записи давали ресурсную амплификацию до ~160 ГБ на одного низкопривилегированного пользователя.

Реализация (12 файлов; семантика «CAS по числу generation + владелец в записи + лимиты; legacy-записи без owner остаются читаемыми»):

- **`rustok-page-builder-admin/draft_session.rs`** (ядро):
  - `DraftSessionOwner { tenant_id, user_id }` (Serialize/Deserialize, PartialEq) + `owner: Option<...>` в `SsrDraftSessionSnapshot`/`PersistentDraftSessionRecord` (`serde(default)`, skip-if-none — wire-совместимо со старыми записями). Хелпер `ensure_session_owner(record, caller)`: attributed-запись принимает только точно такого же caller'а → `OwnerMismatch`; `None`-запись (legacy/внутренние пути) пропускает всех; первый attributed commit **усыновляет** владельца (adoption) — после этого анонимные caller'ы блокируются.
  - Трейт `SsrDraftSessionStore`: `load/commit/commit_with_context/remove` получают `owner: Option<&DraftSessionOwner>` последним параметром; `OwnerMismatch` — opaque (не светит user_id/tenant_id).
  - **KV CAS**: `DraftSessionKvDriver::set(key, value, cas, ttl)` + новый обязательный `compare_and_set(key, expected_cas, value, cas, ttl) -> bool` — CAS по **числу generation** (byte-стабильность сериализации AdminCanvasController не гарантирована из-за HashMap-порядка в FlyEditor/FlyUiStateMachine), `expected_cas: None` = SETNX-семантика «ключ отсутствует». `MemoryKvDriver` хранит `(bytes, cas, expires)`; реальный Redis-драйвер реализуется WATCH/MULTI или Lua без изменения контракта. `ExternalKvDraftSessionBackend::compare_and_save_record(expected: Option<&record>, new)` прокидывает `record.generation` как expected-cas.
  - **Файловый CAS**: `with_token_lock(dir, token, action)` — lock-файл через `OpenOptions::create_new` (O_EXCL; только std, без новых зависимостей — crates.io недоступен), stale-break при mtime > 30 s (крэшнувший холдер), wait-deadline 2 s (→ `LockTimeout`), poll 10 ms; имя файла безопасно — токен уже валидируется `is_safe_token` (≤64, `[A-Za-z0-9_-]`). Под локом — семантическое сравнение (token, generation, page_id, owner). Обновление существующей записи (`commit_existing_under_lock`) выполняется только под локом; входы «отскакивают» обратно в caller'а через `TokenLockCommit::Missing`, если запись исчезла/истекла (ownership-раунд-трип без клонирования контроллера). `remove` — тоже под локом (не может удалить свежезаписанную запись или воскресить удалённую); TTL-refresh в `load` — через `compare_and_save_record` (load больше не может регрессировать чужой коммит старой генерацией).
  - **Durable CAS-флоу**: load_record → fail-fast (page/owner/generation) → seed memory → `memory.commit` → `backend.compare_and_save_record(expected = загруженная запись, new)`; при проигрыше — reseed memory из авторитетной записи бэкенда + `GenerationConflict{expected: актуальная, actual: caller'а}`. Новая сессия: `expected = None` → set-if-absent.
  - **Лимиты (§2.5c)**: `MAX_DRAFT_SESSION_RECORD_BYTES = 20 MiB` (против 16 MiB max_project_bytes; проверка `ensure_record_size` через serde_json::to_vec на File/KV `save_record` и KV CAS) → `RecordTooLarge{bytes, maximum}`; `MAX_DRAFT_SESSIONS_PER_OWNER = 8` — eviction старейших same-owner сессий в InMemory (insert + commit), File `save_record` для новых записей (scan), KV `prune_records` (group-by-owner).
  - InMemory: `DraftEntry.owner`, per-owner cap с eviction по `expires_at`, owner-чек в `load`/`commit_internal`/`remove`, adoption в commit.
- **`rustok-pages/admin`**: `PagesBuilderSaveSnapshot` + `owner: Option<DraftSessionOwner>` (5 конструкторов: endpoint — `Some`; builder/composition/lib/metadata — `None`); `browser_intent.rs`: owner в `load`/`remove`/`commit_with_context`; **read-only гейт** — при `capabilities.edit == false` commit не вызывается вовсе (ответ эхом возвращает загруженный `draft_token`/`draft_generation` либо `""`/`0`; пустой токен фильтруется envelope-нормализацией при следующем load) — read-only `select` больше не создаёт сессию; `composition.rs`: SSR-restore строит owner из `current_user`+`tenant` (без аутентификации — `None`, legacy-поведение); `browser_problem.rs`: `OwnerMismatch` → HTTP 403 (opaque detail), `RecordTooLarge` → 413, `LockTimeout` → 503; реэкспорт `DraftSessionOwner` из lib.rs.
- **`apps/admin/main.rs`**: intent-endpoint строит `Some(DraftSessionOwner { tenant_id: tenant_slug, user_id: verified_user.id })` — только verified user, никаких client-supplied идентификаторов.
- **Доки**: CRATE_API.md обоих крейтов (owner-binding семантика, CAS-контракты, лимиты, новые коды ошибок).

Тесты (draft_session.rs: +13 к существующим; browser_problem.rs: +1): owner mismatch на load/commit/remove + cross-tenant; legacy `None`-запись читаема attributed-caller'ом + adoption запирает анонимов; durable owner-чек по бэкенд-записи при пустом memory-кэше (перезапуск пода); per-owner cap InMemory (10 коммитов → выживают последние 8) и File; `RecordTooLarge` на 20 MiB+ runtime context; атомарность `MemoryKvDriver::compare_and_set` (win/stale/SETNX); файловая гонка два-пода-один-каталог (один выигрывает gen 2, второй `GenerationConflict{2,1}`, lock-файлы убраны); KV-гонка pod1/pod2 (выигравший gen 2, проигравший конфликт + reseed памяти); **детерминированная интерливинг-гонка** `RacingKvDriver` (test-драйвер вклинивает чужой коммит прямо перед CAS) — CAS-проигрыш на пути, недостижимом последовательными вызовами, + reseed из winner-записи; fail-fast по перечитанному бэкенду после чужого коммита; прямые CAS-контракты обоих бэкендов (create/setnx-lose/update-win/stale-lose/owner-lose). Все 35 существующих вызовов стора (33 в тестах draft_session.rs, 1 в contract_tests.rs, 1 в тесте browser_intent.rs) обновлены на новую арность.

Верификация: компиляция в песочнице невозможна (тулчейна нет, crates.io вне allowlist — поэтому lock через std `OpenOptions::create_new`, а не fs2/flock). Статическое ревью: полный проход по диффу; баланс скобок/паренов проверен программно; ownership-анализ замыканий (`commit_existing_under_lock` возвращает входы через `TokenLockCommit::Missing` — use-after-move исключён); rustfmt-моделирование (max_width 100 / fn_call_width 60 / chain_width 60) — удлинившиеся после `+owner` цепочки разбиты вертикально; единственный внешние call-site'ы стора (pages/admin ×5, main.rs, contract_tests.rs) все обновлены, иных имплементоров трейтов в репо нет. **`cargo fmt/clippy/test -p rustok-page-builder-admin -p rustok-pages-admin` НЕ запускались** — CI: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p rustok-page-builder-admin -p rustok-pages-admin`.

Замечания по дизайну: (1) CAS-токен — число generation, а не хэш байтов: Redis-пути (WATCH/MULTI, Lua) не зависят от недетерминированной serde-сериализации контроллера; (2) legacy `None`-записи не инвалидируются массово — мягкая миграция через adoption при первом attributed commit; (3) read-only гейт на уровне dispatch, а не стора: внутренние пути с `CapabilityState::full()` сохраняют прежнее поведение, а `PagesBrowserIntentResponse` для read-only остаётся валидным (эхо токена); (4) lock-файл — advisory, защищает только кооперирующие процессы RusTok; stale-break 30 s > любых операций под локом; (5) per-owner cap File-стора сканирует каталог только при создании новой записи (update не меняет счётчик).

**Ревью-проход (2026-11-10, глубокий self-review фикса 5 до запуска CI):** перечитаны все диффы целиком; найдено и исправлено 5 дефектов:
1. *Критичный — слои*: `FileSsrDraftSessionStore::delete_record` делегировал в owner-проверяющий `remove(token, None)` → `Durable::remove` owned-сессии ВСЕГДА падал бы с `OwnerMismatch` даже для законного владельца. `delete_record` переписан как безусловный бэкенд-примитив под токен-локом (owner-проверка остаётся в слое durable-store, где ей место).
2. *Лимиты*: `ensure_record_size` мерил compact-сериализацию, а файловый бэкенд пишет pretty — файл мог превысить 20 MiB кап. Хелпер переведён на фактические байты: `ensure_record_size(json_bytes.len())` после соответствующей сериализации (pretty — File, compact — KV) в трёх точках записи.
3. *InMemory*: re-seed существующего токена через `insert_snapshot` (durable-синхронизация памяти) ошибочно эвиктил чужую same-owner сессию — теперь per-owner eviction только для genuinely новых токенов (`!entries.contains_key`).
4. *Учёт*: KV `prune_records` считал запись дважды (глобальный кап + per-owner кап могли удалить/посчитать один ключ) — удаления собираются в `BTreeSet`, применяется однократно, счёт точен; заодно File `prune` получил тот же per-owner кап, что KV (единая семантика бэкендов).
5. *Лок*: holder-identity в lock-файле (uuid записывается при acquire, сверка перед remove) — холдер больше не может удалить чужой свежий лок после stale-break; провал записи identity прибирает лок и возвращает Io вместо неатрибутированного захвата; в доке лока честно задокументирована остаточная гонка stale-break (µs-окно, требует крэша холдера + двух waiter'ов; устраняется flock/fcntl при появлении зависимости).
Верно подтверждены при ревью: NLL-паттерн `get`→`?`→`remove` (последнее использование ссылки до мутации), object-safety всех трёх трейтов (dyn-использование), transparent-Display цепочка для 403-теста, `path` srcset-диагностики (`{component}.attributes.srcset`), string-continuation в e2e-ассерте, `image → img` маппинг тега, отсутствие атрибутной пост-фильтрации в артефактном пайплайне (`body_html = rendered.html` напрямую).

Следующий по плану: srcset §3.1 (пункт 4 таблицы §10) — последний из очереди.

### Фикс 6 (2026-11-10): рендер `srcset` с per-URL-проверкой (§3.1) — РЕАЛИЗОВАН

Root cause: рендерер безусловно выбрасывал `srcset` из выхода вместе с `style`/`srcdoc`/`xlink:href`/`ping`/`background`, при этом `sizes` НЕ выбрасывался — в артефакт попадал осиротевший `sizes`. ~150 строк грамматической валидации (`validate_srcset`: дескрипторы `w`/`x`, запрет `data:`, уникальность, лимит кандидатов) были мертвы относительно выхода, а README модуля обещал «default policy permits validated `srcset`».

Реализация (4 файла; семантика «srcset рендерится целиком, если каждый URL кандидата проходит ту же политику, что `src`; один плохой кандидат роняет весь атрибут»):

- **`crates/ui/fly/src/render.rs`**: `srcset` убран из безусловного drop-списка; новый хелпер `srcset_allowed(value, policy)` — парсит структуру кандидатов (split по `,`, непустой URL + не более одного дескриптора, без лишних токенов), отвергает `data:` URL'ы явно (строже, чем `allow_data_images` для `src` — зеркалит publish-политику) и проверяет каждый URL через `url_allowed(url, UrlAttributeKind::Resource, policy)` — тот же ресурсный профиль, что у `src`/`poster`. All-or-nothing: частичный srcset молча менял бы, какое изображение выберет браузер. `sizes` остаётся как был — теперь с рендерящимся `srcset` он больше не осиротевший.
- **`crates/ui/fly/src/render.rs` (тесты)**: валидный srcset (`/img/hero.webp 800w, /img/hero@2x.webp 1600w`) + `sizes` доходят до HTML; drop-кейсы — `javascript:` кандидат, `data:` кандидат, лишний токен в кандидате, пустое значение.
- **`crates/modules/rustok-page-builder/src/static_landing.rs` (e2e)**: end-to-end «srcset в проекте → srcset в артефакте» — `compile_publish` с https-кандидатами: `body_html` содержит `srcset` и `sizes`, `verify_integrity` проходит; http-кандидат отклоняется ДО создания артефакта (`landing_srcset_rejected`, путь `attributes.srcset` — `require_secure_resource_urls` смотрит только `src`/`poster`, srcset-кандидаты доходят именно до srcset-валидации); `data:`-кандидат отклоняется аналогично. Валидация и рендер теперь когерентны: то, что пропускает политика, реально попадает в артефакт; то, что рендер может выбросить, политика не пропустит.
- **`README.md` модуля**: обещание «default policy permits validated `srcset`» теперь истинно end-to-end; добавлено описание рендер-поведения (per-candidate проверка, all-or-nothing drop).

Верификация: компиляция в песочнице невозможна. Статическое ревью: полный проход по диффу; баланс скобок проверен полноценным сканером (строки/комментарии/char-literals); строка-continuation в e2e-ассерте сверен с семантикой Rust `\`-newline (пробел перед `\` сохраняется, ведущие пробелы следующей строки съедаются); `UrlAttributeKind::Resource` в fly — семантический аналог `UrlKind::ResourceImage` publish-политики (relative || https [+ data-images, но srcset отвергает data: раньше); порядок проверок в рендере: Bool-атрибуты → scalar → srcset-ветка → generic URL-чек. **`cargo test -p fly -p rustok-page-builder` НЕ запускались** — CI.

Замечания по дизайну: (1) рендер не дублирует полную грамматику дескрипторов (уникальность/семейство w-vs-x) — это обязанность publish-гейта перед артефактом; рендер проверяет то, что важно для любого рендера (структура + URL-политика); (2) data: в srcset запрещён жёстче, чем data-images в `src` — намеренное зеркало publish-политики (`validate_srcset` отвергает `data:` до `validate_url`); (3) `sizes` при упавшем srcset остаётся в HTML превью-рендера — инертный атрибут без srcset, для артефактов исключено парой «политика пропускает только валидную пару».

Остались в очереди §10 (все P2): пункт 7 — Lightweight-ответы интентов + кэш fetch_page (§5.1); пункт 8 — компиляция вне эксклюзивного лока (§4.2); пункт 9 — согласование preview/publish политики (§6.2) + «настраиваемая» политика (§6.1); пункт 10 — обработка ошибки save без расхождения клиент/сервер (§6.4); пункт 11 — файловый стор: права, атомарность, per-tenant (§2.6). Пункт 12 (parity/stranded/draft-race/srcset тесты в scoped-наборах) закрыт фиксами 3–6.

### Фикс 7 (2026-11-10): Lightweight-ответы не-мутирующих интентов + кэш persisted-ревизии вместо fetch_page на каждый intent (§5.1) — РЕАЛИЗОВАН

Root cause: `result()` сериализовал весь документ (`GrapesJsCodec::encode_value`) в КАЖДОМ ответе интента — включая `select`/`hover`, где документ не меняется; pages-слой делал HTTP `fetch_page` на каждый intent, хотя страница нужна только для seed/reseed контроллера. На странице в 1 МБ каждый клик в редакторе = fetch + полная сериализация + мегабайт мёртвого JSON в ответе.

Предварительное исследование потребителей `result.project_data` (решало Option vs Null): TS-клиентов нет (grep apps/next-admin, apps/next-frontend пуст); `fly-browser.js` читает из ответа только `revision_id`, `project_hash`, `draft_token`, `draft_generation`, `reload` — project_data игнорирует; Dioxus/adapter-канвасы (`ui/dioxus.rs:194`, `editor/adapter_workspace.rs:51`) вызывают `dispatch_browser_intent` и результат НЕ читают (`Ok(_)`) — рендер напрямую из своего контроллера; pages-слой только пересериализует. Вывод: `Option<Value>` + `skip_serializing_if` безопасен, мутирующие пути оставлены с полным документом (консервативно, wire-совместимо).

Реализация (2 файла):

- **`crates/modules/rustok-page-builder/admin/src/browser_intent.rs`**: `BrowserIntentDispatchResult.project_data: Value` → `Option<Value>` (`#[serde(default, skip_serializing_if = "Option::is_none")]` — десериализация старых пейлоадов совместима); `dispatch_browser_intent` теперь передаёт в `result(controller, effects, is_mutating_intent(&envelope))` — тот же предикат, которым `validate_revision` решает, проверять ли revision/hash-конфликты (self-consistency: интенты без revision-чека не могут изменить документ, значит lightweight-ответ не может дезинформировать); не-мутирующие (select/hover/focus_requested/hover_requested и прочие вне `BrowserIntentKind::is_mutating` + вне расширенного списка) получают `project_data: None`, мутирующие — `Some(encode_value(document))` без изменений поведения.
- **`crates/modules/rustok-pages/admin/src/browser_intent.rs`**: (a) реструктуризация `dispatch_pages_browser_intent_with_store`: `draft_store.load` перенесён ДО fetch (было после), fetch стал ленивым — выполняется только когда нет сессии или сессия не подтверждена кэшем; (b) `PersistedRevisionCache` — статический `Mutex<HashMap<u64, (String, Instant)>>`, ключ = DefaultHasher(token, tenant_slug, page_id) (токен хэшируется — креды не дублируются в долгоживущих записях; разные токены не разделяют view ревизии), TTL 30с, лимит 512 записей (при переполнении после prune-expired карта очищается целиком — цена только лишние fetch, не ошибки); (c) fast-path: существующая сессия с `revision_id == cached` → fetch пропускается вовсе (страница нужна только для seed/reseed); (d) кэш обновляется после fetch (cache-miss/reseed) и после успешного publish — из `revision_id` ответа `PublishPageBuilderResult` (источник — только что сохранённая страница), без refetch; (e) `controller_snapshot` (context-update/post-publish пути) оставлен с `Some(...)` — консервативно, оба пути вызывают reload.
- Юнит-тесты кэша (4): roundtrip per key + победа последней записи; expiry по границе TTL (`now < expires_at`, просроченная запись удаляется); bounded (512 живых → 513-я сбрасывает карту, остаётся 1); раздельность ключей по token/tenant/page.

Корректность кэша (ключевой аргумент): save-путь авторитетен независимо от кэша — `save_project` (builder.rs:551+) сам делает `fetch_page` и возвращает `REVISION_CONFLICT: expected …, current …` при расхождении ДО записи. Stale-запись кэша лишь задерживает детектирование чужой правки в пределах TTL (30с): вместо тихого reseed-по-свежей-странице пользователь получит явный REVISION_CONFLICT на своём save. Аналогично удалённая страница: `PageNotFound` теперь может прийти на save-момент, а не на каждый клик. Сессия с MATCHING revision вообще не нуждается в странице — единственное назначение fetch было подтвердить это совпадение.

Верификация: компиляция в песочнице невозможна (без изменений). Статическое ревью: полный проход по диффу обоих файлов; баланс скобок state-machine сканером (строки/комментарии/char-literals/raw-strings) — OK; все 4 места конструирования `BrowserIntentDispatchResult` обновлены (page-builder result(), pages controller_snapshot, тест request_effects); grep-полнота: других `.project_data`-читателей результата интента нет (fly-browser.js, fly-actions.js, composition.rs, apps/admin, adapter/dioxus-канвасы); строки ≤100 символов; импорт `DefaultHasher` в стиле `std::collections::hash_map::DefaultHasher` — как в rustok-core/cache_atomic.rs. **`cargo test -p rustok-page-builder-admin -p rustok-pages-admin` НЕ запускались** — CI.

Замечания по дизайну: (1) мутирующие ответы по-прежнему несут полный документ — аудит предлагал «дифф или клиентскую реконструкцию из effects», но клиент всё равно делает full reload после мутации (`reload: is_mutating() || persistence`), поэтому дифф дал бы ноль выгоды при усложнении протокола; lightweight применён только к не-мутирующим, где выигрыш реален; (2) TTL=30с — окно позднего детектирования чужих правок; при необходимости тонкой настройки вынести в конфиг (сейчас const, как соседние MAX_*); (3) кэш per-process — многоинстансовые деплои просто не получают выигрыша, корректность не страдает (save-time check в фасаде).

Следующий по плану: фикс 8 — компиляция вне эксклюзивного лока (§4.2).

Остались в очереди §10 (все P2): пункт 8 — компиляция вне эксклюзивного лока (§4.2); пункт 9 — согласование preview/publish политики (§6.2) + «настраиваемая» политика (§6.1); пункт 10 — обработка ошибки save без расхождения клиент/сервер (§6.4); пункт 11 — файловый стор: права, атомарность, per-tenant (§2.6). Пункт 12 закрыт фиксами 3–6; пункт 7 закрыт этим фиксом.
